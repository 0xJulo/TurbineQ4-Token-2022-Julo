use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{
        BaseStateWithExtensions, 
        ExtensionType, 
        StateWithExtensions,
        default_account_state::DefaultAccountState,
        metadata_pointer::MetadataPointer,
        mint_close_authority::MintCloseAuthority,
        transfer_fee::TransferFeeConfig,
    }, 
    state::{AccountState, Mint}
};

#[test]
pub fn create_mint() {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(t22_remittance::ID, "../../target/deploy/t22_remittance.so").unwrap();

    // Set up the person who pays for the transaction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Pick the address the mint will live at.
    // The payer will be the person who pays for it.
    let mint = Keypair::new();

    // The CreateMint come from the create_mint instruction, it is the bit
    // under derive Accounts. Anchor generates this from the accounts struct.
    let accounts = t22_remittance::accounts::CreateMint {
        payer: payer.pubkey(),
        mint: mint.pubkey(),
        token_program: anchor_spl::token_2022::ID,
        system_program: anchor_lang::system_program::ID,
    };

    // The instruction is a request to a ptogram, comprised of what program it is
    // going to call, which accounts it can touch, and the data
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: accounts.to_account_metas(None),
        data: t22_remittance::instruction::CreateMint {}.data(),
    };

    // The transaction is the envelope that is send to the chain, it holds the
    // instruction, names the fee payer and carries the sigantures. Here it is
    // the payers to send SOL and the mint to claim its address.
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &mint],
        svm.latest_blockhash()
    );

    // The transaction is sent to the chain and the program is executed.
    svm.send_transaction(tx).unwrap();

    // Get the mint account from the chain record, using the mint pubkey
    let mint_account = svm.get_account(&mint.pubkey()).unwrap();

    // Check to see whether the owner of the account is the Token 2022 program
    assert_eq!(mint_account.owner, anchor_spl::token_2022::ID);

    // This will turn the accounts raw byte data into something we can query.
    // It will allow us to check the different extensions on the account.
    let state = StateWithExtensions::<Mint>::unpack(&mint_account.data).unwrap();

    // Check the mint has 6 decimal places
    assert_eq!(state.base.decimals, 6);

    // Variables for getting the extension type from the mint account
    // Here you are extending the state variable in order to access this.
    let extensions = state.get_extension_types().unwrap();

    // Check to see whether the mint account has the different extensions
    assert!(extensions.contains(&ExtensionType::TransferFeeConfig));
    assert!(extensions.contains(&ExtensionType::MetadataPointer));
    assert!(extensions.contains(&ExtensionType::DefaultAccountState));
    assert!(extensions.contains(&ExtensionType::MintCloseAuthority));

    // Check to see if the default state account extension is in the mint.
    // Run a check to see if it is frozen.
    // This proves the "frozen until KYC" is on the mint.
    let default_account_state = state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_account_state.state, AccountState::Frozen as u8);

    // Check if the metadata pointer extension is in the mint.
    // The address at 0 points to the metadata address, and this should equal the mint pubkey
    let metadata_pointer = state.get_extension::<MetadataPointer>().unwrap();
    assert_eq!(metadata_pointer.metadata_address.0.to_bytes(), mint.pubkey().to_bytes());

    // Check if the mint close authority extension is in the mint.
    // The address at 0 points to the close authority address, and this should equal the payer pubkey
    // which is the wallet who signed and paid the rent.
    let mint_close_authority = state.get_extension::<MintCloseAuthority>().unwrap();
    assert_eq!(mint_close_authority.close_authority.0.to_bytes(), payer.pubkey().to_bytes());

    // Check if the transfer fee config extension is in the mint.
    // The transfer fee is 1% (100 basis points)
    let transfer_fee_config = state.get_extension::<TransferFeeConfig>().unwrap();
    assert_eq!(u16::from(transfer_fee_config.newer_transfer_fee.transfer_fee_basis_points), 100);
    assert_eq!(u64::from(transfer_fee_config.newer_transfer_fee.maximum_fee), 1_000_000);
}