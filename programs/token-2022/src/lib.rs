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
pub mod token_2022 {
    use super::*;

    pub fn create_mint(ctx: Context<CreateMint>) -> Result<()> {
        instructions::create_mint::create_mint(ctx)
    }

    pub fn thaw_account(ctx: Context<ThawAccount>) -> Result<()> {
        instructions::thaw_account::thaw_account(ctx)
    }
}
