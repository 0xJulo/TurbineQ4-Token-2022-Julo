use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;

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

    let accounts = token_2022::accounts::CreateMint {}
}