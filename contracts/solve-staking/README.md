# SOLVE Staking — minimal Token-2022 program

Small, purpose-built staking contract for the SOLVE Token-2022 mint
`GwyWFsDKW9a2ref1EWqdUS7B37Toii433zrAh9Dipump`.

Reserved program address: `GyCrZ9JQq1LAWJ3fWqn52uEnAFUmg52NHQvcatq2mcDe`.

## The keys

Three, all under ignored `.keys/`, none in git, and **none of them backed up by
anything the repository does**. Copy them off this machine by hand.

| File | Address | Lose it and |
|---|---|---|
| `solve_staking-keypair.json` | `GyCrZ9JQ…` | the program cannot be deployed or upgraded at its reserved address |
| `funding_mainnet-keypair.json` | `TUgoFCpH…` | the pool can never be funded again — the address is compiled in and there is no instruction to change it. The seed phrase on paper is the backup |
| `funding_authority-keypair.json` | `ACbZ5vaj…` | nothing: since 2026-08-28 this key is devnet only, plus the devnet mint authority |
| `devnet_mint-keypair.json` | `9dybdAGg…` | only the throwaway devnet mint is gone; rebuild and remint |

🔴 **The funding authority is also the upgrade authority** on the devnet
deployment — one file both tops the pool up and can replace the program. It was
generated in a chat session on a laptop, which is fine for devnet and is not
fine for mainnet. Split the two roles onto separate keys, and put at least the
funding one behind hardware or a Squads multisig, before a mainnet build.

Until 2026-08-25 the funding key lived alone inside
`contracts/bounty-escrow/.solana-config/id.json` — another project's config
directory, where a tidy-up would have destroyed it without anyone noticing what
was lost. It is now beside the other two as well.

🔴 **Before any build that will be deployed**, replace `FUNDING_AUTHORITY` in
`src/lib.rs` with the owner's durable wallet or multisig. The address is
compiled into the program and has no rotation instruction: losing that key
means the pool can never be funded again, while staked principal stays locked
behind its own timers. The value currently in the source is a development key
under ignored `.keys/`.

## Fixed policy

- **Rewards are split in plain proportion to the staked amount.** Twice the
  stake earns exactly twice the reward. There is no lock multiplier and no
  tier: nothing can change that ratio.
- Each `fund` is spread over up to seven days. Up to, not exactly: a later
  pour re-spreads whatever is still undelivered, and time when nobody is staked
  carries forward instead of being emitted. This is a smoothing window, not a
  calendar epoch.
- **Funding is allowed at any time, including into an empty pool.** Whatever
  the previous pour has not paid out yet is folded into the new rate rather
  than stranded, and emission with nobody to pay accumulates in `rollover` for
  whoever stakes next. So the owner can fund on the rhythm at which creator
  fees actually arrive, and no staker can make a funding transaction fail by
  leaving in front of it.
- **A pour has a floor of `100_000 SOLVE`, the same as a stake.** Every pour
  re-spreads the undelivered remainder over a fresh window, so a stream of dust
  pours would drag the schedule out — 63% delivered after a week instead of all
  of it, 99% after five. Nothing is lost either way and the sum still converges
  on everything funded, but the schedule should not be free for the funding
  authority to move.
- Only the wallet compiled in as funding authority may initialize the pool and
  fund it; nobody can withdraw funded rewards directly.
- One position per wallet.
- **Stake must sit for three days before it can leave.** After that the wallet
  may hold or exit at any moment. Topping up restarts the three days for the
  whole position, otherwise the hold could be dodged by seeding a position
  once and adding to it later.
- Minimum contribution is `100_000 SOLVE` (`100_000_000_000` base units) per
  stake call — chosen so a position is worth far more than the rent its
  account costs.
- Rewards accrue continuously, per second, using global reward-per-weight
  accounting. A wallet earns only for the time it was actually staked.
- Principal and rewards use separate Token-2022 vaults controlled by the pool
  PDA. There is no admin withdrawal or pause instruction.

The owner buys SOLVE using 15% of creator fees, then funds the pool with the
exact token amount received by that swap. No fixed APR is promised: the pool
is fixed and divides among whoever is present, so the rate falls as
participation grows.

## Compact instruction format

| Tag | Instruction | Data after tag | Accounts |
|---:|---|---|---|
| `0` | initialize | none | payer, pool, principal vault, reward vault, mint, system, token |
| `1` | fund | `amount: u64` | funder, source, pool, reward vault, mint, token |
| `2` | stake | `amount: u64` | owner, source, pool, position, principal vault, mint, system, token |
| `3` | claim | none | owner, destination, pool, position, reward vault, mint, token |
| `4` | unstake all | none | owner, destination, pool, position, principal vault, mint, token |
| `5` | close position | none | owner, position |

This is a native Solana program rather than Anchor. It uses fixed-width state
and no serialization framework to reduce the `.so` size and refundable program
rent. Standard Token-2022 instructions still perform every token transfer.

### Program addresses

Everything the program owns is derived, so any observer can reproduce the
addresses from the program id alone:

| Account | Seeds |
|---|---|
| pool state | `"pool"` |
| principal vault | `"vault_principal"` |
| reward vault | `"vault_reward"` |
| staker position | `"position"`, staker pubkey |

`initialize` creates both vaults itself instead of accepting caller-supplied
token accounts. That is deliberate: a supplied account could already carry a
delegate or a close authority able to drain the vault, and the program has no
instruction that could undo it. Because the program creates them and never
issues `Approve` or `SetAuthority`, neither can ever be set.

### Error codes

`ProgramError::Custom(n)`:

| n | Meaning |
|---:|---|
| 1 | malformed instruction data |
| 2 | wrong account, wrong owner, or wrong derived address |
| 3 | account already initialized |
| 4 | a stake below the `100_000 SOLVE` minimum, or a pour below the same floor |
| 5 | retired. Was "funding a pool that holds no stake"; funding an empty pool is allowed now and the number stays reserved so the codes below keep their meaning |
| 6 | nothing accrued to claim |
| 7 | still inside the three-day hold, or nothing staked |
| 8 | closing a position that still holds stake or unclaimed rewards |
| 9 | arithmetic overflow |

## Build and exact deployment deposit

Docker Desktop must be running:

```powershell
./scripts/build.ps1
```

The script prints the compiled byte size and rent for the upgradeable
program-data account (`.so` plus its 45-byte loader header). Deployment also
uses a similarly sized temporary buffer. Close that buffer after a successful
deploy to reclaim its rent; transaction fees and the small executable program
account remain additional costs.

## Safety invariants

- The mint is compiled into the program and cannot be changed.
- Vault addresses are program-derived, created by the program, and pinned in
  pool state; they can carry neither a delegate nor a close authority.
- Only a position owner may claim, unstake, or close it.
- Principal cannot leave before the position's unlock timestamp.
- Funding early cannot strand the undelivered part of the previous pour: it is
  folded into the new rate, and those tokens are already in the vault, so the
  fold redistributes rather than promising anything new.
- Rewards emitted while the pool is empty roll into the following pour.
- All reward arithmetic uses checked `u128`; token amounts remain `u64`.
- Rounding always truncates in the vault's favour, so total payouts can never
  exceed the total funded. `tests::payouts_never_exceed_what_was_funded`
  asserts this after every step of a 4,000-step randomized walk. The flip side,
  stated so nobody reads "nothing is lost" too widely: the truncated remainders
  themselves stay in the reward vault for good. They are dust — under one base
  unit per position per settlement — and there is no sweep instruction for the
  same reason there is no withdrawal instruction.
- `fund` never trusts a timestamp older than the pool's last update. A clock
  that ran backwards would otherwise let the leftover fold count seconds that
  were already distributed, and promise more than was funded.
- Neither vault can be named as a destination of `claim` or `unstake`. The
  principal vault would be a self-transfer the token program accepts as a
  no-op, zeroing the position while the tokens never moved; the reward vault
  would put them where no instruction can pay them out.

### Sending tokens straight to a vault destroys them

Both vaults are ordinary Token-2022 accounts at published program addresses, so
anyone can transfer SOLVE into either one. Nothing will happen. Reward accounting
follows the emission rate, not the vault balance, so a donation to the reward
vault is never distributed, and a transfer to the principal vault belongs to no
position. There is no sweep instruction and there will not be one: any such
instruction is an admin withdrawal, and the absence of admin withdrawals is the
property this program exists to have.

The same applies to naming a vault as the destination of `claim` or `unstake`.
That the program now refuses, since it has both vault addresses to hand and no
legitimate call ever names one.

### Known and accepted limitations

- **Topping up restarts the whole hold.** Adding to a position sets
  `unlock_at = now + 3 days` for the entire balance. Any interface must warn
  about this before the transaction is signed.
- **Unstaking is all-or-nothing.** There is no partial withdrawal.
- **A staker's share falls as others join.** The pour is fixed, so the rate is
  whatever that pour divided by the total staked comes to. An interface must
  show the live figure rather than any promised percentage.
- **Reward dust is not recoverable.** Truncation remainders and any
  `rollover_scaled` left when funding stops permanently stay in the reward
  vault. Recovering them would require an admin withdrawal instruction, which
  would destroy the property that nobody can take tokens out of the vaults.
  Tokens sent directly to a vault are lost for the same reason: the emission
  rate comes from the `fund` argument, not from the vault balance.
- **Funding needs existing stake.** The first pour can only be made after
  someone has staked, and funding is blocked again if every staker exits.

The SOLVE mint was checked on mainnet on 2026-08-23: six decimals, Token-2022
metadata extensions only, and no transfer fee, transfer hook, freeze authority,
or mint authority. Mint decimals and mint extensions are fixed at
initialization, which is why `SOLVE_DECIMALS` and the 165-byte `VAULT_LEN` are
safe as compile-time constants.

## Cost

Solana rent is `(128 + size) × 6960` lamports. With fixed account sizes that
gives, independent of any particular build:

- pool state, 149 bytes: `0.00192792 SOL`;
- each staker position, 77 bytes: `0.00142680 SOL`, paid by that staker and
  refunded to them by instruction `5`;
- two 165-byte Token-2022 vaults: `0.00407856 SOL` total;
- the executable program account: about `0.00114144 SOL`.

The program-data rent depends on the compiled size. Measured with Agave CLI
4.2.1 and platform-tools 1.54 on 2026-08-24:

- program binary: `107,416` bytes;
- SHA-256: `f702dd866a0af820ed5ad8c31658cf0d06238f8b5d4fd11dfa1549d8a3f3a93b`;
- upgradeable program-data rent: `0.74881944 SOL` (binary + 45-byte header).

Permanent refundable balance is therefore `0.75596736 SOL`. Deployment also
needs a temporary buffer of the same size as the program data, so the wallet
should hold about `1.51 SOL` plus fees; closing the buffer after a successful
deploy returns `0.7488 SOL`.

Dropping the lock tiers took only `1,024` bytes off the binary, which says
where the size actually lives: not in this program's logic but in the
formatting machinery behind `msg!` and the bincode helpers used to build
system instructions. Anything that would meaningfully shrink the deployment
deposit has to come from there, not from trimming policy.

🔴 **The target architecture must be passed explicitly.** `cargo-build-sbf`
still defaults to `v0`, and an Agave 4.2.1 validator refuses to deploy a v0
executable — "Detected sbpf_version required by the executable which are not
enabled", surfacing as a `BPFLoaderUpgradeab1e` failure that reads like a
corrupt binary. `scripts/build.ps1` therefore takes `-Arch`, defaulting to
`v3`. Confirm the target cluster enables the chosen version before deploying
to it.

Building for `v3` is also **smaller**: `99,624` bytes against `107,696` for the
same source at `v0`, about `0.056 SOL` less program-data rent.

🔴 Byte-for-byte reproducibility has **not** been re-verified since the
2026-08-24 rewrite: the recorded hash comes from a single build. Rebuild after
wiping the build directory and confirm the hash matches before publishing it.

Publish the recorded size and SHA-256 alongside the build recipe so anyone can
rebuild and compare against the deployed bytes.

Platform-tools prints stack-frame diagnostics while compiling unused
transitive cryptography generics from `solana-program`, then links the program
successfully. Those functions are never reached from any instruction, but it
is one more reason to require a full local-validator test before deployment.

## Testing status

Twenty-three tests, all passing as of 2026-08-24.

Twelve unit tests cover state serialization, plain proportional splitting,
window arithmetic, the leftover fold when funding early, empty-pool rollover,
the minimum-stake bound on the reward accumulator, and the solvency invariant
— the last being a 4,000-step randomized walk that asserts, after every step,
that claimed plus outstanding never exceeds funded.

Eleven integration tests run the compiled SBF binary against the real
Token-2022 program and the real SOLVE mint account, covering what arithmetic
cannot reach: instruction dispatch, pool and vault creation, the full
lifecycle of two differently sized stakers across two fundings with claims,
exit and rent refund, and every rejection path — a stranger initializing or
funding, funding an empty pool, dust below the minimum, leaving inside the
three-day hold, and reaching for another wallet's position or a substituted
vault or pool account. Two of them pin the behaviour this design turns on:
funding mid-window must keep the undelivered remainder, and a top-up must
restart the hold for the whole position.

The heaviest instruction, `unstake`, consumed 19,223 of the 200,000 compute
unit budget in those runs.

The suite proves the vaults come out clean: after `initialize`, both are
165-byte Token-2022 accounts owned by the pool PDA with the delegate and
close-authority slots empty.

```bash
docker build -f Dockerfile.build -t solve-staking-build:latest .
docker run --rm -v "$PWD:/work" -w /work -e SBF_OUT_DIR=/work/target/deploy   solve-staking-build:latest sh -c '
    cargo-build-sbf --features test-authority &&
    cargo +stable test --features test-authority'
```

Two toolchains, on purpose: `cargo-build-sbf` pins rustc 1.86.0 through
platform-tools, while `solana-program-test` and its dependency tree need a
current compiler. `Dockerfile.build` installs both. The production binary builds on 1.86;
the tests and `cargo clippy --all-targets` use `+stable`, since pulling in the
dev-dependencies raises the minimum compiler. On 1.86, lint the library alone:
`cargo clippy --lib -- -D warnings`.

`--features test-authority` swaps the funding authority for a publicly known
keypair so the suite can sign as it. Such a binary logs
`BUILT-WITH-TEST-AUTHORITY-DO-NOT-DEPLOY` on every `initialize`, and
`scripts/build.ps1` deletes any binary containing that marker rather than hand
it over. Build without features for anything that will reach a cluster.

Fixtures in `tests/fixtures/`, both taken from mainnet on 2026-08-23:

| File | What | SHA-256 |
|---|---|---|
| `solve_mint.bin` | the 412-byte SOLVE mint account | `b9161351921b487fbe76a9ba7e351e1cc3406cc0af0fa8e4a1619a45d3fa7945` |
| `spl_token_2022.so` | the deployed Token-2022 program | `0999dbf708971e723b08d1caafc988826a59c6001ed6dc02260da07defbe1469` |

Refresh them with:

```bash
solana program dump -u m TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb tests/fixtures/spl_token_2022.so
```

### Exercised on a real validator, 2026-08-24

The program was deployed to a local Agave 4.2.1 validator carrying the genuine
mainnet SOLVE mint and Token-2022, and driven end to end through `client/`
using the `fast-clock` build, which shortens the hold to three minutes and the
window to seven. Every instruction was sent by an independent client rather
than by the same Rust that asserts on it, which is the gap `cargo test` cannot
close.

- rewards accrued at `0.214285 SOLVE/s` from a 90 SOLVE pour over seven
  minutes, and the off-chain figure in `show` matched the chain second by
  second;
- funding again mid-window was accepted and folded the undelivered remainder:
  vault held `166.500001` after two 90 pours and one `13.499999` claim, and the
  rate roughly doubled;
- a top-up moved the hold from two minutes left back to three, and unstaking
  inside it was refused with `0x7`;
- after unstake, claim and close: the position account was gone and its rent
  returned, both vaults were still 165 bytes with empty delegate and close
  authority slots;
- **conservation held exactly** — `505000.000000` base units in, the same out
  across the two wallets and two vaults, with `84.903063` correctly left as
  rollover because the only staker exited mid-window.

What this did not cover: the production timings, which only unit tests pin, and
a public cluster.

### Exercised on public devnet, 2026-08-24

Deployed to devnet at the reserved program id with `devnet-mint,fast-clock` and
driven through the same lifecycle. Conservation again held exactly:
`505000.000000` base units across both wallets and both vaults, the position
account was closed with its rent returned, and both vaults finished as 165-byte
Token-2022 accounts with empty delegate and close-authority slots.

Three things a local validator did not show:

- **The first deploy failed** with `Data writes to account failed: Custom
  error: Max retries exceeded`, leaving an orphaned buffer holding
  `0.69508824 SOL`. `--use-rpc` (send over RPC rather than TPU) got it through
  on the retry, and `solana program close <buffer>` returned the lamports.
  Budget for a retry: the run consumed about `0.81 SOL` of the 5 it started
  with, most of it program-data rent.
- **The public RPC drops requests routinely** — `initialize` only landed on the
  second attempt, with `fetch failed` on the first.
- **Blind retries are dangerous around money.** A shell loop retrying `fund`
  because it had not seen the confirmation sent a second pour that also
  succeeded, doubling the epoch. Harmless in a test, not on mainnet: check
  whether the previous transaction landed before resending anything that moves
  tokens. The client itself does not retry; this was the operator's loop.

Still missing before mainnet: a run at the production timings, which only unit
tests currently pin.
The integration suite warps the clock, which is the only way to cross a
seven-day window in a test, but it cannot catch anything that depends on real
validator behaviour.

Do not use `--final` for the first release: retaining upgrade authority is the
practical way to fix a defect discovered during devnet and limited mainnet
testing.
