use anchor_lang::prelude::*;
use anchor_spl::token_2022::{
    Token2022, 
    initialize_mint2, 
    InitializeMint2,
};
use anchor_spl::token_2022::spl_token_2022::{
    extension::ExtensionType,
    state::{Mint, AccountState},
};
use anchor_spl::token_2022_extensions::{
    MintCloseAuthorityInitialize,
    MetadataPointerInitialize,
    DefaultAccountStateInitialize,
    TransferFeeInitialize,
};

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

    // Metadata pointer is a mint extension, not a token one.
    // It tells wallets where the token's metadata lives. 
    anchor_spl::token_2022_extensions::metadata_pointer_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            MetadataPointerInitialize { 
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(ctx.accounts.payer.key()),
        Some(ctx.accounts.mint.key()),
    )?;

    // Default account state is a mint extension.
    // It writes the starting rule for every new token account, frozen means that
    // the a new holder cannot send tokens until their ownn account is thawed.
    anchor_spl::token_2022_extensions::default_account_state_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            DefaultAccountStateInitialize { 
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        &AccountState::Frozen,
    )?;

    // Transfer fee is a mint extension.
    // It takes a fee on every transfer. The payer can change the rate later. 
    anchor_spl::token_2022_extensions::transfer_fee_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferFeeInitialize { 
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        Some(&ctx.accounts.payer.key()),
        Some(&ctx.accounts.payer.key()),
        100,
        1_000_000,
    )?;

    // InitializeMint2 seals the mint, it writes the decimals, who may mint
    // and who may freeze it. 
    // This is not an extension, and after this has run no other extensions can be used.
    initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            InitializeMint2 {
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        6,
        &ctx.accounts.payer.key(),
        Some(&ctx.accounts.payer.key()),
    )?;

    Ok(())
}