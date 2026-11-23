mod common;

use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions, StateWithExtensions, transfer_fee::TransferFeeAmount}, 
    state::Account
};

#[test]
pub fn transfer() {
    let (mut svm, payer, mint) = common::setup();

    let sender = Keypair::new();

    let sender_token_account = common::create_thawed_account(&mut svm, &payer, &mint, &sender);

    let recipient = Keypair::new();

    let recipient_token_account = common::create_thawed_account(&mut svm, &payer, &mint, &recipient);

    // This function is about creating new tokens. They are created
    // directly inside the senders token account, and the mints total
    // supply is increased by this amount.
    // After this, the sender balance is 1000 tokens, and the mint
    // supply is also 1000 tokens. 
    // Only the payer can do it because it is the mint authority. 
    let mint_to_ix = anchor_spl::token_2022::spl_token_2022::instruction::mint_to(
        &anchor_spl::token_2022::ID,
        &mint.pubkey(),
        &sender_token_account,
        &payer.pubkey(),
        &[],
        1_000_000_000,
    ).unwrap();

    let mint_to_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[mint_to_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash()
    );

    svm.send_transaction(mint_to_tx).unwrap();

    // Where mint creates tokens within the sender account, where as transfer
    // moves the tokens from one token account to another. 
    let transfer_account = t22_remittance::accounts::Transfer {
        sender: sender.pubkey(),
        mint: mint.pubkey(),
        from: sender_token_account,
        to: recipient_token_account,
        token_program: anchor_spl::token_2022::ID,
    };

    // This is the data that carries the amount, in this case 100 tokens. 
    // There is a 1% fee here as well. 
    let transfer_ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: transfer_account.to_account_metas(None),
        data: t22_remittance::instruction::Transfer { amount: 100_000_000 }.data(),
    };

    // Two signers in the actual transaction. Payer covers the transaction fee
    // and the sender approves moving the tokens. 
    let transfer_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        svm.latest_blockhash()
    );

    svm.send_transaction(transfer_tx).unwrap();

    let sender_account = svm.get_account(&sender_token_account).unwrap();
    let sender_state = StateWithExtensions::<Account>::unpack(&sender_account.data).unwrap();
    assert_eq!(sender_state.base.amount, 900_000_000);

    let recipient_account = svm.get_account(&recipient_token_account).unwrap();
    let recipient_state = StateWithExtensions::<Account>::unpack(&recipient_account.data).unwrap();
    assert_eq!(recipient_state.base.amount, 99_000_000);

    let withheld = recipient_state.get_extension::<TransferFeeAmount>().unwrap();
    assert_eq!(u64::from(withheld.withheld_amount), 1_000_000);
}