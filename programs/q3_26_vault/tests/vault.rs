use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    q3_26_vault::{state::VaultState, STATE, VAULT_SEED},
    solana_instruction::error::InstructionError,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    solana_transaction_error::TransactionError,
};

const INITIAL_AIRDROP: u64 = 2_000_000_000;
const DEPOSIT: u64 = 500_000_000;
const WITHDRAW: u64 = 100_000_000;

/// Anchor custom error codes for the vault program.
const ERR_INVALID_AMOUNT: u32 = 6000;
const ERR_INSUFFICIENT_BALANCE: u32 = 6001;
/// Anchor `ConstraintSeeds` error code (2006).
const ERR_CONSTRAINT_SEEDS: u32 = 2006;

fn setup() -> (LiteSVM, Keypair) {
    let program_id = q3_26_vault::id();
    let user = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/q3_26_vault.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&user.pubkey(), INITIAL_AIRDROP).unwrap();
    (svm, user)
}

fn pdas(program_id: &Pubkey, user: &Pubkey) -> (Pubkey, Pubkey) {
    let (vault_state, _) = Pubkey::find_program_address(&[STATE, user.as_ref()], program_id);
    let (vault, _) = Pubkey::find_program_address(&[VAULT_SEED, user.as_ref()], program_id);
    (vault_state, vault)
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

fn initialize_ix(user: &Pubkey, vault_state: &Pubkey, vault: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_vault::id(),
        &q3_26_vault::instruction::Initialize {}.data(),
        q3_26_vault::accounts::Initialize {
            user: *user,
            vault_state: *vault_state,
            vault: *vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn deposit_ix(user: &Pubkey, vault_state: &Pubkey, vault: &Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_vault::id(),
        &q3_26_vault::instruction::Deposit { amount }.data(),
        q3_26_vault::accounts::Deposit {
            user: *user,
            vault_state: *vault_state,
            vault: *vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn withdraw_ix(user: &Pubkey, vault_state: &Pubkey, vault: &Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_vault::id(),
        &q3_26_vault::instruction::Withdraw { amount }.data(),
        q3_26_vault::accounts::Withdraw {
            user: *user,
            vault_state: *vault_state,
            vault: *vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn close_ix(user: &Pubkey, vault_state: &Pubkey, vault: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        q3_26_vault::id(),
        &q3_26_vault::instruction::Close {}.data(),
        q3_26_vault::accounts::Close {
            user: *user,
            vault_state: *vault_state,
            vault: *vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn read_vault_state(svm: &LiteSVM, key: &Pubkey) -> VaultState {
    let account = svm
        .get_account(key)
        .unwrap_or_else(|| panic!("missing account {key}"));
    let mut data = account.data.as_ref();
    VaultState::try_deserialize(&mut data).unwrap()
}

#[test]
fn test_initialize() {
    let (mut svm, user) = setup();
    let program_id = q3_26_vault::id();
    let (vault_state, vault) = pdas(&program_id, &user.pubkey());

    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    assert!(
        svm.get_account(&vault_state).is_some(),
        "vault_state should exist after initialize"
    );
    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt,
        "vault should be rent-exempt after initialize"
    );

    let state = read_vault_state(&svm, &vault_state);
    let (_, state_bump) =
        Pubkey::find_program_address(&[STATE, user.pubkey().as_ref()], &program_id);
    let (_, vault_bump) =
        Pubkey::find_program_address(&[VAULT_SEED, user.pubkey().as_ref()], &program_id);
    assert_eq!(state.state_bump, state_bump);
    assert_eq!(state.vault_bump, vault_bump);
}

#[test]
fn test_initialize_twice_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());

    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    expect_failure(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    );
}

#[test]
fn test_deposit() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    send(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();

    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT,
        "vault should increase by the deposit"
    );
}

#[test]
fn test_deposit_zero_amount_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    expect_custom_error(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, 0)],
        ERR_INVALID_AMOUNT,
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt,
        "zero deposit must not move lamports"
    );
}

#[test]
fn test_deposit_unauthorized_fails() {
    let (mut svm, victim) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &victim.pubkey());
    send(
        &mut svm,
        &[&victim],
        &[initialize_ix(&victim.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), INITIAL_AIRDROP).unwrap();

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    expect_custom_error(
        &mut svm,
        &[&attacker],
        &[deposit_ix(
            &attacker.pubkey(),
            &vault_state,
            &vault,
            DEPOSIT,
        )],
        ERR_CONSTRAINT_SEEDS,
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt,
        "unauthorized deposit must not move lamports"
    );
}

#[test]
fn test_deposit_before_initialize_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    expect_failure(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    );
}

#[test]
fn test_withdraw() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    send(
        &mut svm,
        &[&user],
        &[withdraw_ix(&user.pubkey(), &vault_state, &vault, WITHDRAW)],
    )
    .unwrap();

    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT - WITHDRAW,
        "vault should decrease by the withdrawn amount"
    );
}

#[test]
fn test_withdraw_zero_amount_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();

    expect_custom_error(
        &mut svm,
        &[&user],
        &[withdraw_ix(&user.pubkey(), &vault_state, &vault, 0)],
        ERR_INVALID_AMOUNT,
    );
}

#[test]
fn test_withdraw_more_than_balance_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    expect_custom_error(
        &mut svm,
        &[&user],
        &[withdraw_ix(
            &user.pubkey(),
            &vault_state,
            &vault,
            rent_exempt + DEPOSIT + 1,
        )],
        ERR_INSUFFICIENT_BALANCE,
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent_exempt + DEPOSIT,
        "failed withdraw must not move lamports"
    );
}

#[test]
fn test_withdraw_reserves_rent() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();

    let rent_exempt = svm.minimum_balance_for_rent_exemption(0);
    // All of the balance above the rent reserve can be withdrawn.
    send(
        &mut svm,
        &[&user],
        &[withdraw_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();
    assert_eq!(svm.get_balance(&vault).unwrap(), rent_exempt);

    // Nothing above the rent reserve is left; a further withdraw must fail.
    expect_custom_error(
        &mut svm,
        &[&user],
        &[withdraw_ix(&user.pubkey(), &vault_state, &vault, 1)],
        ERR_INSUFFICIENT_BALANCE,
    );
}

#[test]
fn test_withdraw_unauthorized_fails() {
    let (mut svm, victim) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &victim.pubkey());
    send(
        &mut svm,
        &[&victim],
        &[
            initialize_ix(&victim.pubkey(), &vault_state, &vault),
            deposit_ix(&victim.pubkey(), &vault_state, &vault, DEPOSIT),
        ],
    )
    .unwrap();

    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), INITIAL_AIRDROP).unwrap();

    let vault_balance_before = svm.get_balance(&vault).unwrap();
    expect_custom_error(
        &mut svm,
        &[&attacker],
        &[withdraw_ix(
            &attacker.pubkey(),
            &vault_state,
            &vault,
            WITHDRAW,
        )],
        ERR_CONSTRAINT_SEEDS,
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        vault_balance_before,
        "unauthorized withdraw must not move lamports"
    );
}

#[test]
fn test_withdraw_before_initialize_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    expect_failure(
        &mut svm,
        &[&user],
        &[withdraw_ix(&user.pubkey(), &vault_state, &vault, WITHDRAW)],
    );
}

#[test]
fn test_close() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    )
    .unwrap();

    let user_lamports_before = svm.get_balance(&user.pubkey()).unwrap();
    send(
        &mut svm,
        &[&user],
        &[close_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    assert!(
        svm.get_account(&vault_state).is_none(),
        "vault_state should be closed"
    );
    assert!(svm.get_account(&vault).is_none(), "vault should be closed");
    assert!(
        svm.get_balance(&user.pubkey()).unwrap() > user_lamports_before,
        "closing the vault should return lamports to the user"
    );
}

#[test]
fn test_close_unauthorized_fails() {
    let (mut svm, victim) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &victim.pubkey());
    send(
        &mut svm,
        &[&victim],
        &[initialize_ix(&victim.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), INITIAL_AIRDROP).unwrap();

    expect_custom_error(
        &mut svm,
        &[&attacker],
        &[close_ix(&attacker.pubkey(), &vault_state, &vault)],
        ERR_CONSTRAINT_SEEDS,
    );
    assert!(
        svm.get_account(&vault_state).is_some(),
        "unauthorized close must not close the vault"
    );
}

#[test]
fn test_close_twice_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[close_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    expect_failure(
        &mut svm,
        &[&user],
        &[close_ix(&user.pubkey(), &vault_state, &vault)],
    );
}

#[test]
fn test_deposit_after_close_fails() {
    let (mut svm, user) = setup();
    let (vault_state, vault) = pdas(&q3_26_vault::id(), &user.pubkey());
    send(
        &mut svm,
        &[&user],
        &[initialize_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();
    send(
        &mut svm,
        &[&user],
        &[close_ix(&user.pubkey(), &vault_state, &vault)],
    )
    .unwrap();

    expect_failure(
        &mut svm,
        &[&user],
        &[deposit_ix(&user.pubkey(), &vault_state, &vault, DEPOSIT)],
    );
}
