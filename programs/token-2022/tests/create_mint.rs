use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions}, 
    state::Mint
};

#[test]
pub fn create_mint() {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(token_2022::ID, "../../target/deploy/token_2022.so").unwrap();

    // Set up the person who pays for the transaction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Pick the address the mint will live at.
    // The payer will be the person who pays for it.
    let mint = Keypair::new();

    // The CreateMint come from the create_mint instruction, it is the bit
    // under derive Accounts. Anchor generates this from the accounts struct.
    let accounts = token_2022::accounts::CreateMint {
        payer: payer.pubkey(),
        mint: mint.pubkey(),
        token_program: anchor_spl::token_2022::ID,
        system_program: anchor_lang::system_program::ID,
    };

    // The instruction is a request to a ptogram, comprised of what program it is
    // going to call, which accounts it can touch, and the data
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: token_2022::ID,
        accounts: accounts.to_account_metas(None),
        data: token_2022::instruction::CreateMint {}.data(),
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

}