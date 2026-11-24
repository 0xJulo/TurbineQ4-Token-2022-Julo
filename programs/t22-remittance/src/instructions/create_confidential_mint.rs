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
    PermanentDelegateInitialize,
};

// THIS IS A COPY OF CREATE MINT BUT ADDING THE CONFIDENTIAL TRANSFER EXTENSIONS.

#[derive(Accounts)]
pub struct CreateConfidentialMint<'info> {

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

pub fn create_confidential_mint(ctx: Context<CreateConfidentialMint>, fee_elgamal_pubkey: [u8; 32]) -> Result<()> {

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
        ExtensionType::PermanentDelegate,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::ConfidentialTransferFeeConfig,
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

    // Permanent delegate extension.
    anchor_spl::token_2022_extensions::permanent_delegate_initialize(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            PermanentDelegateInitialize {
                token_program_id: ctx.accounts.token_program.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        &ctx.accounts.payer.key(),
    )?;

    // CONFIDENTIAL TRANSFERS
    // Anchor has no helper function for confidential transfers

    // First, build the request to Token2022 to turn on confidential transfers. 
    // The payer is the authority and manual approval of accounts
    let confidential_ix = anchor_spl::token_2022::spl_token_2022::extension::confidential_transfer::instruction::initialize_mint(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.mint.key(),
        Some(ctx.accounts.payer.key()),
        false,
        None,
    )?;

    // Invoke hands the request to Token2022 which writes the extension. 
    anchor_lang::solana_program::program::invoke(
        &confidential_ix,
        &[ctx.accounts.mint.to_account_info()],
    )?;

    let confidential_fee_ix = anchor_spl::token_2022::spl_token_2022::extension::confidential_transfer_fee::instruction::initialize_confidential_transfer_fee_config(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.mint.key(),
        Some(ctx.accounts.payer.key()),
        &fee_elgamal_pubkey.into(),
    )?;

    anchor_lang::solana_program::program::invoke(
        &confidential_fee_ix,
        &[ctx.accounts.mint.to_account_info()],
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