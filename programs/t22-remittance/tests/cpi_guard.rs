mod common;

use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions, StateWithExtensions, cpi_guard::CpiGuard}, 
    state::Account
};

#[test]
pub fn cpi_guard() {
    let (mut svm, payer, mint) = common::setup();

    let sender = Keypair::new();

    let sender_token_account = common::create_thawed_account(&mut svm, &payer, &mint, &sender);

    let recipient = Keypair::new();

    let recipient_token_account = common::create_thawed_account(&mut svm, &payer, &mint, &recipient);

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

    // Get the delegate PDA, made up of "delegate" plus the program's ID
    // It will return the address and the bump. You don't need bump so underline it.
    let (delegate, _bump) = anchor_lang::prelude::Pubkey::find_program_address(&[b"delegate"], &t22_remittance::ID);
    // This says that the delegate may move up to 200 of my tokens.
    // It is a Token 2022 instruction, and satisfies the approve aspect of the task.
    let approve_ix = anchor_spl::token_2022::spl_token_2022::instruction::approve(
        &anchor_spl::token_2022::ID,
        &sender_token_account,
        &delegate,
        &sender.pubkey(),
        &[],
        200_000_000,
    ).unwrap();
    let approve_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[approve_ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        svm.latest_blockhash()
    );
    svm.send_transaction(approve_tx).unwrap();

    let first_accounts = t22_remittance::accounts::DelegatedTransfer {
        delegate: delegate,
        mint: mint.pubkey(),
        from: sender_token_account,
        to: recipient_token_account,
        token_program: anchor_spl::token_2022::ID,
    };
    let first_ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: first_accounts.to_account_metas(None),
        data: t22_remittance::instruction::DelegatedTransfer { amount: 50_000_000 }.data(),
    };
    let first_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[first_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash()
    );
    svm.send_transaction(first_tx).unwrap();

    let after_first = svm.get_account(&recipient_token_account).unwrap();
    let after_first_state = StateWithExtensions::<Account>::unpack(&after_first.data).unwrap();
    assert_eq!(after_first_state.base.amount, 49_500_000);

    let realloc_ix = anchor_spl::token_2022::spl_token_2022::instruction::reallocate(
        &anchor_spl::token_2022::ID,
        &sender_token_account,
        &payer.pubkey(),
        &sender.pubkey(),
        &[],
        &[anchor_spl::token_2022::spl_token_2022::extension::ExtensionType::CpiGuard],
    ).unwrap();
    let guard_ix = anchor_spl::token_2022::spl_token_2022::extension::cpi_guard::instruction::enable_cpi_guard(
        &anchor_spl::token_2022::ID,
        &sender_token_account,
        &sender.pubkey(),
        &[],
    ).unwrap();
    let guard_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[realloc_ix, guard_ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        svm.latest_blockhash()
    );
    svm.send_transaction(guard_tx).unwrap();

    // Read the sender's account and check the guard is really switched on.
    let guarded = svm.get_account(&sender_token_account).unwrap();
    let guarded_state = StateWithExtensions::<Account>::unpack(&guarded.data).unwrap();
    let cpi_guard = guarded_state.get_extension::<CpiGuard>().unwrap();
    assert!(bool::from(cpi_guard.lock_cpi));

    // The same delegated transfer again, now with CPI Guard on.
    let second_accounts = t22_remittance::accounts::DelegatedTransfer {
        delegate: delegate,
        mint: mint.pubkey(),
        from: sender_token_account,
        to: recipient_token_account,
        token_program: anchor_spl::token_2022::ID,
    };
    let second_ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: second_accounts.to_account_metas(None),
        data: t22_remittance::instruction::DelegatedTransfer { amount: 50_000_000 }.data(),
    };
    // This transaction is identical to the first one, and the chain rejects an exact
    // repeat. Moving on to a new blockhash makes it a different transaction.
    svm.expire_blockhash();
    let second_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[second_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash()
    );
    svm.send_transaction(second_tx).unwrap();

    // The recipient now holds two lots of 49.5 tokens, so 99 in total.
    let after_second = svm.get_account(&recipient_token_account).unwrap();
    let after_second_state = StateWithExtensions::<Account>::unpack(&after_second.data).unwrap();
    assert_eq!(after_second_state.base.amount, 99_000_000);
}