use {
    anchor_lang::AccountDeserialize,
    anchor_spl::associated_token,
    litesvm::LiteSVM,
    litesvm_token::CreateMint,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// SPL token account amount lives at bytes 64..72
fn token_balance(svm: &LiteSVM, addr: &Pubkey) -> u64 {
    let data = svm.get_account(addr).unwrap().data;
    u64::from_le_bytes(data[64..72].try_into().unwrap())
}

// SPL mint supply lives at bytes 36..44
fn mint_supply(svm: &LiteSVM, addr: &Pubkey) -> u64 {
    let data = svm.get_account(addr).unwrap().data;
    u64::from_le_bytes(data[36..44].try_into().unwrap())
}

fn get_config(svm: &LiteSVM, addr: &Pubkey) -> amm_video::Config {
    let data = svm.get_account(addr).unwrap().data;
    amm_video::Config::try_deserialize(&mut data.as_slice()).unwrap()
}

fn treasuries(config: &Pubkey) -> (Pubkey, Pubkey) {
    (
        Pubkey::find_program_address(&[b"treasury_x", config.as_ref()], &amm_video::id()).0,
        Pubkey::find_program_address(&[b"treasury_y", config.as_ref()], &amm_video::id()).0,
    )
}

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = amm_video::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config =
        Pubkey::find_program_address(&[b"config", &123u64.to_le_bytes()], &amm_video::id()).0;
    let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;

    // Derive the PDA for the vault associated token account using the config PDA and Mint A
    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);

    (
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    )
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();

    let instruction = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_ok());

    let cfg = get_config(&svm, &config);
    assert_eq!(cfg.seed, 123);
    assert_eq!(cfg.authority, Some(payer.pubkey()));
    assert_eq!(cfg.mint_x, mint_x);
    assert_eq!(cfg.mint_y, mint_y);
    assert_eq!(cfg.fee, 30);
    assert_eq!(cfg.protocol_fee, 100);
    assert!(!cfg.locked);

    // vaults, treasuries and LP mint all start empty
    let (treasury_x, treasury_y) = treasuries(&config);
    assert_eq!(token_balance(&svm, &vault_x), 0);
    assert_eq!(token_balance(&svm, &vault_y), 0);
    assert_eq!(token_balance(&svm, &treasury_x), 0);
    assert_eq!(token_balance(&svm, &treasury_y), 0);
    assert_eq!(mint_supply(&svm, &mint_lp), 0);
}

#[test]
fn test_initialize_invalid_fee() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();

    // fee + protocol_fee >= 10_000 bps must be rejected
    let instruction = create_initialise_ix_with_fees(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, 9_950, 100,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_err());
}

#[test]
pub fn test_deposit() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok());

    let user = payer.pubkey();
    let user_x = associated_token::get_associated_token_address(&user, &mint_x);
    let user_y = associated_token::get_associated_token_address(&user, &mint_y);
    let user_lp = associated_token::get_associated_token_address(&user, &mint_lp);

    // initial deposit takes max_x/max_y and mints `amount` LP
    assert_eq!(token_balance(&svm, &vault_x), 200_000_000);
    assert_eq!(token_balance(&svm, &vault_y), 200_000_000);
    assert_eq!(token_balance(&svm, &user_x), 800_000_000);
    assert_eq!(token_balance(&svm, &user_y), 800_000_000);
    assert_eq!(token_balance(&svm, &user_lp), 100_000_000);
    assert_eq!(mint_supply(&svm, &mint_lp), 100_000_000);
}

#[test]
pub fn test_withdraw() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let withdraw_ix = create_withdraw_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(
        &mut svm,
        &[init_ix, deposit_ix, withdraw_ix],
        &payer,
        &[&payer],
    );
    assert!(res.is_ok());

    let user = payer.pubkey();
    let user_x = associated_token::get_associated_token_address(&user, &mint_x);
    let user_y = associated_token::get_associated_token_address(&user, &mint_y);
    let user_lp = associated_token::get_associated_token_address(&user, &mint_lp);

    // burning 10% of LP supply returns 10% of each vault
    assert_eq!(token_balance(&svm, &vault_x), 180_000_000);
    assert_eq!(token_balance(&svm, &vault_y), 180_000_000);
    assert_eq!(token_balance(&svm, &user_x), 820_000_000);
    assert_eq!(token_balance(&svm, &user_y), 820_000_000);
    assert_eq!(token_balance(&svm, &user_lp), 90_000_000);
    assert_eq!(mint_supply(&svm, &mint_lp), 90_000_000);
}

#[test]
pub fn test_swap() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let swap_ix = create_swap_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok());

    let user = payer.pubkey();
    let user_x = associated_token::get_associated_token_address(&user, &mint_x);
    let user_y = associated_token::get_associated_token_address(&user, &mint_y);
    let (treasury_x, treasury_y) = treasuries(&config);

    // 1% protocol fee on the 10M input goes to the treasury, remainder to the vault
    assert_eq!(token_balance(&svm, &treasury_x), 100_000);
    assert_eq!(token_balance(&svm, &treasury_y), 0);
    assert_eq!(token_balance(&svm, &vault_x), 209_900_000);
    assert_eq!(token_balance(&svm, &user_x), 790_000_000);

    // Y paid out from the vault, at least min_amount_out, LP supply untouched
    let amount_out = token_balance(&svm, &user_y) - 800_000_000;
    assert!(amount_out >= 5_000_000);
    assert_eq!(token_balance(&svm, &vault_y), 200_000_000 - amount_out);
    assert_eq!(mint_supply(&svm, &mint_lp), 100_000_000);
}

#[test]
pub fn test_lock_unlock() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let lock_ix = create_lock_ix(&payer, config);

    let res = send(&mut svm, &[init_ix, lock_ix], &payer, &[&payer]);
    assert!(res.is_ok());
    assert!(get_config(&svm, &config).locked);

    // deposit must fail while the pool is locked
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[deposit_ix.clone()], &payer, &[&payer]);
    assert!(res.is_err());

    // unlock, then the same deposit succeeds
    let unlock_ix = create_unlock_ix(&payer, config);
    let res = send(&mut svm, &[unlock_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok());
    assert!(!get_config(&svm, &config).locked);
}

#[test]
pub fn test_lock_requires_authority() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert!(res.is_ok());

    // a random signer must not be able to lock the pool
    let intruder = Keypair::new();
    svm.airdrop(&intruder.pubkey(), 1_000_000_000).unwrap();
    let lock_ix = create_lock_ix(&intruder, config);
    let res = send(&mut svm, &[lock_ix], &intruder, &[&intruder]);
    assert!(res.is_err());
}
