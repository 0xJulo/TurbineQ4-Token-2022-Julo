use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_spl::token_2022::Token2022;

#[derive(Accounts)]
pub struct ThawAccount<'info> {

    // This does not need to be mutable. The freeze authority only signs
    // the instruction doesn't create an account, so their own account doesn't change.
    pub freeze_authority: Signer<'info>,

    // This mint describes the token.
    pub mint: InterfaceAccount<'info, Mint>,

    // The token account is someone actual holding of the token.
    // It is mutable because thawing rewrites the state from frozen to normal. 
    #[account(mut)]
    pub token_account: InterfaceAccount<'info, TokenAccount>,

    // Your instruction doesn't thaw the account itself. It asks the Token 2022
    // program do it, as it is the owner and is the only thing that can act on it. 
    pub token_program: Program<'info, Token2022>,
}

pub fn thaw_account(ctx: Context<ThawAccount>,) -> Result<()> {
    anchor_spl::token_2022::thaw_account(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            anchor_spl::token_2022::ThawAccount {
                account: ctx.accounts.token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.freeze_authority.to_account_info(),
            },
        ),
    )?;
    
    Ok(())
}
