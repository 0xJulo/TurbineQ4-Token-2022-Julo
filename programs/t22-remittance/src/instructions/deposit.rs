use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_spl::token_2022::Token2022;

#[derive(Accounts)]
pub struct Deposit<'info> {
    // The owner only signs here, the account doesn't change
    pub owner: Signer<'info>,
    // The mint is read only here, to check the deposit against it
    pub mint: InterfaceAccount<'info, Mint>,
    // The token account is the only account that changes.
    #[account(mut)]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, Token2022>,
}

pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    // Create the instruction for the deposit to confidential transfer
    let deposit_ix = anchor_spl::token_2022::spl_token_2022::extension::confidential_transfer::instruction::deposit(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.token_account.key(),
        &ctx.accounts.mint.key(),
        amount,
        ctx.accounts.mint.decimals,
        &ctx.accounts.owner.key(),
        &[],
    )?;
    // Send the request to Token 2022
    anchor_lang::solana_program::program::invoke(
        &deposit_ix,
        &[
            ctx.accounts.token_account.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.owner.to_account_info(),
        ],
    )?;
    
    Ok(())
}