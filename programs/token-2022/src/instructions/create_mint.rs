use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_2022::spl_token_2022::{
    extension::ExtensionType,
    state::Mint,
}
use anchor_spl::token_2022_extensions::{mint_close_authority_initialize, MintCloseAuthorityInitialize};

#[derive(Accounts)]
pub struct CreateMint<'info> {

    // The payer signs to say "you may take SOL from me".
    // Creating the mints costs SOL.
    #[account(mut)]
    pub payer: Signer<'info>,

    // The mint signs to say "this new address is mine".
    // The account does not exist yet,, the signature proces
    // you own the key for the address where it wil be created.
    #[account(mut)]
    pub mint: Signer<'info>,

    // The tutors code uses the Interface<TokenInterface> type here.
    // This is where you can use the old and new token programs.
    // You've used Token2022 because you are specifically using this program.
    pub token_program: Program<'info, Token2022>,

    // The system program is used to create the account.
    pub system_program: Program<'info, System>,
}

pub fn create_mint(ctx: Context<CreateMint>) -> Result<()> {

    // THIS ORDER IS THE SAME FOR EVERY TOKEN-2022 MINT.
    // 1. Create a list of the extensions you want to use.
    // 2. Ask Token-2022 for the byte size with try_calculate_account_len
    // 3. Create the account at that size.
    // 4. Initialize each extension.
    // 5. Call InitializeMint for a mint, or InitializeAccount for a token account.
    let extensions = [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MetadataPointer,
        ExtensionType::DefaultAccountState,
        ExtensionType::MintCloseAuthority,
    ];

    let space = ExtensionType::try_calculate_account_len::<Mint>(&extensions)?;

    // Space is the account size in bytes. 
    // Lamports is the rent deposit for that size. The account
    // can stay onchain only if it holds at least this much SOL. 
    let lamports = Rent::get()?.minimum_balance(space);

    // Our program is not allow to create an account, this is for the system program.
    // We need to do a CPI to the system program in order to create this account.
    anchor_lang::system_program::create_account(
        CpiContext::new(
            ctx.accounts.system_program.key(),
            anchor_lang::system_program::CreateAccount {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.mint.to_account_info(),
            },
        ),
        lamports,
        space as u64,
        &ctx.accounts.token_program.key(),
    )?;

    // Mint Close Authroity Initialize takes in two arguments:
    // The token program ID and the mint account.
    // This is a Mint extension, not a Token one.
    anchor_spl::token_2022_extensions::mint_close_authority_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            MintCloseAuthorityInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
    )?;

    Ok(())
}