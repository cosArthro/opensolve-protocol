# solve-staking client

A small CLI for driving the staking program against a local validator or
devnet. No IDL, no indexer, no backend: every address is derived from the
program id and both state accounts are fixed-width structs, so the whole thing
is one file.

```bash
npm install
node solve-staking.mjs help
```

`show` reproduces the on-chain reward arithmetic off-chain. That is the piece a
web front end has to copy — the stored `accrued` field only moves when a
transaction touches the position, so anything that reads it alone displays a
figure that is stale between actions. `rewardPerWeightAt` and `earnedAt` in
`solve-staking.mjs` are the whole of it, about ten lines.

> Node 20 note: a transitive dependency of `@solana/web3.js` resolves `uuid` to
> an ESM-only release that Node 20 cannot `require()`. `package.json` pins it
> back to the CommonJS line through `overrides`. Remove that pin once the
> machine runs a newer Node.

## Driving a full lifecycle in twenty minutes

The production hold is three days and the smoothing window seven, which makes
a real-time test a three-day affair. The `fast-clock` build feature shrinks
them to three and seven **minutes**. Nothing in the program reads how long
either is — it only compares `now` against `unlock_at` and divides by the
window — so the exercised code path is identical.

A binary built that way logs `BUILT-WITH-FAST-CLOCK-DO-NOT-DEPLOY` on every
`initialize`, and `scripts/build.ps1` deletes any binary carrying a
`BUILT-WITH-*` marker rather than hand it over.

### 1. Build with the shortened clock

```bash
docker run --rm -v "/c/Proof of Meaning/.claude/worktrees/solve-staking/contracts/solve-staking:/work" \
  -w /work -e SBF_OUT_DIR=/work/target/deploy solve-staking-build:latest \
  cargo-build-sbf --manifest-path /work/Cargo.toml --features fast-clock
```

### 2. Mint nothing — the mint has no authority

SOLVE has no mint authority, so no one can create SOLVE anywhere, local
validator included. Hand the validator pre-made token accounts instead:

```bash
node solve-staking.mjs fixture <ALICE_PUBKEY> 500000 --out alice.json
node solve-staking.mjs fixture <AUTHORITY_PUBKEY> 5000 --out authority.json
```

### 3. Start a validator carrying the real mint

Cloned from mainnet, so the test runs against the genuine six-decimal
metadata-only mint rather than a stand-in. Token-2022 comes from the fixture
already in the repo, so this half works offline.

```bash
solana-test-validator --reset \
  --url mainnet-beta \
  --clone GwyWFsDKW9a2ref1EWqdUS7B37Toii433zrAh9Dipump \
  --bpf-program TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb ../tests/fixtures/spl_token_2022.so \
  --account <ALICE_ATA> alice.json \
  --account <AUTHORITY_ATA> authority.json
```

`node solve-staking.mjs addresses <PUBKEY>` prints the ATA for each wallet.

### 4. Deploy

```bash
solana airdrop 100 <AUTHORITY_PUBKEY> --url localhost
solana program deploy --url localhost \
  --program-id ../.keys/solve_staking-keypair.json \
  ../target/deploy/solve_staking.so
```

### 5. Walk the lifecycle

`--keypair` for the funding authority is the file whose public key is
`ACbZ5vajyFFseZoYTdrzcxLSJbnnf4pt3MQoZ7XtDrws`.

```bash
A="--url http://127.0.0.1:8899 --keypair /path/to/authority.json"
S="--url http://127.0.0.1:8899 --keypair /path/to/alice.json"

node solve-staking.mjs initialize $A          # pool + both vaults
node solve-staking.mjs stake 100000 $S        # the minimum stake
node solve-staking.mjs fund 90 $A             # spread over seven minutes
node solve-staking.mjs show $S                # watch it tick, run it again
node solve-staking.mjs claim $S               # take what accrued

# Funding again before the window runs out must keep the undelivered part.
# --force because this repeats an amount funded minutes ago, and refusing that
# by default is the point of the guard; see "Funding twice" below.
node solve-staking.mjs fund 90 $A --force
node solve-staking.mjs show $S

# A top-up restarts the hold on the whole position.
node solve-staking.mjs stake 100000 $S
node solve-staking.mjs unstake $S             # refused: error 7, still held

# Three minutes later.
node solve-staking.mjs unstake $S
node solve-staking.mjs claim $S
node solve-staking.mjs close $S               # rent comes back
```

What this catches that `cargo test` cannot: instruction encoding and account
ordering built by a real client, deployment mechanics, real fees and blockhash
expiry, and the compute budget under a real runtime.

## Devnet

Same flow with `--url https://api.devnet.solana.com`, with one difference: the
mainnet SOLVE mint **cannot** be recreated there. Its address is a pump.fun
vanity keypair nobody outside pump.fun holds, and an account can only be
created at an address whose key you can sign with. Cloning works on a local
validator because the validator writes account state directly; devnet has no
such door.

So devnet needs its own Token-2022 mint — six decimals, no extensions, so the
compiled-in `SOLVE_DECIMALS` and 165-byte `VAULT_LEN` stay correct. That is the
`devnet-mint` feature, whose address is
`9dybdAGgG1w4yZS4oBzgY424pxHscQFQkWr9qobRQvFH`; its keypair sits beside the
program keypair under `.keys/` and must be backed up with it. The address is
compiled in, so the mint keypair has to exist **before** the build.

```bash
# 1. build (both features: test mint, and minutes instead of days)
docker run --rm -v "<contract>:/work" -w /work -e SBF_OUT_DIR=/work/target/deploy \
  solve-staking-build:latest \
  cargo-build-sbf --manifest-path /work/Cargo.toml --features devnet-mint,fast-clock --arch v3

# 2. fund the funding authority on devnet — see the note below
# 3. create the mint and hand out test tokens
D="--url https://api.devnet.solana.com --mint 9dybdAGgG1w4yZS4oBzgY424pxHscQFQkWr9qobRQvFH"
node solve-staking.mjs create-mint $D --keypair <authority.json> --mint-keypair ../.keys/devnet_mint-keypair.json
node solve-staking.mjs mint-to <AUTHORITY> 5000   $D --keypair <authority.json>
node solve-staking.mjs mint-to <STAKER>    500000 $D --keypair <authority.json>

# 4. deploy, then walk the same lifecycle as above with $D
solana program deploy --url devnet --program-id ../.keys/solve_staking-keypair.json \
  ../target/deploy/solve_staking.so
```

`create-mint` refuses to run if `--mint-keypair` and `--mint` disagree, because
the program only accepts the address compiled into it.

> **Getting devnet SOL is the awkward part.** Deployment needs about `1.4 SOL`
> — roughly `0.695` for program data and the same again for the temporary
> buffer, returned when it is closed. The `requestAirdrop` RPC faucet is
> throttled per IP and commonly answers `429` or `Internal error`; alternative
> public endpoints want an API key. The reliable route is
> <https://faucet.solana.com> signed in with GitHub, which needs a human, or a
> transfer from another devnet wallet.

## Funding twice

`fund` refuses an identical amount funded against the same pool in the last
fifteen minutes, and exits without sending anything.

This is not theoretical. During the devnet run on 2026-08-24 the first `fund`
landed, its confirmation was slow to come back, the command was repeated, and
the pool was topped up twice — 27 seconds apart, both visible on chain:

```
node solve-staking.mjs recent-funds 604800 --url https://api.devnet.solana.com
              90 SOLVE  81354s ago  XGt7fNbc...
              90 SOLVE  81381s ago  RFw873eD...
```

On devnet that cost nothing. On mainnet it is a week of rewards paid twice, and
it cannot be reversed: the program has no withdrawal instruction, which is the
property that makes it trustworthy everywhere else.

So, after a timeout — **do not re-run the fund.** Ask instead:

```bash
node solve-staking.mjs recent-funds        # needs no key, reads only
```

A timeout from `send` means the confirmation did not arrive in 45 seconds. It
does not mean the transaction failed, and it may still land afterwards.

| Flag | Effect |
|---|---|
| `--force` | send anyway — for a genuine second top-up |
| `--window <seconds>` | how far back to look; default 900 |

A different amount is never blocked; it only prints a note naming the last fund.
