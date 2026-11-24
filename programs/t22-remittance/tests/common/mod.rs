#![allow(dead_code)]

pub mod confidential_helpers;

use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};

// THIS CODE IS COPIED FROM CREATE MINT BUT CAN BE REUSED
// IN FUTURE TESTS WITHOUT DOING THE SAME SET UP EACH TIME.

pub fn setup() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(t22_remittance::ID, "../../target/deploy/t22_remittance.so").unwrap();

    // Set up the person who pays for the transaction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Pick the address the mint will live at.
    // The payer will be the person who pays for it.
    let mint = Keypair::new();

    // The CreateMint come from the create_mint instruction, it is the bit
    // under derive Accounts. Anchor generates this from the accounts struct.
    let accounts = t22_remittance::accounts::CreateMint {
        payer: payer.pubkey(),
        mint: mint.pubkey(),
        token_program: anchor_spl::token_2022::ID,
        system_program: anchor_lang::system_program::ID,
    };

    // The instruction is a request to a ptogram, comprised of what program it is
    // going to call, which accounts it can touch, and the data
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: accounts.to_account_metas(None),
        data: t22_remittance::instruction::CreateMint {}.data(),
    };

    // The transaction is the envelope that is send to the chain, it holds the
    // instruction, names the fee payer and carries the sigantures. Here it is
    // the payers to send SOL and the mint to claim its address.
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &mint],
        svm.latest_blockhash()
    );

    // The transaction is sent to the chain and the program is executed.
    svm.send_transaction(tx).unwrap();

    // Return the setup environment.
    (svm, payer, mint)
}

pub fn create_thawed_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    user: &Keypair,
) -> anchor_lang::prelude::Pubkey {

    // Work out the users ATA, you need to address and the mint along
    // with the token program ID to work this out.
    let user_token_account = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &user.pubkey(),
        &mint.pubkey(),
        &anchor_spl::token_2022::ID,
    );

    // Build the instruction that opens the user's token account. The built-in
    // Associated Token Account program creates it, and the payer covers the rent.
    let create_ata_ix =anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        &user.pubkey(),
        &mint.pubkey(),
        &anchor_spl::token_2022::ID,
    );

    // Only the payer signs. The user does not need to, because anyone may
    // open a token account for someone else.
    let create_ata_tx =solana_transaction::Transaction::new_signed_with_payer(
        &[create_ata_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(create_ata_tx).unwrap();

    let thaw_accounts = t22_remittance::accounts::ThawAccount {
        freeze_authority: payer.pubkey(),
        mint: mint.pubkey(),
        token_account: user_token_account,
        token_program: anchor_spl::token_2022::ID,
    };

    let thaw_ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: thaw_accounts.to_account_metas(None),
        data: t22_remittance::instruction::ThawAccount {}.data(),
    };

    let thaw_tx = solana_transaction::Transaction::new_signed_with_payer(
        &[thaw_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(thaw_tx).unwrap();

    user_token_account
}