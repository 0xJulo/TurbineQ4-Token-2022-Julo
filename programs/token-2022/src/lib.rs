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
}
