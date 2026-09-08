use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::{
        get_associated_token_address_with_program_id, ID as ASSOCIATED_TOKEN_PROGRAM_ID,
    },
    escrowq32026::{state::Escrow, ESCROW_SEED},
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    litesvm_token::{
        get_spl_account, spl_token::state::Account as TokenAccount, CreateAssociatedTokenAccount,
        CreateMint, MintTo, TOKEN_ID as TOKEN_PROGRAM_ID,
    },
    solana_instruction::error::InstructionError,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_transaction_error::TransactionError,
};

const INITIAL_AIRDROP: u64 = 2_000_000_000;
const DECIMALS: u8 = 6;
const DEPOSIT_A: u64 = 1_000;
const RECEIVE_B: u64 = 400;

/// Anchor custom error codes for the escrow program.
const ERR_INVALID_AMOUNT: u32 = 6000;
const ERR_INSUFFICIENT_BALANCE: u32 = 6002;
const ERR_INVALID_TERMS: u32 = 6005;
/// Anchor `ConstraintSeeds` error code (2006).
const ERR_CONSTRAINT_SEEDS: u32 = 2006;

struct Env {
    svm: LiteSVM,
    maker: Keypair,
    taker: Keypair,
    mint_a: Pubkey,
    mint_b: Pubkey,
    maker_ata_a: Pubkey,
    taker_ata_a: Pubkey,
    taker_ata_b: Pubkey,
    maker_ata_b: Pubkey,
    program_id: Pubkey,
}

fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, &TOKEN_PROGRAM_ID)
}

fn escrow_pda(program_id: &Pubkey, maker: &Pubkey, seed: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[ESCROW_SEED, maker.as_ref(), seed.to_le_bytes().as_ref()],
        program_id,
    )
}

fn new_env() -> Env {
    let program_id = escrowq32026::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(program_id, bytes).unwrap();

    let maker = Keypair::new();
    let taker = Keypair::new();
    svm.airdrop(&maker.pubkey(), INITIAL_AIRDROP).unwrap();
    svm.airdrop(&taker.pubkey(), INITIAL_AIRDROP).unwrap();

    let mint_a = CreateMint::new(&mut svm, &maker)
        .decimals(DECIMALS)
        .send()
        .unwrap();
    let mint_b = CreateMint::new(&mut svm, &maker)
        .decimals(DECIMALS)
        .send()
        .unwrap();

    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&maker.pubkey())
        .send()
        .unwrap();
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    // Pre-create the maker's mint_b ATA so it exists for take/unauthorized paths.
    let _maker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&maker.pubkey())
        .send()
        .unwrap();

    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, DEPOSIT_A)
        .owner(&maker)
        .send()
        .unwrap();
    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, RECEIVE_B)
        .owner(&maker)
        .send()
        .unwrap();

    let taker_ata_a = ata(&taker.pubkey(), &mint_a);
    let maker_ata_b = ata(&maker.pubkey(), &mint_b);

    Env {
        svm,
        maker,
        taker,
        mint_a,
        mint_b,
        maker_ata_a,
        taker_ata_a,
        taker_ata_b,
        maker_ata_b,
        program_id,
    }
}

fn send(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    ixs: &[Instruction],
) -> Result<(), FailedTransactionMetadata> {
    let payer = signers[0];
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx).map(|_| ())
}

fn expect_custom_error(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    ixs: &[Instruction],
    expected: u32,
) {
    match send(svm, signers, ixs) {
        Err(failed) => match failed.err {
            TransactionError::InstructionError(_, InstructionError::Custom(code)) => {
                assert_eq!(code, expected, "unexpected anchor error code")
            }
            other => panic!("unexpected transaction error: {other:?}"),
        },
        Ok(()) => panic!("transaction unexpectedly succeeded"),
    }
}

fn expect_failure(svm: &mut LiteSVM, signers: &[&Keypair], ixs: &[Instruction]) {
    assert!(
        send(svm, signers, ixs).is_err(),
        "transaction unexpectedly succeeded"
    );
}

fn token_amount(svm: &LiteSVM, addr: &Pubkey) -> u64 {
    get_spl_account::<TokenAccount>(svm, addr).unwrap().amount
}

fn make_ix(
    env: &Env,
    seed: u64,
    deposit: u64,
    receive: u64,
    escrow: Pubkey,
    vault: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        env.program_id,
        &escrowq32026::instruction::Make {
            seed,
            deposit,
            receive,
        }
        .data(),
        escrowq32026::accounts::Make {
            maker: env.maker.pubkey(),
            mint_a: env.mint_a,
            mint_b: env.mint_b,
            maker_ata_a: env.maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn take_ix(env: &Env, taker: &Keypair, maker: &Keypair, escrow: Pubkey) -> Instruction {
    let vault = ata(&escrow, &env.mint_a);
    let taker_ata_a = ata(&taker.pubkey(), &env.mint_a);
    let taker_ata_b = ata(&taker.pubkey(), &env.mint_b);
    let maker_ata_b = ata(&maker.pubkey(), &env.mint_b);
    Instruction::new_with_bytes(
        env.program_id,
        &escrowq32026::instruction::Take {}.data(),
        escrowq32026::accounts::Take {
            taker: taker.pubkey(),
            maker: maker.pubkey(),
            escrow,
            mint_a: env.mint_a,
            mint_b: env.mint_b,
            vault,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn refund_ix(env: &Env, maker: &Keypair, escrow: Pubkey) -> Instruction {
    let vault = ata(&escrow, &env.mint_a);
    let maker_ata_a = ata(&maker.pubkey(), &env.mint_a);
    Instruction::new_with_bytes(
        env.program_id,
        &escrowq32026::instruction::Refund {}.data(),
        escrowq32026::accounts::Refund {
            maker: maker.pubkey(),
            mint_a: env.mint_a,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn update_ix(env: &Env, maker: &Keypair, escrow: Pubkey, receive: u64) -> Instruction {
    Instruction::new_with_bytes(
        env.program_id,
        &escrowq32026::instruction::Update { receive }.data(),
        escrowq32026::accounts::Update {
            maker: maker.pubkey(),
            escrow,
        }
        .to_account_metas(None),
    )
}

fn make_env(env: &mut Env, seed: u64) -> Pubkey {
    let (escrow, _) = escrow_pda(&env.program_id, &env.maker.pubkey(), seed);
    let vault = ata(&escrow, &env.mint_a);
    let ix = make_ix(&env, seed, DEPOSIT_A, RECEIVE_B, escrow, vault);
    send(&mut env.svm, &[&env.maker], &[ix]).unwrap();
    escrow
}

fn read_escrow(svm: &LiteSVM, key: &Pubkey) -> Escrow {
    let account = svm
        .get_account(key)
        .unwrap_or_else(|| panic!("missing escrow account {key}"));
    let mut data = account.data.as_ref();
    Escrow::try_deserialize(&mut data).unwrap()
}

#[test]
fn test_make() {
    let mut env = new_env();
    let seed = 7;
    let (escrow, bump) = escrow_pda(&env.program_id, &env.maker.pubkey(), seed);
    let vault = ata(&escrow, &env.mint_a);

    let ix = make_ix(&env, seed, DEPOSIT_A, RECEIVE_B, escrow, vault);
    send(&mut env.svm, &[&env.maker], &[ix]).unwrap();

    assert!(
        env.svm.get_account(&escrow).is_some(),
        "escrow should exist"
    );
    assert!(env.svm.get_account(&vault).is_some(), "vault should exist");
    assert_eq!(
        token_amount(&env.svm, &env.maker_ata_a),
        0,
        "maker should have deposited all token A"
    );
    assert_eq!(
        token_amount(&env.svm, &vault),
        DEPOSIT_A,
        "vault should hold the deposit"
    );

    let state = read_escrow(&env.svm, &escrow);
    assert_eq!(state.seed, seed);
    assert_eq!(state.maker, env.maker.pubkey());
    assert_eq!(state.mint_a, env.mint_a);
    assert_eq!(state.mint_b, env.mint_b);
    assert_eq!(state.receive, RECEIVE_B);
    assert_eq!(state.bump, bump);
}

#[test]
fn test_make_zero_deposit_fails() {
    let mut env = new_env();
    let seed = 1;
    let (escrow, _) = escrow_pda(&env.program_id, &env.maker.pubkey(), seed);
    let vault = ata(&escrow, &env.mint_a);

    let ix = make_ix(&env, seed, 0, RECEIVE_B, escrow, vault);
    expect_custom_error(&mut env.svm, &[&env.maker], &[ix], ERR_INVALID_AMOUNT);
    assert_eq!(
        token_amount(&env.svm, &env.maker_ata_a),
        DEPOSIT_A,
        "failed make must not move tokens"
    );
    assert!(
        env.svm.get_account(&escrow).is_none(),
        "no escrow should be created on failed make"
    );
}

#[test]
fn test_make_zero_receive_fails() {
    let mut env = new_env();
    let seed = 1;
    let (escrow, _) = escrow_pda(&env.program_id, &env.maker.pubkey(), seed);
    let vault = ata(&escrow, &env.mint_a);

    let ix = make_ix(&env, seed, DEPOSIT_A, 0, escrow, vault);
    expect_custom_error(&mut env.svm, &[&env.maker], &[ix], ERR_INVALID_AMOUNT);
    assert_eq!(
        token_amount(&env.svm, &env.maker_ata_a),
        DEPOSIT_A,
        "failed make must not move tokens"
    );
}

#[test]
fn test_take() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 7);
    let vault = ata(&escrow, &env.mint_a);

    let maker_lamports_before = env.svm.get_balance(&env.maker.pubkey()).unwrap();
    let ix = take_ix(&env, &env.taker, &env.maker, escrow);
    send(&mut env.svm, &[&env.taker], &[ix]).unwrap();

    // Exact Token B payment: taker had exactly RECEIVE_B, now zero.
    assert_eq!(
        token_amount(&env.svm, &env.taker_ata_b),
        0,
        "taker should have paid exactly the receive amount"
    );
    // Maker received exactly the token B payment.
    assert_eq!(
        token_amount(&env.svm, &env.maker_ata_b),
        RECEIVE_B,
        "maker should have received exactly the receive amount"
    );
    // Taker received exactly the deposited token A.
    assert_eq!(
        token_amount(&env.svm, &env.taker_ata_a),
        DEPOSIT_A,
        "taker should receive exactly the deposit amount"
    );
    // Escrow and vault are closed.
    assert!(
        env.svm.get_account(&escrow).is_none(),
        "escrow should be closed"
    );
    assert!(
        env.svm.get_account(&vault).is_none(),
        "vault should be closed"
    );
    // Closing the escrow returns its lamports to the maker.
    assert!(
        env.svm.get_balance(&env.maker.pubkey()).unwrap() > maker_lamports_before,
        "closing should return escrow lamports to maker"
    );
}

#[test]
fn test_take_insufficient_token_b_fails() {
    let mut env = new_env();
    // Make with a receive amount greater than the taker's Token B balance.
    let seed = 7;
    let (escrow, _) = escrow_pda(&env.program_id, &env.maker.pubkey(), seed);
    let vault = ata(&escrow, &env.mint_a);
    let make = make_ix(&env, seed, DEPOSIT_A, RECEIVE_B + 1, escrow, vault);
    send(&mut env.svm, &[&env.maker], &[make]).unwrap();

    assert_eq!(
        token_amount(&env.svm, &env.taker_ata_b),
        RECEIVE_B,
        "taker must hold less than the required amount"
    );
    let take = take_ix(&env, &env.taker, &env.maker, escrow);
    expect_custom_error(
        &mut env.svm,
        &[&env.taker],
        &[take],
        ERR_INSUFFICIENT_BALANCE,
    );
    // Nothing should be moved on failure.
    assert_eq!(
        token_amount(&env.svm, &env.taker_ata_b),
        RECEIVE_B,
        "failed take must not move tokens"
    );
    assert!(
        env.svm.get_account(&escrow).is_some(),
        "escrow must survive a failed take"
    );
}

#[test]
fn test_take_twice_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 3);

    let take = take_ix(&env, &env.taker, &env.maker, escrow);
    send(&mut env.svm, &[&env.taker], &[take]).unwrap();

    let take_again = take_ix(&env, &env.taker, &env.maker, escrow);
    expect_failure(&mut env.svm, &[&env.taker], &[take_again]);
}

#[test]
fn test_take_by_maker_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 5);

    // Anchor rejects a duplicate mutable account (`taker` == `maker` are both
    // mutable) before the program's own Unauthorized check can run. Either way
    // the maker can never take their own escrow.
    let take = take_ix(&env, &env.maker, &env.maker, escrow);
    expect_failure(&mut env.svm, &[&env.maker], &[take]);
    assert!(
        env.svm.get_account(&escrow).is_some(),
        "escrow must survive an unauthorized take"
    );
}

#[test]
fn test_refund() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 9);
    let vault = ata(&escrow, &env.mint_a);

    let refund = refund_ix(&env, &env.maker, escrow);
    send(&mut env.svm, &[&env.maker], &[refund]).unwrap();

    assert_eq!(
        token_amount(&env.svm, &env.maker_ata_a),
        DEPOSIT_A,
        "refund should return the full deposit to the maker"
    );
    assert!(
        env.svm.get_account(&escrow).is_none(),
        "escrow should be closed"
    );
    assert!(
        env.svm.get_account(&vault).is_none(),
        "vault should be closed"
    );
}

#[test]
fn test_refund_unauthorized_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 2);

    let attacker = Keypair::new();
    env.svm
        .airdrop(&attacker.pubkey(), INITIAL_AIRDROP)
        .unwrap();

    let refund = refund_ix(&env, &attacker, escrow);
    expect_custom_error(&mut env.svm, &[&attacker], &[refund], ERR_CONSTRAINT_SEEDS);
    assert!(
        env.svm.get_account(&escrow).is_some(),
        "unauthorized refund must not close the escrow"
    );
}

#[test]
fn test_refund_twice_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 4);

    let refund = refund_ix(&env, &env.maker, escrow);
    send(&mut env.svm, &[&env.maker], &[refund]).unwrap();

    let refund_again = refund_ix(&env, &env.maker, escrow);
    expect_failure(&mut env.svm, &[&env.maker], &[refund_again]);
}

#[test]
fn test_update() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 6);

    let update = update_ix(&env, &env.maker, escrow, RECEIVE_B * 2);
    send(&mut env.svm, &[&env.maker], &[update]).unwrap();

    let state = read_escrow(&env.svm, &escrow);
    assert_eq!(state.receive, RECEIVE_B * 2, "receive terms should update");
}

#[test]
fn test_update_unauthorized_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 8);

    let attacker = Keypair::new();
    env.svm
        .airdrop(&attacker.pubkey(), INITIAL_AIRDROP)
        .unwrap();

    let update = update_ix(&env, &attacker, escrow, RECEIVE_B * 2);
    expect_custom_error(&mut env.svm, &[&attacker], &[update], ERR_CONSTRAINT_SEEDS);
    let state = read_escrow(&env.svm, &escrow);
    assert_eq!(state.receive, RECEIVE_B, "terms must not change");
}

#[test]
fn test_update_same_terms_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 11);

    let update = update_ix(&env, &env.maker, escrow, RECEIVE_B);
    expect_custom_error(&mut env.svm, &[&env.maker], &[update], ERR_INVALID_TERMS);
    let state = read_escrow(&env.svm, &escrow);
    assert_eq!(state.receive, RECEIVE_B);
}

#[test]
fn test_update_zero_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 12);

    let update = update_ix(&env, &env.maker, escrow, 0);
    expect_custom_error(&mut env.svm, &[&env.maker], &[update], ERR_INVALID_AMOUNT);
}

#[test]
fn test_update_after_close_fails() {
    let mut env = new_env();
    let escrow = make_env(&mut env, 13);

    // Close the escrow via a successful take.
    let take = take_ix(&env, &env.taker, &env.maker, escrow);
    send(&mut env.svm, &[&env.taker], &[take]).unwrap();

    let update = update_ix(&env, &env.maker, escrow, RECEIVE_B * 2);
    expect_failure(&mut env.svm, &[&env.maker], &[update]);
}

#[test]
fn test_incorrect_pda_fails() {
    let mut env = new_env();
    // Create a legitimate escrow so a valid context exists.
    let escrow = make_env(&mut env, 14);

    // Derive a (different) escrow address from the wrong seed: not the real PDA.
    let (wrong_escrow, _) = escrow_pda(&env.program_id, &env.maker.pubkey(), 999);

    // Refund against the incorrect PDA must be rejected.
    let refund = refund_ix(&env, &env.maker, wrong_escrow);
    expect_failure(&mut env.svm, &[&env.maker], &[refund]);

    // Update against the incorrect PDA must be rejected too.
    let update = update_ix(&env, &env.maker, wrong_escrow, RECEIVE_B * 2);
    expect_failure(&mut env.svm, &[&env.maker], &[update]);

    // The real escrow must be untouched.
    let state = read_escrow(&env.svm, &escrow);
    assert_eq!(state.receive, RECEIVE_B);
}
