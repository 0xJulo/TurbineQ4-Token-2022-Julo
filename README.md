# Week 4 Assignment (Token-2022)

The goal of this assignment is to issue a remittance stablecoin using Token-2022 extensions. The token needs a fee on every transfer, new accounts that stay frozen until KYC clears, a pointer to on-chain metadata, and the ability to close the mint if it is ever decommissioned.

Part 1, the public mint:

1. Create a mint with four extensions: TransferFeeConfig, MetadataPointer (pointed at the mint itself), DefaultAccountState (frozen) and MintCloseAuthority.
2. Write a transfer instruction that uses `transfer_checked_with_fee`, with the fee worked out by `calculate_epoch_fee`.
3. Read mint and token account state only through `StateWithExtensions`.
4. Write the unfreeze path, where the freeze authority thaws one account after "KYC".

Part 2, the re-issued mint:

5. Re-issue the mint with the same extensions, plus PermanentDelegate and confidential transfers with manual approval.
6. Implement the confidential lifecycle: configure, deposit, apply pending balance, transfer and withdraw.

Part 1 is complete and covered below. Part 2 has not been started yet.

The program is called `t22_remittance` and lives in `programs/t22-remittance`. In the code, `t22_remittance::` is this program and `anchor_spl::token_2022::` is the official Token-2022 program.

## Results (Part 1)

### **Step 1:** Setting up the workspace

- Created the Anchor workspace with `anchor init` and removed the default counter template
- `anchor-spl` is used with the `token_2022` and `token_2022_extensions` features
- The Rust toolchain is pinned to 1.89 in `rust-toolchain.toml` to match the Solana build tools
- Each instruction has its own file in `src/instructions/`, and `src/lib.rs` only holds the entry points

### **Step 2:** Create mint

- `src/instructions/create_mint.rs` creates the mint with all four extensions
- Both the payer and the mint sign. The mint account does not exist yet, so its signature proves ownership of the address it will be created at
- The creation order is the same for every Token-2022 mint:
  1. List the extensions
  2. Work out the account size with `ExtensionType::try_calculate_account_len`
  3. Create the account at that size, owned by the Token-2022 program, with a CPI to the System Program
  4. Initialize each extension
  5. Call `initialize_mint2` last
- The four extensions are set up as follows:
  - MintCloseAuthority: the payer can close the mint
  - MetadataPointer: the metadata address is the mint itself
  - DefaultAccountState: every new token account starts frozen
  - TransferFeeConfig: 100 basis points (1%), capped at 1,000,000 base units (1 token)
- `initialize_mint2` sets 6 decimals and makes the payer both the mint authority and the freeze authority
  - A freeze authority is required here. Without one, nobody could thaw the accounts that start frozen

### **Step 3:** Thaw account

- `src/instructions/thaw_account.rs` unfreezes one token account. This is the "KYC" gate
- Four accounts are needed: the freeze authority (signer), the mint, the token account (`mut`) and the Token-2022 program
- The instruction is a single CPI to Token-2022's `thaw_account`
- Token-2022 checks that the signer matches the freeze authority stored on the mint, so nobody else can thaw an account
- Freeze and thaw are basic token program features. The extension only adds the rule that new accounts start frozen
- Thawing changes one token account. The mint's default stays frozen, so the next new account is still locked

### **Step 4:** Transfer with fee

- `src/instructions/transfer.rs` moves tokens from one token account to another and takes the amount as an argument
- Five accounts are needed: the sender (signer), the mint, the `from` and `to` token accounts (both `mut`) and the Token-2022 program
- The function does three things:
  1. Reads the mint's raw bytes and decodes them with `StateWithExtensions`, then pulls out `TransferFeeConfig`
  2. Gets the current epoch from the Clock sysvar and works out the fee with `calculate_epoch_fee(epoch, amount)`
  3. Calls `transfer_checked_with_fee` with the amount, the mint's decimals and that fee
- The fee is worked out at the current epoch instead of using a stored rate, because a fee change only takes effect at a later epoch
- Token-2022 rejects the transfer if the stated fee does not match the mint's rule
- The fee is not sent to the issuer during the transfer. It is withheld inside the recipient's token account until the issuer collects it

### **Step 5:** Testing with LiteSVM

- There is one test file per instruction in `tests/`, and each test starts from a fresh LiteSVM chain
- `tests/common/mod.rs` holds two helpers shared between tests:
  - `setup()` starts the chain, loads the program from `target/deploy/t22_remittance.so`, airdrops 1 SOL to a new payer and creates the mint. It returns the chain, the payer and the mint
  - `create_thawed_account()` opens an Associated Token Account for a wallet, thaws it and returns its address
- All state is read back through `StateWithExtensions`, for both the mint and token accounts
- `tests/create_mint.rs` creates the mint and checks:
  - The account exists, is owned by Token-2022 and has 6 decimals
  - All four extensions are on the mint
  - The default account state is frozen
  - The metadata pointer points at the mint
  - The close authority is the payer
  - The fee is 100 basis points with a maximum of 1,000,000
- `tests/thaw_account.rs` opens a token account for a new user and checks:
  - The account starts frozen
  - After the thaw instruction, its state is `Initialized`
- `tests/transfer.rs` thaws a sender and a recipient, mints 1,000 tokens to the sender and transfers 100. It checks:
  - The sender has 900 tokens left
  - The recipient has 99 usable tokens
  - 1 token is withheld as the fee in the recipient's token account

## Running the tests

```bash
anchor build
cargo test
```

`anchor build` must run first, as the tests load the compiled program from `target/deploy/`. It also needs to be run again after any change to the program's source.

All 3 tests pass: `create_mint`, `thaw_account` and `transfer`.
