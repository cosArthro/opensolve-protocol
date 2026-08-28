//! Integration tests for the SOLVE staking program.
//!
//! These run the compiled SBF binary against the real Token-2022 program
//! (dumped from mainnet into `tests/fixtures/spl_token_2022.so`) and the real
//! SOLVE mint account (`tests/fixtures/solve_mint.bin`). Unit tests cover the
//! arithmetic; everything here is the part arithmetic cannot reach —
//! instruction dispatch, account and vault creation, every access check, and
//! the CPIs.
//!
//! ```text
//! cargo-build-sbf --features test-authority
//! SBF_OUT_DIR=$PWD/target/deploy cargo test --features test-authority --test integration
//! ```

use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::{Account, AccountSharedData},
    clock::Clock,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};

const PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("GyCrZ9JQq1LAWJ3fWqn52uEnAFUmg52NHQvcatq2mcDe");
const MINT: Pubkey = Pubkey::from_str_const("GwyWFsDKW9a2ref1EWqdUS7B37Toii433zrAh9Dipump");
const TOKEN_2022: Pubkey = Pubkey::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const SYSTEM_PROGRAM: Pubkey = Pubkey::from_str_const("11111111111111111111111111111111");

/// Matches the `test-authority` feature in `src/lib.rs`. Public on purpose.
const AUTHORITY_SECRET: [u8; 64] = [
    79, 165, 188, 107, 200, 31, 127, 8, 119, 44, 36, 187, 136, 106, 248, 107, 229, 136, 47, 82,
    121, 135, 64, 29, 63, 148, 243, 62, 45, 32, 43, 209, 91, 209, 130, 70, 159, 13, 254, 197, 187,
    223, 147, 108, 181, 7, 166, 73, 89, 53, 56, 165, 192, 88, 221, 150, 22, 220, 217, 60, 175, 73,
    255, 107,
];

const WEEK: i64 = 7 * 24 * 60 * 60;
const MIN_HOLD: i64 = 3 * 24 * 60 * 60;
const START: i64 = 1_800_000_000;
const ONE_SOLVE: u64 = 1_000_000;
/// Mirrors `MIN_STAKE` in `src/lib.rs`: one hundred thousand SOLVE.
const MIN_STAKE: u64 = 100_000 * ONE_SOLVE;
const MIN_FUND: u64 = MIN_STAKE;
/// A week's worth of creator fees at the rate measured on 2026-08-25, rounded.
/// The suite used to pour tens of SOLVE, which is below the floor `fund` now
/// enforces and was never a realistic figure anyway.
const POUR: u64 = 2_000_000 * ONE_SOLVE;
const WALLET_LAMPORTS: u64 = 100_000_000_000;

// Error codes, mirroring the table in README.md.
const E_WRONG_ACCOUNT: u32 = 2;
const E_ALREADY_INITIALIZED: u32 = 3;
const E_BAD_AMOUNT: u32 = 4;
const E_NOTHING_TO_CLAIM: u32 = 6;
const E_LOCKED_OR_EMPTY: u32 = 7;
const E_POSITION_NOT_EMPTY: u32 = 8;

// ---------------------------------------------------------------- addresses

fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &PROGRAM_ID).0
}

fn pool() -> Pubkey {
    pda(&[b"pool"])
}
fn principal_vault() -> Pubkey {
    pda(&[b"vault_principal"])
}
fn reward_vault() -> Pubkey {
    pda(&[b"vault_reward"])
}
fn position(owner: &Pubkey) -> Pubkey {
    pda(&[b"position", owner.as_ref()])
}

// ------------------------------------------------------------------- setup

/// A 165-byte Token-2022 account: no delegate, no close authority, initialized.
fn token_account(owner: &Pubkey, amount: u64) -> Account {
    let mut data = vec![0u8; 165];
    data[..32].copy_from_slice(MINT.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // AccountState::Initialized
    Account { lamports: 2_039_280, data, owner: TOKEN_2022, executable: false, rent_epoch: u64::MAX }
}

struct World {
    ctx: ProgramTestContext,
    authority: Keypair,
    authority_tokens: Pubkey,
    /// The bank derives the clock from the slot, so the suite keeps its own
    /// notion of time and stamps it onto the sysvar after every warp.
    slot: u64,
    time: i64,
}

async fn setup() -> World {
    let mut pt = ProgramTest::default();
    pt.prefer_bpf(true);
    pt.add_program("solve_staking", PROGRAM_ID, None);
    pt.add_program("spl_token_2022", TOKEN_2022, None);

    // The real mainnet mint account, extensions and all.
    let mint_data =
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/solve_mint.bin"))
            .expect("tests/fixtures/solve_mint.bin");
    assert_eq!(mint_data[44], 6, "fixture must be the six-decimal SOLVE mint");
    pt.add_account(
        MINT,
        Account {
            lamports: 413_758_400,
            data: mint_data,
            owner: TOKEN_2022,
            executable: false,
            rent_epoch: u64::MAX,
        },
    );

    let authority = Keypair::try_from(&AUTHORITY_SECRET[..]).unwrap();
    let authority_tokens = Pubkey::new_unique();
    pt.add_account(
        authority.pubkey(),
        Account {
            lamports: WALLET_LAMPORTS,
            data: vec![],
            owner: SYSTEM_PROGRAM,
            executable: false,
            rent_epoch: u64::MAX,
        },
    );
    pt.add_account(authority_tokens, token_account(&authority.pubkey(), 0));

    let ctx = pt.start_with_context().await;
    let mut world = World { ctx, authority, authority_tokens, slot: 1, time: START };
    world.stamp_time();
    world
}

impl World {
    fn now(&self) -> i64 {
        self.time
    }

    fn stamp_time(&mut self) {
        let clock =
            Clock { slot: self.slot, unix_timestamp: self.time, ..Clock::default() };
        self.ctx.set_sysvar(&clock);
    }

    async fn warp(&mut self, seconds: i64) {
        self.slot += 1;
        self.ctx.warp_to_slot(self.slot).expect("warp");
        self.time += seconds;
        self.stamp_time();
    }

    fn wallet(&mut self, lamports: u64) -> Keypair {
        let wallet = Keypair::new();
        self.ctx.set_account(
            &wallet.pubkey(),
            &AccountSharedData::new(lamports, 0, &SYSTEM_PROGRAM),
        );
        wallet
    }

    /// A funded wallet with a SOLVE token account holding `amount`.
    fn staker(&mut self, amount: u64) -> (Keypair, Pubkey) {
        let wallet = self.wallet(WALLET_LAMPORTS);
        let tokens = Pubkey::new_unique();
        self.ctx
            .set_account(&tokens, &AccountSharedData::from(token_account(&wallet.pubkey(), amount)));
        (wallet, tokens)
    }

    async fn account(&mut self, key: &Pubkey) -> Option<Account> {
        self.ctx.banks_client.get_account(*key).await.unwrap()
    }

    async fn token_balance(&mut self, key: &Pubkey) -> u64 {
        let account = self.account(key).await.expect("token account missing");
        u64::from_le_bytes(account.data[64..72].try_into().unwrap())
    }

    async fn lamports(&mut self, key: &Pubkey) -> u64 {
        self.account(key).await.map(|a| a.lamports).unwrap_or(0)
    }

    async fn send(&mut self, ix: Instruction, signer: &Keypair) -> Result<(), TransactionError> {
        let blockhash = self.ctx.get_new_latest_blockhash().await.unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&signer.pubkey()),
            &[signer],
            blockhash,
        );
        self.ctx.banks_client.process_transaction(tx).await.map_err(|e| match e {
            solana_program_test::BanksClientError::TransactionError(e) => e,
            solana_program_test::BanksClientError::SimulationError { err, .. } => err,
            other => panic!("unexpected banks error: {other:?}"),
        })
    }

    /// Several instructions in one transaction, which is the only way to reach
    /// the close-then-restake path.
    async fn must_many(&mut self, ixs: &[Instruction], signer: &Keypair) {
        let blockhash = self.ctx.get_new_latest_blockhash().await.unwrap();
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&signer.pubkey()),
            &[signer],
            blockhash,
        );
        if let Err(e) = self.ctx.banks_client.process_transaction(tx).await {
            panic!("transaction should have succeeded: {e:?}");
        }
    }

    async fn must(&mut self, ix: Instruction, signer: &Keypair) {
        if let Err(e) = self.send(ix, signer).await {
            panic!("transaction should have succeeded: {e:?}");
        }
    }

    /// Gives the authority `amount` SOLVE to fund with, as the buy-back would.
    fn credit_authority(&mut self, amount: u64) {
        let account = token_account(&self.authority.pubkey(), amount);
        let key = self.authority_tokens;
        self.ctx.set_account(&key, &AccountSharedData::from(account));
    }

    fn authority(&self) -> Keypair {
        self.authority.insecure_clone()
    }
}

fn expect_custom(result: Result<(), TransactionError>, code: u32) {
    match result {
        Err(TransactionError::InstructionError(0, InstructionError::Custom(got))) => {
            assert_eq!(got, code, "wrong error code")
        }
        other => panic!("expected custom error {code}, got {other:?}"),
    }
}

// ------------------------------------------------------------- instructions

fn ix_initialize(payer: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(pool(), false),
            AccountMeta::new(principal_vault(), false),
            AccountMeta::new(reward_vault(), false),
            AccountMeta::new_readonly(MINT, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
        ],
        data: vec![0],
    }
}

fn ix_fund(funder: &Pubkey, source: &Pubkey, amount: u64) -> Instruction {
    let mut data = vec![1];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*funder, true),
            AccountMeta::new(*source, false),
            AccountMeta::new(pool(), false),
            AccountMeta::new(reward_vault(), false),
            AccountMeta::new_readonly(MINT, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
        ],
        data,
    }
}

fn ix_stake(owner: &Pubkey, source: &Pubkey, amount: u64) -> Instruction {
    let mut data = vec![2];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(*source, false),
            AccountMeta::new(pool(), false),
            AccountMeta::new(position(owner), false),
            AccountMeta::new(principal_vault(), false),
            AccountMeta::new_readonly(MINT, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
        ],
        data,
    }
}

fn ix_claim(owner: &Pubkey, destination: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(*destination, false),
            AccountMeta::new(pool(), false),
            AccountMeta::new(position(owner), false),
            AccountMeta::new(reward_vault(), false),
            AccountMeta::new_readonly(MINT, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
        ],
        data: vec![3],
    }
}

fn ix_unstake(owner: &Pubkey, destination: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(*destination, false),
            AccountMeta::new(pool(), false),
            AccountMeta::new(position(owner), false),
            AccountMeta::new(principal_vault(), false),
            AccountMeta::new_readonly(MINT, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
        ],
        data: vec![4],
    }
}

fn ix_close(owner: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(position(owner), false),
        ],
        data: vec![5],
    }
}

// ------------------------------------------------------------- state reading

struct PoolState {
    principal_vault: Pubkey,
    reward_vault: Pubkey,
    total_weight: u128,
    period_finish: i64,
}

async fn read_pool(w: &mut World) -> PoolState {
    let data = w.account(&pool()).await.expect("pool missing").data;
    assert_eq!(&data[..4], b"SLVP");
    assert_eq!(data.len(), 149);
    PoolState {
        principal_vault: Pubkey::try_from(&data[5..37]).unwrap(),
        reward_vault: Pubkey::try_from(&data[37..69]).unwrap(),
        total_weight: u128::from_le_bytes(data[69..85].try_into().unwrap()),
        period_finish: i64::from_le_bytes(data[141..149].try_into().unwrap()),
    }
}

struct PositionState {
    owner: Pubkey,
    amount: u64,
    accrued: u64,
    unlock_at: i64,
}

async fn read_position(w: &mut World, owner: &Pubkey) -> PositionState {
    let data = w.account(&position(owner)).await.expect("position missing").data;
    assert_eq!(&data[..4], b"SLVS");
    assert_eq!(data.len(), 77);
    PositionState {
        owner: Pubkey::try_from(&data[5..37]).unwrap(),
        amount: u64::from_le_bytes(data[37..45].try_into().unwrap()),
        accrued: u64::from_le_bytes(data[61..69].try_into().unwrap()),
        unlock_at: i64::from_le_bytes(data[69..77].try_into().unwrap()),
    }
}

// -------------------------------------------------------------------- tests

#[tokio::test]
async fn initialize_creates_a_pool_and_two_clean_vaults() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;

    let state = read_pool(&mut w).await;
    assert_eq!(state.principal_vault, principal_vault());
    assert_eq!(state.reward_vault, reward_vault());
    assert_eq!(state.total_weight, 0);
    assert_eq!(state.period_finish, 0);
    assert_ne!(principal_vault(), reward_vault());

    for vault in [principal_vault(), reward_vault()] {
        let account = w.account(&vault).await.expect("vault missing");
        assert_eq!(account.owner, TOKEN_2022, "vault must belong to Token-2022");
        assert_eq!(account.data.len(), 165);
        assert_eq!(&account.data[..32], MINT.as_ref(), "vault mint");
        assert_eq!(&account.data[32..64], pool().as_ref(), "vault authority is the pool PDA");
        assert_eq!(account.data[108], 1, "vault initialized");
        assert_eq!(&account.data[72..76], &[0, 0, 0, 0], "vault must have no delegate");
        assert_eq!(&account.data[129..133], &[0, 0, 0, 0], "vault must have no close authority");
        assert_eq!(w.token_balance(&vault).await, 0);
    }
}

#[tokio::test]
async fn only_the_funding_authority_can_initialize() {
    let mut w = setup().await;
    let stranger = w.wallet(WALLET_LAMPORTS);
    expect_custom(w.send(ix_initialize(&stranger.pubkey()), &stranger).await, E_WRONG_ACCOUNT);
    assert!(w.account(&pool()).await.is_none(), "no pool was created");
}

#[tokio::test]
async fn initializing_twice_is_refused() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    expect_custom(
        w.send(ix_initialize(&authority.pubkey()), &authority).await,
        E_ALREADY_INITIALIZED,
    );
}

/// The whole intended lifecycle: two stakers of different size, two fundings,
/// claims in between, then the hold expiring, an exit and a rent refund.
#[tokio::test]
async fn two_stakers_two_fundings_then_exit() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;

    // Alice stakes twice what Bob does, so she must earn exactly twice as
    // much. There is no lock multiplier that could change that ratio.
    let (alice, alice_tokens) = w.staker(2 * MIN_STAKE);
    let (bob, bob_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, 2 * MIN_STAKE), &alice).await;
    w.must(ix_stake(&bob.pubkey(), &bob_tokens, MIN_STAKE), &bob).await;

    assert_eq!(w.token_balance(&principal_vault()).await, 3 * MIN_STAKE);
    assert_eq!(w.token_balance(&alice_tokens).await, 0);
    let staked_at = w.now();
    let alice_position = read_position(&mut w, &alice.pubkey()).await;
    assert_eq!(alice_position.owner, alice.pubkey());
    assert_eq!(alice_position.amount, 2 * MIN_STAKE);
    assert_eq!(alice_position.unlock_at, staked_at + MIN_HOLD);
    assert_eq!(read_pool(&mut w).await.total_weight, 3 * MIN_STAKE as u128);

    // Fresh stake cannot leave until the three-day hold is up.
    expect_custom(
        w.send(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await,
        E_LOCKED_OR_EMPTY,
    );

    // The owner funds with a week of creator fees, bought back as SOLVE.
    let pour = POUR;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;
    assert_eq!(w.token_balance(&reward_vault()).await, pour);
    let funded_at = w.now();
    assert_eq!(read_pool(&mut w).await.period_finish, funded_at + WEEK);

    // Halfway through, Alice holds two thirds of the pool and half the pour
    // has been emitted, so she is owed two thirds of half of it.
    w.warp(WEEK / 2).await;
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;
    let paid = w.token_balance(&alice_tokens).await;
    let expected = pour / 3;
    assert!(
        paid.abs_diff(expected) < ONE_SOLVE,
        "two thirds of half the pour: got {paid}, expected about {expected}"
    );
    assert_eq!(read_position(&mut w, &alice.pubkey()).await.accrued, 0, "claim zeroes it");
    expect_custom(
        w.send(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await,
        E_NOTHING_TO_CLAIM,
    );

    // A second pour, once the first window has run out.
    w.warp(WEEK / 2).await;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;
    w.warp(WEEK).await;

    // Alice's three days are long past, so she can take her principal back.
    w.must(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await;
    let after_exit = read_position(&mut w, &alice.pubkey()).await;
    assert_eq!(after_exit.amount, 0);
    assert_eq!(read_pool(&mut w).await.total_weight, MIN_STAKE as u128, "only Bob is left");
    assert_eq!(w.token_balance(&principal_vault()).await, MIN_STAKE);

    // Unclaimed rewards survive the exit and can still be claimed.
    assert!(after_exit.accrued > 0, "the second pour accrued before she left");
    expect_custom(w.send(ix_close(&alice.pubkey()), &alice).await, E_POSITION_NOT_EMPTY);
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;

    // Now the empty position can be closed and its rent returned.
    let before = w.lamports(&alice.pubkey()).await;
    let rent = w.lamports(&position(&alice.pubkey())).await;
    assert_eq!(rent, 1_426_800, "77-byte position rent");
    w.must(ix_close(&alice.pubkey()), &alice).await;
    let after = w.lamports(&alice.pubkey()).await;
    assert!(after > before, "rent came back: {before} -> {after}");
    assert!(w.account(&position(&alice.pubkey())).await.is_none(), "position is gone");

    // Every SOLVE that entered a vault is still accounted for.
    let paid_out = w.token_balance(&alice_tokens).await + w.token_balance(&bob_tokens).await;
    let in_vaults =
        w.token_balance(&principal_vault()).await + w.token_balance(&reward_vault()).await;
    assert_eq!(paid_out + in_vaults, 3 * MIN_STAKE + 2 * pour);
}

#[tokio::test]
async fn a_stranger_cannot_fund() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;

    // A real pour, so the refusal is about who signed and not about the size.
    let (stranger, stranger_tokens) = w.staker(MIN_FUND);
    expect_custom(
        w.send(ix_fund(&stranger.pubkey(), &stranger_tokens, MIN_FUND), &stranger).await,
        E_WRONG_ACCOUNT,
    );
    assert_eq!(w.token_balance(&reward_vault()).await, 0);
}

/// The owner claims creator fees on no fixed schedule, so funding must not be
/// gated on the previous window running out. Whatever the previous pour has
/// not paid yet is folded into the new rate instead of being stranded.
#[tokio::test]
async fn funding_is_allowed_at_any_time_and_never_drops_the_leftover() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;

    let (alice, alice_tokens) = w.staker(2 * MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;

    let pour = POUR;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;

    // Half a window later, half the pour is still undelivered. Funding again
    // here used to be refused; now it must succeed and keep that half.
    w.warp(WEEK / 2).await;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;
    let refunded_at = w.now();
    assert_eq!(
        read_pool(&mut w).await.period_finish,
        refunded_at + WEEK,
        "the window restarts from the moment of funding"
    );

    // Run the new window out. As the only staker she must end up with both
    // pours in full, which is only true if the leftover was carried over.
    w.warp(WEEK).await;
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;
    let rewards = w.token_balance(&alice_tokens).await - MIN_STAKE;
    assert!(
        rewards.abs_diff(2 * pour) < ONE_SOLVE,
        "both pours must reach the staker, got {rewards} of {}",
        2 * pour
    );
}

#[tokio::test]
async fn stake_rejects_dust() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(2 * MIN_STAKE);

    expect_custom(w.send(ix_stake(&alice.pubkey(), &alice_tokens, 0), &alice).await, E_BAD_AMOUNT);
    expect_custom(
        w.send(ix_stake(&alice.pubkey(), &alice_tokens, ONE_SOLVE), &alice).await,
        E_BAD_AMOUNT,
    );
    expect_custom(
        w.send(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE - 1), &alice).await,
        E_BAD_AMOUNT,
    );
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
}

#[tokio::test]
async fn nobody_can_touch_another_wallets_position() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    w.credit_authority(POUR);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, POUR), &authority).await;
    w.warp(WEEK).await;

    let (thief, thief_tokens) = w.staker(0);

    // Alice's position account, but signed and addressed by the thief.
    let mut steal_rewards = ix_claim(&thief.pubkey(), &thief_tokens);
    steal_rewards.accounts[3] = AccountMeta::new(position(&alice.pubkey()), false);
    expect_custom(w.send(steal_rewards, &thief).await, E_WRONG_ACCOUNT);

    let mut steal_principal = ix_unstake(&thief.pubkey(), &thief_tokens);
    steal_principal.accounts[3] = AccountMeta::new(position(&alice.pubkey()), false);
    expect_custom(w.send(steal_principal, &thief).await, E_WRONG_ACCOUNT);

    let mut steal_rent = ix_close(&thief.pubkey());
    steal_rent.accounts[1] = AccountMeta::new(position(&alice.pubkey()), false);
    expect_custom(w.send(steal_rent, &thief).await, E_WRONG_ACCOUNT);

    assert_eq!(w.token_balance(&thief_tokens).await, 0);
    assert_eq!(read_position(&mut w, &alice.pubkey()).await.amount, MIN_STAKE);
}

#[tokio::test]
async fn a_substituted_vault_or_pool_is_refused() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    w.credit_authority(POUR);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, POUR), &authority).await;
    w.warp(WEEK).await;

    // A perfectly valid token account owned by the pool PDA, just not the
    // vault the pool recorded.
    let impostor = Pubkey::new_unique();
    w.ctx.set_account(
        &impostor,
        &AccountSharedData::from(token_account(&pool(), 1_000 * ONE_SOLVE)),
    );

    let mut wrong_reward = ix_claim(&alice.pubkey(), &alice_tokens);
    wrong_reward.accounts[4] = AccountMeta::new(impostor, false);
    expect_custom(w.send(wrong_reward, &alice).await, E_WRONG_ACCOUNT);

    let mut wrong_principal = ix_unstake(&alice.pubkey(), &alice_tokens);
    wrong_principal.accounts[4] = AccountMeta::new(impostor, false);
    expect_custom(w.send(wrong_principal, &alice).await, E_WRONG_ACCOUNT);

    // And the pool state itself cannot be swapped for a byte-identical copy
    // sitting at a different address.
    let real = w.account(&pool()).await.unwrap();
    let fake_pool = Pubkey::new_unique();
    w.ctx.set_account(&fake_pool, &AccountSharedData::from(real));
    let mut wrong_pool = ix_claim(&alice.pubkey(), &alice_tokens);
    wrong_pool.accounts[2] = AccountMeta::new(fake_pool, false);
    expect_custom(w.send(wrong_pool, &alice).await, E_WRONG_ACCOUNT);

    assert_eq!(w.token_balance(&impostor).await, 1_000 * ONE_SOLVE);
}

/// Rewards that stream while nobody is staked must not vanish: they roll into
/// whatever pour comes next.
#[tokio::test]
async fn rewards_from_an_empty_pool_roll_forward() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;

    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    let pour = POUR;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;

    // She leaves at the one-week mark, having earned the whole first pour.
    w.warp(WEEK).await;
    w.must(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await;
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;
    let first = w.token_balance(&alice_tokens).await - MIN_STAKE;
    assert!(first.abs_diff(pour) < ONE_SOLVE, "sole staker takes the pour: {first}");
    assert_eq!(read_pool(&mut w).await.total_weight, 0);

    // Two idle weeks, and the owner pours into an empty pool. That used to be
    // refused outright, which handed a lone staker a way to fail the funding
    // transaction simply by leaving first. It is accepted now: with nobody to
    // pay, the emission accumulates in rollover instead of being handed out.
    w.warp(2 * WEEK).await;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;
    assert_eq!(read_pool(&mut w).await.total_weight, 0, "still nobody staked");

    // She comes back, a third pour is funded, and neither the idle weeks nor
    // the pour made while the pool stood empty may cost anyone a token.
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    w.credit_authority(pour);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, pour), &authority).await;
    w.warp(WEEK).await;
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;

    // Her wallet now holds the first pour and the two later ones. Her
    // principal is staked again, so it is not part of this balance.
    let rewards = w.token_balance(&alice_tokens).await;
    assert!(
        rewards.abs_diff(3 * pour) < ONE_SOLVE,
        "all three pours paid in full, nothing lost to the idle weeks: {rewards}"
    );
    assert!(
        w.token_balance(&reward_vault()).await < ONE_SOLVE,
        "only rounding dust may remain"
    );
}

#[tokio::test]
async fn topping_up_restarts_the_hold_on_the_whole_position() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(3 * MIN_STAKE);

    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    let first_unlock = read_position(&mut w, &alice.pubkey()).await.unlock_at;

    // Two days in, one day short of being free, she adds to the position.
    w.warp(2 * 86_400).await;
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    let now = w.now();
    let after = read_position(&mut w, &alice.pubkey()).await;
    assert_eq!(after.amount, 2 * MIN_STAKE);
    assert_eq!(after.unlock_at, now + MIN_HOLD);
    assert_eq!(
        after.unlock_at - first_unlock,
        2 * 86_400,
        "a top-up on day two pushes the whole balance out another three days"
    );

    // The original three days have passed, but the position is held again.
    w.warp(2 * 86_400).await;
    expect_custom(
        w.send(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await,
        E_LOCKED_OR_EMPTY,
    );

    // One more day and it is free.
    w.warp(86_400 + 1).await;
    w.must(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await;
    assert_eq!(read_position(&mut w, &alice.pubkey()).await.amount, 0);
}

/// Both vaults are refused as a destination. Naming the principal vault is a
/// self-transfer the token program accepts as a no-op, which would zero the
/// position and drop its weight while the tokens never moved; naming the
/// reward vault moves them somewhere no instruction can pay out of. Either way
/// a single mistyped account destroys the caller's own money, so the program
/// refuses instead of obeying.
#[tokio::test]
async fn a_vault_is_never_a_valid_destination() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    w.credit_authority(POUR);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, POUR), &authority).await;
    w.warp(WEEK).await;

    for destination in [principal_vault(), reward_vault()] {
        expect_custom(
            w.send(ix_unstake(&alice.pubkey(), &destination), &alice).await,
            E_WRONG_ACCOUNT,
        );
        expect_custom(
            w.send(ix_claim(&alice.pubkey(), &destination), &alice).await,
            E_WRONG_ACCOUNT,
        );
    }

    // Nothing moved, and the ordinary path still works.
    assert_eq!(
        read_position(&mut w, &alice.pubkey()).await.amount,
        MIN_STAKE,
        "the position survived four refusals",
    );
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;
    w.must(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await;
}

/// The most dangerous ordering in the program. `Position::new` starts
/// `reward_per_weight_paid` at zero, so a position created after the pool has
/// a history would be owed that whole history if `stake` added the amount
/// before settling. Closing and re-staking inside one transaction is the
/// shortest path into that state, and nothing else in the suite reaches it.
#[tokio::test]
async fn closing_and_restaking_in_one_transaction_earns_nothing_retroactively() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;

    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
    w.credit_authority(POUR);
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, POUR), &authority).await;

    // A whole window runs, so the pool carries a large reward-per-weight.
    w.warp(WEEK).await;
    w.must(ix_unstake(&alice.pubkey(), &alice_tokens), &alice).await;
    w.must(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await;
    let vault_before = w.token_balance(&reward_vault()).await;

    w.must_many(
        &[ix_close(&alice.pubkey()), ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE)],
        &alice,
    )
    .await;

    assert_eq!(
        read_position(&mut w, &alice.pubkey()).await.accrued,
        0,
        "a re-created position was owed the pool's history",
    );
    expect_custom(
        w.send(ix_claim(&alice.pubkey(), &alice_tokens), &alice).await,
        E_NOTHING_TO_CLAIM,
    );
    assert_eq!(
        w.token_balance(&reward_vault()).await,
        vault_before,
        "a re-created position drew on the reward vault",
    );
}

/// The mint is compared against the compiled-in address before anything reads
/// it. That check exists in every instruction that moves tokens; this pins one
/// of them so a refactor cannot quietly drop it.
#[tokio::test]
async fn a_substituted_mint_is_refused() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(2 * MIN_STAKE);

    let other_mint = Pubkey::new_unique();
    w.ctx.set_account(&other_mint, &AccountSharedData::new(1, 0, &TOKEN_2022));

    let mut wrong = ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE);
    wrong.accounts[5] = AccountMeta::new_readonly(other_mint, false);
    expect_custom(w.send(wrong, &alice).await, E_WRONG_ACCOUNT);

    // The same call with the real mint goes through.
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;
}

/// Every pour re-spreads whatever the previous window has not delivered over a
/// fresh window, so a stream of dust pours drags the schedule out: 63% paid
/// after a week instead of all of it, 99% after five. Nothing is lost and the
/// sum still converges, but the schedule is not the funding authority's to move
/// for free, so a pour has to be worth at least what a position is worth.
#[tokio::test]
async fn a_dust_pour_is_refused() {
    let mut w = setup().await;
    let authority = w.authority();
    w.must(ix_initialize(&authority.pubkey()), &authority).await;
    let (alice, alice_tokens) = w.staker(MIN_STAKE);
    w.must(ix_stake(&alice.pubkey(), &alice_tokens, MIN_STAKE), &alice).await;

    w.credit_authority(MIN_FUND);
    expect_custom(
        w.send(ix_fund(&authority.pubkey(), &w.authority_tokens, 1), &authority).await,
        E_BAD_AMOUNT,
    );
    expect_custom(
        w.send(ix_fund(&authority.pubkey(), &w.authority_tokens, MIN_FUND - 1), &authority).await,
        E_BAD_AMOUNT,
    );
    w.must(ix_fund(&authority.pubkey(), &w.authority_tokens, MIN_FUND), &authority).await;
    assert_eq!(w.token_balance(&reward_vault()).await, MIN_FUND);
}
