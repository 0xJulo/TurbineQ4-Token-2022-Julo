mod common;
use common::confidential_helpers::*;

use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_zk_sdk::encryption::elgamal::ElGamalKeypair;
use spl_token_2022_interface::{
    extension::{
        confidential_transfer::{instruction as ct_ix, ConfidentialTransferAccount},
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    instruction::initialize_account3,
    state::Account,
};
use solana_zk_sdk::encryption::derivation::derive_confidential_keys;
use solana_zk_sdk::zk_elgamal_proof_program::pubkey_validity::build_pubkey_validity_proof_data;
use spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation;
use std::num::NonZeroI8;
use anchor_lang::solana_program::instruction::Instruction;
use solana_zk_elgamal_proof_interface::instruction::ProofInstruction;
use solana_zk_sdk::encryption::{
    auth_encryption::AeCiphertext,
    elgamal::{ElGamalCiphertext, ElGamalPubkey},
};
use spl_token_2022_interface::instruction::mint_to;
use spl_token_confidential_transfer_proof_generation::{
    transfer_with_fee::transfer_with_fee_split_proof_data, withdraw::withdraw_proof_data,
};

#[test]
pub fn confidential_lifecycle() {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(t22_remittance::ID, "../../target/deploy/t22_remittance.so").unwrap();

    // Set up the person who pays for the transaction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Pick the address the mint will live at.
    // The payer will be the person who pays for it.
    let mint = Keypair::new();

    // Create a new random keypair. There is a public half and a private half.
    let fee_keypair = ElGamalKeypair::new_rand();
    // Take the public half and turn it into 32 raw byts for create_confidential_mint.
    let fee_elgamal_pubkey: [u8; 32] = fee_keypair.pubkey().into();

    // The list of accounts need for create_confidential_mint.
    // Who pays, where the mint goes, and the two programs it calls.
    let accounts = t22_remittance::accounts::CreateConfidentialMint {
        payer: payer.pubkey(),
        mint: mint.pubkey(),
        token_program: anchor_spl::token_2022::ID,
        system_program: anchor_lang::system_program::ID,
    };
    // Name your program, attach the accounts you just made. The data is the fee_elgamal_pubkey.
    // you created above. This is the "letter".
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: accounts.to_account_metas(None),
        data: t22_remittance::instruction::CreateConfidentialMint {fee_elgamal_pubkey}.data(),
    };
    // This is the "envelope". It holds the letter and is signed by the payer
    // and the mint in order to claim its address.
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &mint],
        svm.latest_blockhash(),
    );
    // This send the envelope to the chain.
    svm.send_transaction(tx).unwrap();

    // This creates the customers wallet.
    let user = Keypair::new();
    // This creates the address where their tokens will live.
    let token_account = Keypair::new();
    // This calculates the size needed for the account, including the Token2022 extenstions
    // that it will carry.
    let space = ExtensionType::try_calculate_account_len::<Account>(&[
        ExtensionType::TransferFeeAmount,
        ExtensionType::ConfidentialTransferAccount,
        ExtensionType::ConfidentialTransferFeeAmount,
    ]).unwrap();
    // It uses space in order to calculate how much rent is needed.
    let lamports = svm.minimum_balance_for_rent_exemption(space);
    // A letter send to the system program, saying to create an empty account of this size
    // that it is paid for by the payer, and it is owned by the token 2022 program.
    let create_account_ix = solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &token_account.pubkey(),
        lamports,
        space as u64,
        &anchor_spl::token_2022::ID,
    );
    // A letter to the token 2022 program, saying to turn the empty account into a token account
    // for this mint specifically, and it is owned by the user.
    let init_account_ix = initialize_account3(
        &anchor_spl::token_2022::ID,
        &token_account.pubkey(),
        &mint.pubkey(),
        &user.pubkey(),
    ).unwrap();
    // The envelope that holds the create account and initialize account instructions.
    let create_account_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[create_account_ix, init_account_ix],
        Some(&payer.pubkey()),
        &[&payer, &token_account],
        svm.latest_blockhash(),
   );
   svm.send_transaction(create_account_tx).unwrap();

   // The users two keys made from their wallet.
   // elgamal is what the chain uses to encrypt amounts.
   // aes lets the user read their balances quickly
   let (elgamal, aes) = derive_confidential_keys(&user, b"").unwrap();
   // Evidence that the user really holds the secret half of the elgamal keypair.
   let proof = build_pubkey_validity_proof_data(&elgamal).unwrap();
   // Two letters here, one ask Token 2022 to switch on hidden balances for this account, starting at 0.
   // The second carries the proof to be checked.
   let configure_ixs = ct_ix::configure_account(
    &anchor_spl::token_2022::ID,
    &token_account.pubkey(),
    &mint.pubkey(),
    &aes.encrypt(0).into(),
    65536,
    &user.pubkey(),
    &[],
    ProofLocation::InstructionOffset(NonZeroI8::new(1).unwrap(), &proof),
   ).unwrap();
   // The envelope with both letters, the payer signs to pay and the user signs because
   // only the owner may configure their own account. 
   let configure_tx = solana_transaction::Transaction::new_signed_with_payer(
    &configure_ixs,
    Some(&payer.pubkey()),
    &[&payer, &user],
    svm.latest_blockhash(),
   );
   svm.send_transaction(configure_tx).unwrap();

   let account_data = svm.get_account(&token_account.pubkey()).unwrap();
   let account_state = StateWithExtensions::<Account>::unpack(&account_data.data).unwrap();
   let ct_account = account_state.get_extension::<ConfidentialTransferAccount>().unwrap();
   assert!(!bool::from(ct_account.approved));
   let approve_ix = ct_ix::approve_account(
    &anchor_spl::token_2022::ID,
    &token_account.pubkey(),
    &mint.pubkey(),
    &payer.pubkey(),
    &[]
   ).unwrap();
   let approve_tx = solana_transaction::Transaction::new_signed_with_payer(
    &[approve_ix],
    Some(&payer.pubkey()),
    &[&payer],
    svm.latest_blockhash(),
   );
   svm.send_transaction(approve_tx).unwrap();

   let approved_data = svm.get_account(&token_account.pubkey()).unwrap();
   let approved_state = StateWithExtensions::<Account>::unpack(&approved_data.data).unwrap();
   let approved_ct = approved_state.get_extension::<ConfidentialTransferAccount>().unwrap();
   assert!(bool::from(approved_ct.approved));

    // This section unlocks the users token account. The mint starts every account frozen as default.
    // This is done through the DefaultAccountState extension.

    // The list of accounts that ThawAccount needs for the instruction.
    let thaw_accounts = t22_remittance::accounts::ThawAccount {
        freeze_authority: payer.pubkey(),
        mint: mint.pubkey(),
        token_account: token_account.pubkey(),
        token_program: anchor_spl::token_2022::ID,
    };
    // It calls the thaw account program to unlock the account.
    let thaw_ix = Instruction {
        program_id: t22_remittance::ID,
        accounts: thaw_accounts.to_account_metas(None),
        data: t22_remittance::instruction::ThawAccount {}.data(),
    };
    // The payer signs the instruction and you include the instruction that needs signed. 
    // Extra signers because different actions need different people to approve them, no one else has to sign here.
    send(&mut svm, &payer, &[thaw_ix], &[]);

    // This section begins the process of minting tokens to the user.
    // It creates new tokens in the users account.

    // This is saying to mint 100,000 new tokens to user.
    // After this the tokens are ordinary public tokens that anyone can see, nothing is hidden yet
    let mint_to_ix = mint_to(
        &anchor_spl::token_2022::ID,
        &mint.pubkey(),
        &token_account.pubkey(),
        &payer.pubkey(),
        &[],
        100_000,
    ).unwrap();
    send(&mut svm, &payer, &[mint_to_ix], &[]);

    // This section is about moving the users token from the public balance to the hidden one. 

    // Ct_ix is what makes this confidential, a shortname you've imported at the top of this file.
    // This is saying to move 100,000 tokens from public into hidden.
    // The user signs to approve the deposit, the payer signs the transaction fee. 
    let deposit_ix = ct_ix::deposit(
        &anchor_spl::token_2022::ID,
        &token_account.pubkey(),
        &mint.pubkey(),
        100_000,
        6,
        &user.pubkey(),
        &[],
    ).unwrap();
    send(&mut svm, &payer, &[deposit_ix], &[&user]);

    // This is a snapshot of the hidden balance adter the deposit.
    let after_deposit = read_confidential(&svm, &token_account.pubkey());
    // Confidential balance has pending and available balances. 
    // This checks that the pending balance is 100,000 from the deposit instruction above. 
    assert_eq!(pending_balance(&after_deposit, &elgamal), 100_000);
    // This checks that the spendable balance is 0, because it hasn't been applied.
    assert_eq!(available_balance(&after_deposit, &aes), 0);

    // Move the pending balance into the available balance so the tokens can be spent.
    apply_pending(&mut svm, &payer, &token_account.pubkey(), &user, &elgamal, &aes);
    // Get the snapshot of the hidden balance after apply pending has run.
    let after_apply = read_confidential(&svm, &token_account.pubkey());
    // Check that the pending balance is 0.
    assert_eq!(pending_balance(&after_apply, &elgamal), 0);
    // Check that the available balance is 100,000.
    assert_eq!(available_balance(&after_apply, &aes), 100_000);

    // This creates the second person.
    let recipient = Keypair::new();
    // Give them a ready to use confidential account.
    let (recipient_token_account, recipient_elgamal, recipient_aes) =
        create_confidential_account(&mut svm, &payer, &mint.pubkey(), &recipient);

    // Confidential transfer
    let transfer_amount = 10_000u64;
    let expected_fee = 100u64;
    // Read the senders confidential balance.
    let sender_ct = read_confidential(&svm, &token_account.pubkey());
    // Read what is available for them to spend, this one is for the El Gamal proof.
    let sender_available: ElGamalCiphertext = sender_ct.available_balance.try_into().unwrap();
    // The same balance as encrypted with AES, the proof builder needs this.
    let sender_decryptable: AeCiphertext = sender_ct.decryptable_available_balance.try_into().unwrap();
    // The recipient's public key, needed for the proof.
    let recipient_pubkey: ElGamalPubkey = recipient_elgamal.pubkey().clone();
    // Build the proofs for the transfer.
    let proofs = transfer_with_fee_split_proof_data(
        &sender_available,
        &sender_decryptable,
        transfer_amount,
        &elgamal,
        &aes,
        &recipient_pubkey,
        None,
        fee_keypair.pubkey(),
        100,
        1_000_000,
    ).unwrap();

    // The sender hands over 5 pieces of evidence to prove the transfer is valid.

    // This checks if the sender has enough tokens to send.
    let equality_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyCiphertextCommitmentEquality,
        &proofs.equality_proof_data,
    );
    // This checks if the amount is encrypted with the right person's key and holds the right number.
    let validity_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyBatchedGroupedCiphertext3HandlesValidity,
        &proofs.transfer_amount_ciphertext_validity_proof_data_with_ciphertext.proof_data,
    );
    // This checks the fee
    let percentage_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyPercentageWithCap,
        &proofs.percentage_with_cap_proof_data,
    );
    // Proof to show the encrypted fee is the right number and encrypyted with the right key.
    let fee_validity_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyBatchedGroupedCiphertext2HandlesValidity,
        &proofs.fee_ciphertext_validity_proof_data,
    );
    // Every hidden number is a real amount, not negative and not to large.
    let range_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyBatchedRangeProofU256,
        &proofs.range_proof_data,
    );

    // This is the sender's new balance after the transfer, encrypted with their AES key
    // so they can still read their own balance afterwards.
    let new_sender_decryptable = aes.encrypt(100_000 - transfer_amount);
    // This is the letter to Token 2022 asking it to make the confidential transfer.
    // It names the sender's account, the mint and the recipient's account. The user signs.
    // The last five lines point at the saved results of the five proofs above.
    let transfer_ixs = ct_ix::transfer_with_fee(
        &anchor_spl::token_2022::ID,
        &token_account.pubkey(),
        &mint.pubkey(),
        &recipient_token_account,
        &new_sender_decryptable.into(),
        &proofs.transfer_amount_ciphertext_validity_proof_data_with_ciphertext.ciphertext_lo,
        &proofs.transfer_amount_ciphertext_validity_proof_data_with_ciphertext.ciphertext_hi,
        &user.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&equality_ctx),
        ProofLocation::ContextStateAccount(&validity_ctx),
        ProofLocation::ContextStateAccount(&percentage_ctx),
        ProofLocation::ContextStateAccount(&fee_validity_ctx),
        ProofLocation::ContextStateAccount(&range_ctx),
    ).unwrap();
    // This sends the transfer. The payer pays and the user signs because they are their tokens.
    send(&mut svm, &payer, &transfer_ixs, &[&user]);

    // The five proof results have been used, so this closes those accounts
    // and returns the rent to the payer.
    close_contexts(
        &mut svm,
        &payer,
        &[equality_ctx, validity_ctx, percentage_ctx, fee_validity_ctx, range_ctx],
    );

    // Get the snapshot of the sender's hidden balance after the transfer.
    let sender_after = read_confidential(&svm, &token_account.pubkey());
    // Check the sender has paid the full amount, so 90,000 is left.
    assert_eq!(available_balance(&sender_after, &aes), 100_000 - transfer_amount);

    // Get the snapshot of the recipient's hidden balance after the transfer.
    let recipient_after = read_confidential(&svm, &recipient_token_account);
    // Check the recipient received the amount minus the fee, 9,900, and that it is in pending.
    assert_eq!(pending_balance(&recipient_after, &recipient_elgamal), transfer_amount - expected_fee);
    // Check the recipient's spendable balance is still 0, because it hasn't been applied.
    assert_eq!(available_balance(&recipient_after, &recipient_aes), 0);

    // Check the fee of 100 is held on the recipient's account. Only the issuer's fee key can read it.
    assert_eq!(withheld_fee(&svm, &recipient_token_account, &fee_keypair), expected_fee);

    // ---- RECIPIENT APPLIES THEIR PENDING BALANCE ----
    // The recipient moves their pending balance into their available balance so it can be spent.
    // This has to happen before they can withdraw.
    apply_pending(&mut svm, &payer, &recipient_token_account, &recipient, &recipient_elgamal, &recipient_aes);

    // Get the snapshot of the recipient's hidden balance after apply pending has run.
    let recipient_applied = read_confidential(&svm, &recipient_token_account);
    // Check the 9,900 is now available to spend.
    assert_eq!(available_balance(&recipient_applied, &recipient_aes), transfer_amount - expected_fee);

    // ---- WITHDRAW ----
    // The recipient will move 1,000 from their hidden balance back to their public balance.
    let withdraw_amount = 1_000u64;
    // This is how much the recipient has available before the withdraw, 9,900.
    let recipient_available = transfer_amount - expected_fee;
    // The recipient's available balance as encrypted on chain. The proofs are built against this.
    let recipient_current: ElGamalCiphertext = recipient_applied.available_balance.try_into().unwrap();

    // Build the two proofs for the withdraw. The amount coming out is public,
    // but the balance left behind is still hidden, so it needs proving.
    let withdraw_proofs = withdraw_proof_data(
        &recipient_current,
        recipient_available,
        withdraw_amount,
        &recipient_elgamal,
    ).unwrap();

    // This checks the recipient's new balance is worked out correctly from their old one.
    let withdraw_equality_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyCiphertextCommitmentEquality,
        &withdraw_proofs.equality_proof_data,
    );
    // This checks the balance left behind is a real amount and not negative,
    // so they cannot withdraw more than they have.
    let withdraw_range_ctx = stage_proof(
        &mut svm,
        &payer,
        ProofInstruction::VerifyBatchedRangeProofU64,
        &withdraw_proofs.range_proof_data,
    );

    // This is the recipient's new balance after the withdraw, encrypted with their AES key.
    let new_recipient_decryptable = recipient_aes.encrypt(recipient_available - withdraw_amount);
    // This is the letter to Token 2022 asking it to move 1,000 from the hidden balance
    // to the public balance. The last two lines point at the saved proof results.
    let withdraw_ixs = ct_ix::withdraw(
        &anchor_spl::token_2022::ID,
        &recipient_token_account,
        &mint.pubkey(),
        withdraw_amount,
        6,
        &new_recipient_decryptable.into(),
        &recipient.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&withdraw_equality_ctx),
        ProofLocation::ContextStateAccount(&withdraw_range_ctx),
    ).unwrap();
    // This sends the withdraw. The recipient signs because they are their tokens.
    send(&mut svm, &payer, &withdraw_ixs, &[&recipient]);

    // Close the two proof result accounts and return the rent to the payer.
    close_contexts(&mut svm, &payer, &[withdraw_equality_ctx, withdraw_range_ctx]);

    // Read the recipient's token account to look at the public balance.
    let recipient_final_data = svm.get_account(&recipient_token_account).unwrap();
    let recipient_final_state = StateWithExtensions::<Account>::unpack(&recipient_final_data.data).unwrap();
    // Check the public balance is now 1,000. Anyone can see this number.
    assert_eq!(recipient_final_state.base.amount, withdraw_amount);

    // Get the final snapshot of the recipient's hidden balance.
    let recipient_final = read_confidential(&svm, &recipient_token_account);
    // Check the hidden balance has gone down by 1,000, so 8,900 is left.
    assert_eq!(available_balance(&recipient_final, &recipient_aes), recipient_available - withdraw_amount);
}


