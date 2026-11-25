use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions,StateWithExtensions, transfer_fee::TransferFeeConfig},
    state::Mint as MintState,
};

#[derive(Accounts)]
pub struct DelegatedTransfer<'info> {
    /// CHECK: This is the program's own PDA, it is used
    /// as a delegate that signs the transfer.
    #[account(seeds = [b"delegate"], bump)]
    pub delegate: UncheckedAccount<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, Token2022>,
}

pub fn delegated_transfer(ctx: Context<DelegatedTransfer>, amount: u64) -> Result<()> {
    // This gets the whole mint data.
    let mint_info = ctx.accounts.mint.to_account_info();
    // This borrows the byte data for reading.
    let mint_data = mint_info.try_borrow_data()?;
    // Decode the bytes into the mint plus its extensions.
    let mint_state = StateWithExtensions::<MintState>::unpack(&mint_data)?;
    // Pull the transfer fee rule out of the mint's extensions.
    let fee_config = mint_state.get_extension::<TransferFeeConfig>()?;
    // Ask the chain which epoch it is now, because the fee can change by epoch.
    let epoch = Clock::get()?.epoch;
    // Work out the fee for this amount at the current epoch.
    let fee = fee_config.calculate_epoch_fee(epoch, amount).ok_or(ProgramError::ArithmeticOverflow)?;
    // Read the mint's decimals, which the transfer call checks.
    let decimals =  mint_state.base.decimals;

    anchor_spl::token_2022_extensions::transfer_checked_with_fee(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            anchor_spl::token_2022_extensions::TransferCheckedWithFee {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                source: ctx.accounts.from.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                destination: ctx.accounts.to.to_account_info(),
                authority: ctx.accounts.delegate.to_account_info(),
            },
            &[&[b"delegate", &[ctx.bumps.delegate]]],
        ),
        amount,
        decimals,
        fee,
    )?;

    Ok(())
}