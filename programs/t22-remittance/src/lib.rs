pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("BKP2EJezJNLM9rzHqnsisjDgN7QpQn8BauYbbLD69L2v");

#[program]
pub mod t22_remittance {
    use super::*;

    pub fn create_mint(ctx: Context<CreateMint>) -> Result<()> {
        instructions::create_mint::create_mint(ctx)
    }

    pub fn thaw_account(ctx: Context<ThawAccount>) -> Result<()> {
        instructions::thaw_account::thaw_account(ctx)
    }

    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        instructions::transfer::transfer(ctx, amount)
    }

    pub fn create_confidential_mint(ctx: Context<CreateConfidentialMint>, fee_elgamal_pubkey: [u8; 32]) -> Result<()> {
        instructions::create_confidential_mint::create_confidential_mint(ctx, fee_elgamal_pubkey)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::deposit(ctx, amount)
    }

    pub fn apply_pending_balance(ctx: Context<ApplyPendingBalance>, expected_pending_balance_credit_counter: u64, new_decryptable_available_balance: [u8; 36]) -> Result<()> {
        instructions::apply_pending_balance::apply_pending_balance(ctx, expected_pending_balance_credit_counter, new_decryptable_available_balance)
    }
}
