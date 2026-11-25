use anchor_lang::prelude::*;
use anchor_spl::token_interface::TokenAccount;
use anchor_spl::token_2022::Token2022;

#[derive(Accounts)]
pub struct ApplyPendingBalance<'info> {
    pub owner: Signer<'info>,
    #[account(mut)]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, Token2022>,
}

pub fn apply_pending_balance(
    ctx: Context<ApplyPendingBalance>,
    expected_pending_balance_credit_counter: u64,
    new_decryptable_available_balance: [u8; 36],
) -> Result<()> {
    let apply_ix = anchor_spl::token_2022::spl_token_2022::extension::confidential_transfer::instruction::apply_pending_balance(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.token_account.key(),
        expected_pending_balance_credit_counter,
        &new_decryptable_available_balance.into(),
        &ctx.accounts.owner.key(),
        &[],
    )?;
    anchor_lang::solana_program::program::invoke(
        &apply_ix,
        &[
            ctx.accounts.token_account.to_account_info(),
            ctx.accounts.owner.to_account_info(),
        ],
    )?;
    Ok(())
}