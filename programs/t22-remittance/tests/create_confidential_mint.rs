use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022::{
    extension::{
        BaseStateWithExtensions, 
        ExtensionType, 
        StateWithExtensions,
        permanent_delegate::PermanentDelegate,
        confidential_transfer::ConfidentialTransferMint,
    }, 
    state::{AccountState, Mint}
};
use solana_zk_sdk::encryption::elgamal::ElGamalKeypair;

#[test]
pub fn create_confidential_mint() {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(t22_remittance::ID, "../../target/deploy/t22_remittance.so").unwrap();

    // Set up the person who pays for the transaction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Pick the address the mint will live at.
    // The payer will be the person who pays for it.
    let mint = Keypair::new();

    let fee_keypair = ElGamalKeypair::new_rand();
    let fee_elgamal_pubkey: [u8; 32] = fee_keypair.pubkey().into();

    let accounts = t22_remittance::accounts::CreateConfidentialMint {
        payer: payer.pubkey(),
        mint: mint.pubkey(),
        token_program: anchor_spl::token_2022::ID,
        system_program: anchor_lang::system_program::ID,
    };

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: t22_remittance::ID,
        accounts: accounts.to_account_metas(None),
        data: t22_remittance::instruction::CreateConfidentialMint {fee_elgamal_pubkey}.data(),
    };

    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &mint],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).unwrap();

    let mint_account = svm.get_account(&mint.pubkey()).unwrap();
    let state = StateWithExtensions::<Mint>::unpack(&mint_account.data).unwrap();
    let extensions = state.get_extension_types().unwrap();

    assert!(extensions.contains(&ExtensionType::TransferFeeConfig));
    assert!(extensions.contains(&ExtensionType::MetadataPointer));
    assert!(extensions.contains(&ExtensionType::DefaultAccountState));
    assert!(extensions.contains(&ExtensionType::MintCloseAuthority));
    assert!(extensions.contains(&ExtensionType::PermanentDelegate));
    assert!(extensions.contains(&ExtensionType::ConfidentialTransferMint));
    assert!(extensions.contains(&ExtensionType::ConfidentialTransferFeeConfig));

    let permanent_delegate = state.get_extension::<PermanentDelegate>().unwrap();

    assert_eq!(permanent_delegate.delegate.0.to_bytes(), payer.pubkey().to_bytes());

    let confidential = state.get_extension::<ConfidentialTransferMint>().unwrap();

    assert!(!bool::from(confidential.auto_approve_new_accounts));

}