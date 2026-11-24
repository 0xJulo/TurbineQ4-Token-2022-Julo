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
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_zk_elgamal_proof_interface::{
    instruction::{close_context_state, ContextStateInfo, ProofInstruction},
    proof_data::ZkProofData,
    state::ProofContextState,
};
use solana_zk_sdk::encryption::{
    auth_encryption::{AeCiphertext, AeKey},
    elgamal::ElGamalCiphertext,
};
use spl_token_2022_interface::extension::confidential_transfer_fee::ConfidentialTransferFeeAmount;


// This wraps instructions into a transaction, signs it and sends it. 
// You are collecting the payer, the instruction and any extra signers needed for the extensions,
// then building the transaction alongside the instructions. 
pub fn send(svm: &mut LiteSVM, payer: &Keypair, ixs: &[Instruction], extra_signers: &[&Keypair]) {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra_signers);
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        &signers,
        svm.latest_blockhash(),
    );
    if let Err(e) = svm.send_transaction(tx) {
        panic!("transaction failed: {:#?}", e.meta.logs);
    }
}
// This fetches the hidden balance data from the token account. 
// You fetch the account and unwrap it, then use StateWithExtensions to unpack it and get the data.
// ConfidentialTransferAccount is the extension that holds the hidden balance data.
pub fn read_confidential(svm: &LiteSVM, account: &Pubkey) -> ConfidentialTransferAccount {
    let data = svm.get_account(account).unwrap();
    let state = StateWithExtensions::<Account>::unpack(&data.data).unwrap();
    *state.get_extension::<ConfidentialTransferAccount>().unwrap()
}
// Pending balance is when tokens have arrived but the owner hasn't applied them yet.
// This function decrypt the bending balance and gives it back as a plain number.
// The balance is stored in two parts, low and high, encrypted separately.
pub fn pending_balance(ct: &ConfidentialTransferAccount, elgamal: &ElGamalKeypair) -> u64 {
    let lo: ElGamalCiphertext = ct.pending_balance_lo.try_into().unwrap();
    let hi: ElGamalCiphertext = ct.pending_balance_hi.try_into().unwrap();
    let lo = elgamal.secret().decrypt_u32(&lo).unwrap();
    let hi = elgamal.secret().decrypt_u32(&hi).unwrap();
    lo + (hi << 16)
}
// Available balance is when the owner has applied the pending balance and the tokens are now available to spend.
// This function decrypts the available balance and gives it back as a plain number.
pub fn available_balance(ct: &ConfidentialTransferAccount, aes: &AeKey) -> u64 {
    let ciphertext: AeCiphertext = ct.decryptable_available_balance.try_into().unwrap();
    aes.decrypt(&ciphertext).unwrap()
}
// This function reads how much fee is being held back on a token account, given as a plain number.
// ConfidentialTransferFeeAmount is the part that hold the encrypted withheld fee amount.
// The fee sits in the holders account but only the issuer can encrypt and read it.
pub fn withheld_fee(svm: &LiteSVM, account: &Pubkey, fee_keypair: &ElGamalKeypair) -> u64 {
    let data = svm.get_account(account).unwrap();
    let state = StateWithExtensions::<Account>::unpack(&data.data).unwrap();
    let fee_amount = state.get_extension::<ConfidentialTransferFeeAmount>().unwrap();
    let ciphertext: ElGamalCiphertext = fee_amount.withheld_amount.try_into().unwrap();
    fee_keypair.secret().decrypt_u32(&ciphertext).unwrap()
}
// This will move the accounts pending balance into the available balance so the tokens can be spent. 
// It doesn't return anything as it runs the apply_pending_balance instruction.
// Underneath this you will see the send though which actually sends the instruction to the chain.
pub fn apply_pending(
    svm: &mut LiteSVM,
    payer: &Keypair,
    account: &Pubkey,
    owner: &Keypair,
    elgamal: &ElGamalKeypair,
    aes: &AeKey,
) {
    let ct = read_confidential(svm, account);
    let counter: u64 = ct.pending_balance_credit_counter.into();
    let new_available = available_balance(&ct, aes) + pending_balance(&ct, elgamal);
    let ix = ct_ix::apply_pending_balance(
        &anchor_spl::token_2022::ID,
        account,
        counter,
        &aes.encrypt(new_available).into(),
        &owner.pubkey(),
        &[],
    ).unwrap();
    send(svm, payer, &[ix], &[owner]);
}
// This will set up a confidential token account for one user. 
// 1. It begins by creating the token account, sized for the three extensions. 
// 2. It will configure to make the owners encryption keys and switch on hidden balances
pub fn create_confidential_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Keypair,
) -> (Pubkey, ElGamalKeypair, AeKey) {
    let token_account = Keypair::new();
    let space = ExtensionType::try_calculate_account_len::<Account>(&[
        ExtensionType::TransferFeeAmount,
        ExtensionType::ConfidentialTransferAccount,
        ExtensionType::ConfidentialTransferFeeAmount,
    ]).unwrap();
    let lamports = svm.minimum_balance_for_rent_exemption(space);
    send(
        svm,
        payer,
        &[
            solana_system_interface::instruction::create_account(
                &payer.pubkey(),
                &token_account.pubkey(),
                lamports,
                space as u64,
                &anchor_spl::token_2022::ID,
            ),
            initialize_account3(&anchor_spl::token_2022::ID, &token_account.pubkey(), mint, &owner.pubkey()).unwrap(),
        ],
        &[&token_account],
    );
    // Use the owners wallet to work out the two encryption keys
    let (elgamal, aes) = derive_confidential_keys(owner, b"").unwrap();
    // A proof that the owner really holds the secret half of the elgamal keypair.
    let proof = build_pubkey_validity_proof_data(&elgamal).unwrap();
    // Builds the letters that switch on the hidden balance for this account.
    // It starts with a balance of 0 and this is encryypted with the owners AES key.
    let configure_ixs = ct_ix::configure_account(
        &anchor_spl::token_2022::ID,
        &token_account.pubkey(),
        mint,
        &aes.encrypt(0).into(),
        65536,
        &owner.pubkey(),
        &[],
        ProofLocation::InstructionOffset(NonZeroI8::new(1).unwrap(), &proof),
    ).unwrap();
    // Only the owner can configure their own account.
    send(svm, payer, &configure_ixs, &[owner]);
    // Create the instruction to approve the account using ct_ix from confidential transfer.
    let approve_ix = ct_ix::approve_account(
        &anchor_spl::token_2022::ID,
        &token_account.pubkey(),
        mint,
        &payer.pubkey(),
        &[],
    ).unwrap();
    send(svm, payer, &[approve_ix], &[]);
    // This will unlock the account so the user can start using it.
    // Default it is frozen
    let thaw_ix = Instruction {
        program_id: t22_remittance::ID,
        accounts: t22_remittance::accounts::ThawAccount {
            freeze_authority: payer.pubkey(),
            mint: *mint,
            token_account: token_account.pubkey(),
            token_program: anchor_spl::token_2022::ID,
        }.to_account_metas(None),
        data: t22_remittance::instruction::ThawAccount {}.data(),
    };
    send(svm, payer, &[thaw_ix], &[]);

    (token_account.pubkey(), elgamal, aes)
}
// This function will check one proof ahead of time and stores the result in its own account.
// Confidential transfers need several proofs, this will create an empty account owned by the 
// proof-checking program. The it will ask to check the proof and save the result there.
pub fn stage_proof<T, U>(
    svm: &mut LiteSVM,
    payer: &Keypair,
    proof_kind: ProofInstruction,
    proof: &T,
) -> Pubkey
where
    T: bytemuck::Pod + ZkProofData<U>,
    U: bytemuck::Pod,
{
    let context_len = std::mem::size_of::<ProofContextState<U>>();
    let context = Keypair::new();
    let lamports = svm.minimum_balance_for_rent_exemption(context_len);
    send(
        svm,
        payer,
        &[solana_system_interface::instruction::create_account(
            &payer.pubkey(),
            &context.pubkey(),
            lamports,
            context_len as u64,
            &solana_zk_elgamal_proof_interface::ID,
        )],
        &[&context],
    );
    let verify_ix = proof_kind.encode_verify_proof(
        Some(ContextStateInfo {
            context_state_account: &context.pubkey(),
            context_state_authority: &payer.pubkey(),
        }),
        proof,
    );
    send(
        svm,
        payer,
        &[ComputeBudgetInstruction::set_compute_unit_limit(400_000), verify_ix],
        &[],
    );
    context.pubkey()
}
// Each stored proof sits in an account that holds rent deposit, once the transfer has been done
// the account has no purpose. Leaving them open would waste SOL so this function closes them.
pub fn close_contexts(svm: &mut LiteSVM, payer: &Keypair, contexts: &[Pubkey]) {
    let ixs: Vec<Instruction> = contexts
        .iter()
        .map(|context| {
            close_context_state(
                ContextStateInfo {
                    context_state_account: context,
                    context_state_authority: &payer.pubkey(),
                },
                &payer.pubkey(),
            )
        })
        .collect();
    send(svm, payer, &ixs, &[]);
}