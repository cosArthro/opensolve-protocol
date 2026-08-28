#!/usr/bin/env node
// Minimal client for the SOLVE staking program.
//
// Everything the program owns is derived from the program id, and both state
// accounts are plain fixed-width structs, so this needs no IDL, no indexer and
// no backend. `show` reproduces the on-chain reward arithmetic off-chain,
// which is exactly what a web front end has to do to display a live figure.
//
//   node solve-staking.mjs show
//   node solve-staking.mjs stake 100000 --keypair ~/alice.json
//
// See README.md in this directory.

import { readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';

// ---------------------------------------------------------------- constants

const DEFAULT_PROGRAM = 'GyCrZ9JQq1LAWJ3fWqn52uEnAFUmg52NHQvcatq2mcDe';
const DEFAULT_MINT = 'GwyWFsDKW9a2ref1EWqdUS7B37Toii433zrAh9Dipump';
const TOKEN_2022 = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const CLOCK = new PublicKey('SysvarC1ock11111111111111111111111111111111');

const DECIMALS = 6;
const SCALE = 10n ** 18n;
const POOL_LEN = 149;
const POSITION_LEN = 77;

// ------------------------------------------------------------------- helpers

const u64 = (buf, o) => buf.readBigUInt64LE(o);
const i64 = (buf, o) => buf.readBigInt64LE(o);
const u128 = (buf, o) => u64(buf, o) | (u64(buf, o + 8) << 64n);

/** Base units -> a human string with the mint's six decimals. */
function fmt(base) {
  const n = BigInt(base);
  const sign = n < 0n ? '-' : '';
  const a = n < 0n ? -n : n;
  const unit = 10n ** BigInt(DECIMALS);
  const frac = (a % unit).toString().padStart(DECIMALS, '0').replace(/0+$/, '');
  const whole = (a / unit).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
  return `${sign}${whole}${frac ? '.' + frac : ''} SOLVE`;
}

/** A human amount ("100000" or "100000.5") -> base units. */
function parseAmount(text) {
  if (!/^\d+(\.\d+)?$/.test(text ?? '')) {
    throw new Error(`amount must be a positive number, got ${JSON.stringify(text)}`);
  }
  const [whole, frac = ''] = text.split('.');
  if (frac.length > DECIMALS) {
    throw new Error(`SOLVE has ${DECIMALS} decimals, ${text} has ${frac.length}`);
  }
  return BigInt(whole) * 10n ** BigInt(DECIMALS) + BigInt(frac.padEnd(DECIMALS, '0') || '0');
}

const le64 = (v) => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(v));
  return b;
};

function loadKeypair(path) {
  const resolved = path.startsWith('~') ? path.replace('~', homedir()) : path;
  const raw = JSON.parse(readFileSync(resolved, 'utf8'));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

/**
 * Flags that never take a value. Without this list `--help` swallowed whatever
 * followed it and stored `undefined` when nothing did, so `flags.help !==
 * undefined` was false and `--help` printed nothing.
 */
const BOOLEAN_FLAGS = new Set(['help', 'force']);

function parseArgs(argv) {
  const positional = [];
  const flags = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith('--')) {
      const name = argv[i].slice(2);
      flags[name] = BOOLEAN_FLAGS.has(name) ? true : argv[++i];
    } else positional.push(argv[i]);
  }
  return { positional, flags };
}

// ------------------------------------------------------------------ addresses

class Program {
  constructor(programId, mint) {
    this.id = new PublicKey(programId);
    this.mint = new PublicKey(mint);
  }
  #pda(seeds) {
    return PublicKey.findProgramAddressSync(seeds, this.id)[0];
  }
  get pool() {
    return this.#pda([Buffer.from('pool')]);
  }
  get principalVault() {
    return this.#pda([Buffer.from('vault_principal')]);
  }
  get rewardVault() {
    return this.#pda([Buffer.from('vault_reward')]);
  }
  position(owner) {
    return this.#pda([Buffer.from('position'), owner.toBuffer()]);
  }
  /** The staker's own SOLVE account. Not program-owned; just a convention. */
  ata(owner) {
    return PublicKey.findProgramAddressSync(
      [owner.toBuffer(), TOKEN_2022.toBuffer(), this.mint.toBuffer()],
      ATA_PROGRAM,
    )[0];
  }
}

// --------------------------------------------------------------- instructions

const rw = (pubkey, isSigner = false) => ({ pubkey, isSigner, isWritable: true });
const ro = (pubkey) => ({ pubkey, isSigner: false, isWritable: false });

function ixInitialize(p, payer) {
  return new TransactionInstruction({
    programId: p.id,
    keys: [
      rw(payer, true),
      rw(p.pool),
      rw(p.principalVault),
      rw(p.rewardVault),
      ro(p.mint),
      ro(SystemProgram.programId),
      ro(TOKEN_2022),
    ],
    data: Buffer.from([0]),
  });
}

function ixFund(p, funder, source, amount) {
  return new TransactionInstruction({
    programId: p.id,
    keys: [
      rw(funder, true),
      rw(source),
      rw(p.pool),
      rw(p.rewardVault),
      ro(p.mint),
      ro(TOKEN_2022),
    ],
    data: Buffer.concat([Buffer.from([1]), le64(amount)]),
  });
}

function ixStake(p, owner, source, amount) {
  return new TransactionInstruction({
    programId: p.id,
    keys: [
      rw(owner, true),
      rw(source),
      rw(p.pool),
      rw(p.position(owner)),
      rw(p.principalVault),
      ro(p.mint),
      ro(SystemProgram.programId),
      ro(TOKEN_2022),
    ],
    data: Buffer.concat([Buffer.from([2]), le64(amount)]),
  });
}

function ixClaim(p, owner, destination) {
  return new TransactionInstruction({
    programId: p.id,
    keys: [
      rw(owner, true),
      rw(destination),
      rw(p.pool),
      rw(p.position(owner)),
      rw(p.rewardVault),
      ro(p.mint),
      ro(TOKEN_2022),
    ],
    data: Buffer.from([3]),
  });
}

function ixUnstake(p, owner, destination) {
  return new TransactionInstruction({
    programId: p.id,
    keys: [
      rw(owner, true),
      rw(destination),
      rw(p.pool),
      rw(p.position(owner)),
      rw(p.principalVault),
      ro(p.mint),
      ro(TOKEN_2022),
    ],
    data: Buffer.from([4]),
  });
}

function ixClose(p, owner) {
  return new TransactionInstruction({
    programId: p.id,
    keys: [rw(owner, true), rw(p.position(owner))],
    data: Buffer.from([5]),
  });
}

/**
 * Token-2022 `InitializeMint2` on a freshly allocated 82-byte account. Six
 * decimals because `SOLVE_DECIMALS` is compiled into the program, and no
 * extensions at all so a vault stays the 165 bytes `VAULT_LEN` assumes. No
 * freeze authority, matching the real mint.
 */
function ixCreateMint(p, payer, mintAuthority, rentLamports) {
  const allocate = SystemProgram.createAccount({
    fromPubkey: payer,
    newAccountPubkey: p.mint,
    lamports: rentLamports,
    space: 82,
    programId: TOKEN_2022,
  });
  const data = Buffer.concat([
    Buffer.from([20]), // InitializeMint2
    Buffer.from([DECIMALS]),
    mintAuthority.toBuffer(),
    Buffer.from([0]), // freeze authority: None
  ]);
  return [allocate, new TransactionInstruction({ programId: TOKEN_2022, keys: [rw(p.mint)], data })];
}

function ixMintTo(p, destination, authority, amount) {
  return new TransactionInstruction({
    programId: TOKEN_2022,
    keys: [rw(p.mint), rw(destination), { pubkey: authority, isSigner: true, isWritable: false }],
    data: Buffer.concat([Buffer.from([7]), le64(amount)]), // MintTo
  });
}

function ixCreateAta(p, payer, owner) {
  return new TransactionInstruction({
    programId: ATA_PROGRAM,
    keys: [
      rw(payer, true),
      rw(p.ata(owner)),
      ro(owner),
      ro(p.mint),
      ro(SystemProgram.programId),
      ro(TOKEN_2022),
    ],
    data: Buffer.from([0]),
  });
}

// ------------------------------------------------------------------ fixtures

/**
 * The SOLVE mint has no mint authority, so nobody can create SOLVE out of thin
 * air — not even on a local validator. To give a test wallet a balance, hand
 * the validator a pre-made token account with `--account <address> <file>`.
 *
 * 165 bytes with no extensions is a valid Token-2022 account for this mint,
 * whose own extensions are metadata only. Delegate and close authority stay
 * zeroed, matching what `initialize` produces for the vaults.
 */
function tokenAccountFixture(p, owner, amount) {
  const data = Buffer.alloc(165);
  p.mint.toBuffer().copy(data, 0);
  owner.toBuffer().copy(data, 32);
  data.writeBigUInt64LE(BigInt(amount), 64);
  data[108] = 1; // AccountState::Initialized
  return {
    pubkey: p.ata(owner).toBase58(),
    account: {
      lamports: 2_039_280,
      data: [data.toString('base64'), 'base64'],
      owner: TOKEN_2022.toBase58(),
      executable: false,
      // Deliberately 0 rather than u64::MAX: the field deserializes as a
      // number, and u64::MAX cannot survive a round trip through JSON's
      // double. Rent exemption comes from the lamports, not from this.
      rentEpoch: 0,
    },
  };
}

// -------------------------------------------------------------- state reading

function decodePool(data) {
  if (data.length !== POOL_LEN || data.subarray(0, 4).toString() !== 'SLVP') {
    throw new Error('not a staking pool account');
  }
  return {
    bump: data[4],
    principalVault: new PublicKey(data.subarray(5, 37)),
    rewardVault: new PublicKey(data.subarray(37, 69)),
    totalWeight: u128(data, 69),
    rewardRateScaled: u128(data, 85),
    rewardPerWeight: u128(data, 101),
    rolloverScaled: u128(data, 117),
    lastUpdate: i64(data, 133),
    periodFinish: i64(data, 141),
  };
}

function decodePosition(data) {
  if (data.length !== POSITION_LEN || data.subarray(0, 4).toString() !== 'SLVS') {
    throw new Error('not a staking position account');
  }
  return {
    bump: data[4],
    owner: new PublicKey(data.subarray(5, 37)),
    amount: u64(data, 37),
    rewardPerWeightPaid: u128(data, 45),
    accrued: u64(data, 61),
    unlockAt: i64(data, 69),
  };
}

/**
 * What `Pool::update` would compute at `now`, without sending a transaction.
 * The stored `accrued` only moves when a transaction touches the position, so
 * a caller that reads the field alone shows a figure that is stale between
 * actions. This is the part a front end must reproduce.
 */
function rewardPerWeightAt(pool, now) {
  const applicable = now < pool.periodFinish ? now : pool.periodFinish;
  if (applicable <= pool.lastUpdate || pool.totalWeight === 0n) return pool.rewardPerWeight;
  const elapsed = BigInt(applicable - pool.lastUpdate);
  return pool.rewardPerWeight + (elapsed * pool.rewardRateScaled) / pool.totalWeight;
}

function earnedAt(pool, position, now) {
  const rpw = rewardPerWeightAt(pool, now);
  return position.accrued + (position.amount * (rpw - position.rewardPerWeightPaid)) / SCALE;
}

/** The validator's clock, not this machine's. */
async function chainNow(connection) {
  const account = await connection.getAccountInfo(CLOCK);
  if (!account) throw new Error('clock sysvar unavailable');
  return i64(account.data, 32);
}

// -------------------------------------------------------------------- actions

/**
 * Sends and waits by polling `getSignatureStatuses`, not by subscribing.
 * web3.js's `sendAndConfirmTransaction` opens an RPC websocket, which is a
 * second port and is often unreachable — a container with only the HTTP port
 * published, a proxy, a locked-down network. When it cannot connect it simply
 * never returns, which reads as the program hanging rather than as a missing
 * port. Polling costs a few extra round trips and always terminates.
 */
async function send(connection, instructions, signers) {
  const list = Array.isArray(signers) ? signers : [signers];
  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash('confirmed');
  const tx = new Transaction({
    feePayer: list[0].publicKey,
    blockhash,
    lastValidBlockHeight,
  }).add(...(Array.isArray(instructions) ? instructions : [instructions]));
  tx.sign(...list);

  const sig = await connection.sendRawTransaction(tx.serialize(), {
    preflightCommitment: 'confirmed',
  });
  for (let attempt = 0; attempt < 90; attempt++) {
    const { value } = await connection.getSignatureStatuses([sig]);
    const status = value[0];
    if (status?.err) {
      throw Object.assign(new Error(`transaction failed: ${JSON.stringify(status.err)}`), {
        logs: (await connection.getTransaction(sig, { commitment: 'confirmed' }))?.meta?.logMessages,
      });
    }
    if (status?.confirmationStatus === 'confirmed' || status?.confirmationStatus === 'finalized') {
      console.log(`ok  ${sig}`);
      return sig;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  // Not "it failed" — the transaction may well land after this returns. The
  // devnet double-fund started with a message like this being read as failure.
  throw new Error(
    `timed out waiting for ${sig} to confirm.\n` +
      '  This does NOT mean it failed. Check the signature before doing anything\n' +
      '  again — for money-moving commands, repeating a transaction that landed\n' +
      '  cannot be undone.',
  );
}

const BASE58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

/** Instruction data comes back base58 whatever encoding you ask for. */
function fromBase58(text) {
  let value = 0n;
  for (const character of text) {
    const index = BASE58.indexOf(character);
    if (index < 0) throw new Error(`not base58: ${text}`);
    value = value * 58n + BigInt(index);
  }
  const bytes = [];
  while (value > 0n) {
    bytes.unshift(Number(value & 0xffn));
    value >>= 8n;
  }
  for (const character of text) {
    if (character !== '1') break;
    bytes.unshift(0);
  }
  return Buffer.from(bytes);
}

/**
 * Funding transactions against this pool inside the last `seconds`.
 *
 * This exists because of a devnet run that funded twice. The first `fund` had
 * landed; its confirmation was slow to come back, the command was re-run, and
 * the pool was topped up a second time. On devnet that was free. In production
 * it is a week of rewards paid twice, and it cannot be undone — the program has
 * no withdrawal instruction, which is exactly the property we want everywhere
 * else.
 *
 * A read costs one round trip and the mistake costs a week, so the read wins.
 */
async function recentFunds(connection, p, seconds) {
  const signatures = await connection.getSignaturesForAddress(p.pool, { limit: 30 });
  const cutoff = Math.floor(Date.now() / 1000) - seconds;
  const found = [];
  for (const entry of signatures) {
    if (!entry.blockTime || entry.blockTime < cutoff) break; // newest first
    if (entry.err) continue;
    const tx = await connection.getTransaction(entry.signature, {
      commitment: 'confirmed',
      maxSupportedTransactionVersion: 0,
    });
    if (!tx) continue;
    const keys = tx.transaction.message.staticAccountKeys ?? tx.transaction.message.accountKeys;
    for (const ix of tx.transaction.message.compiledInstructions ?? tx.transaction.message.instructions) {
      const programId = keys[ix.programIdIndex];
      if (!programId?.equals(p.id)) continue;
      const data = Buffer.isBuffer(ix.data) ? ix.data : fromBase58(ix.data);
      if (data.length !== 9 || data[0] !== 1) continue;
      found.push({
        signature: entry.signature,
        blockTime: entry.blockTime,
        amount: data.readBigUInt64LE(1),
      });
    }
  }
  return found;
}

/**
 * Refuses a second identical `fund` unless the operator insists.
 *
 * Deliberately not automatic: two genuine top-ups of the same size in one
 * window are legitimate, and the program folds the unspent remainder in rather
 * than dropping it. Only the operator knows which case this is, so the check
 * reports what it found and stops.
 */
async function guardAgainstDoubleFund(connection, p, amount, force, windowSeconds = 900) {
  let recent;
  try {
    recent = await recentFunds(connection, p, windowSeconds);
  } catch (error) {
    console.error(`could not check for a recent fund: ${error.message}`);
    console.error('check the pool by hand before repeating a fund, or pass --force');
    if (!force) process.exit(1);
    return;
  }

  const identical = recent.filter((entry) => entry.amount === amount);
  if (!identical.length) {
    if (recent.length) {
      const last = recent[0];
      const ago = Math.floor(Date.now() / 1000) - last.blockTime;
      console.log(`note: ${fmt(last.amount)} was funded ${ago}s ago (${last.signature})`);
    }
    return;
  }

  console.error('');
  console.error(`refusing: ${fmt(amount)} was already funded in the last ${windowSeconds}s.`);
  for (const entry of identical) {
    const ago = Math.floor(Date.now() / 1000) - entry.blockTime;
    console.error(`  ${entry.signature}  ${ago}s ago`);
  }
  console.error('');
  console.error('If you are re-running after a timeout, that transaction succeeded and');
  console.error('there is nothing to repeat. Rewards cannot be taken back out of the pool.');
  console.error('If this really is a second top-up, pass --force.');
  if (!force) process.exit(1);
  console.error('--force given, sending anyway.');
}

async function tokenBalance(connection, key) {
  const account = await connection.getAccountInfo(key);
  return account ? u64(account.data, 64) : null;
}

async function show(connection, p, who) {
  const poolAccount = await connection.getAccountInfo(p.pool);
  if (!poolAccount) {
    console.log(`pool ${p.pool.toBase58()} does not exist yet — run "initialize"`);
    return;
  }
  const pool = decodePool(poolAccount.data);
  const now = await chainNow(connection);

  console.log(`program        ${p.id.toBase58()}`);
  console.log(`mint           ${p.mint.toBase58()}`);
  console.log(`pool           ${p.pool.toBase58()}`);
  console.log(`total staked   ${fmt(pool.totalWeight)}`);
  console.log(`principal vault${'  '}${fmt((await tokenBalance(connection, p.principalVault)) ?? 0n)}`);
  console.log(`reward vault   ${fmt((await tokenBalance(connection, p.rewardVault)) ?? 0n)}`);

  const remaining = Number(pool.periodFinish - now);
  if (pool.periodFinish === 0n) {
    console.log('emission       never funded');
  } else if (remaining <= 0) {
    console.log('emission       window has run out — fund to restart it');
  } else {
    // rate is scaled by 1e18; show it per day for a readable figure.
    const perDay = (pool.rewardRateScaled * 86_400n) / SCALE;
    console.log(`emission       ${fmt(perDay)}/day, ${Math.floor(remaining / 60)} min left in window`);
  }

  if (!who) return;
  const positionAccount = await connection.getAccountInfo(p.position(who));
  if (!positionAccount) {
    console.log(`\n${who.toBase58()} has no position`);
    return;
  }
  const position = decodePosition(positionAccount.data);
  const earned = earnedAt(pool, position, now);
  const share = pool.totalWeight === 0n ? 0 : Number((position.amount * 10_000n) / pool.totalWeight) / 100;
  const locked = Number(position.unlockAt - now);

  console.log(`\nposition       ${p.position(who).toBase58()}`);
  console.log(`staked         ${fmt(position.amount)}  (${share}% of the pool)`);
  console.log(`earned now     ${fmt(earned)}   <- ticks up every second`);
  console.log(`  of which settled on-chain ${fmt(position.accrued)}`);
  console.log(
    locked > 0
      ? `hold           ${Math.ceil(locked / 60)} min left before unstake is allowed`
      : 'hold           elapsed — can unstake',
  );
}

// ----------------------------------------------------------------------- main

const USAGE = `solve-staking — drive the SOLVE staking program

  addresses                 print every derived address
  show [pubkey]             pool state, and a wallet's live earnings
  fixture <pubkey> <amount> emit a pre-funded token account for the validator's
                            --account flag; the mint has no mint authority, so
                            this is the only way to give a test wallet SOLVE
  initialize                create the pool and both vaults (funding authority)
  fund <amount>             pour rewards, spread over the window (funding authority).
                            Refuses an identical fund made in the last 15 minutes;
                            override with --force when it is a real second one
  create-ata [pubkey]       create a SOLVE token account for a wallet
  create-mint               devnet only: create the throwaway test mint,
                            needs --mint-keypair matching the compiled address
  mint-to <pubkey> <amount> devnet only: mint test SOLVE to a wallet
  stake <amount>            stake, or add to an existing position
  claim                     take accrued rewards
  unstake                   withdraw the whole principal, once the hold elapsed
  close                     close an emptied position and reclaim its rent
  recent-funds [seconds]    list funds against the pool in the last N seconds
                            (default 900) — ask this after a timeout instead of
                            re-running fund

Flags
  --url <endpoint>          default http://127.0.0.1:8899
  --keypair <path>          signer; default ~/.config/solana/id.json
  --program <pubkey>        default ${DEFAULT_PROGRAM}
  --mint <pubkey>           default ${DEFAULT_MINT}
  --source <pubkey>         token account to pay from; default the signer's ATA
  --destination <pubkey>    token account to receive; default the signer's ATA
  --force                   send a fund that duplicates a very recent one
  --window <seconds>        how far back the duplicate check looks; default 900

Amounts are in SOLVE, not base units: "stake 100000" stakes one hundred
thousand SOLVE, the program's minimum.`;

async function main() {
  const { positional, flags } = parseArgs(process.argv.slice(2));
  const command = positional[0];
  if (!command || command === 'help' || flags.help) {
    console.log(USAGE);
    return;
  }

  const connection = new Connection(flags.url ?? 'http://127.0.0.1:8899', 'confirmed');
  const p = new Program(flags.program ?? DEFAULT_PROGRAM, flags.mint ?? DEFAULT_MINT);

  if (command === 'addresses') {
    console.log(`program          ${p.id.toBase58()}`);
    console.log(`mint             ${p.mint.toBase58()}`);
    console.log(`pool             ${p.pool.toBase58()}`);
    console.log(`principal vault  ${p.principalVault.toBase58()}`);
    console.log(`reward vault     ${p.rewardVault.toBase58()}`);
    if (positional[1]) {
      const who = new PublicKey(positional[1]);
      console.log(`position         ${p.position(who).toBase58()}`);
      console.log(`token account    ${p.ata(who).toBase58()}`);
    }
    return;
  }

  if (command === 'fixture') {
    // Writing to stdout keeps this usable as `... > alice-tokens.json`.
    const owner = new PublicKey(positional[1]);
    const amount = parseAmount(positional[2]);
    const fixture = tokenAccountFixture(p, owner, amount);
    if (flags.out) {
      writeFileSync(flags.out, JSON.stringify(fixture, null, 2));
      console.error(`wrote ${flags.out}: ${fmt(amount)} at ${fixture.pubkey}`);
    } else {
      console.log(JSON.stringify(fixture, null, 2));
    }
    return;
  }

  if (command === 'show') {
    let who = positional[1] ? new PublicKey(positional[1]) : null;
    if (!who && flags.keypair) who = loadKeypair(flags.keypair).publicKey;
    await show(connection, p, who);
    return;
  }

  if (command === 'recent-funds') {
    // The question to ask after a timeout, instead of re-running the fund.
    // Read-only and above the signing boundary on purpose: someone checking
    // whether their money already moved should not have to unlock a key first.
    const seconds = positional[1] ? Number(positional[1]) : 900;
    const found = await recentFunds(connection, p, seconds);
    if (!found.length) {
      console.log(`no fund in the last ${seconds}s`);
      return;
    }
    for (const entry of found) {
      const ago = Math.floor(Date.now() / 1000) - entry.blockTime;
      console.log(`${fmt(entry.amount).padStart(22)}  ${ago}s ago  ${entry.signature}`);
    }
    return;
  }

  // Everything past here signs.
  const signer = loadKeypair(flags.keypair ?? `${homedir()}/.config/solana/id.json`);
  const me = signer.publicKey;
  const source = flags.source ? new PublicKey(flags.source) : p.ata(me);
  const destination = flags.destination ? new PublicKey(flags.destination) : p.ata(me);

  switch (command) {
    case 'initialize':
      console.log(`initializing pool as ${me.toBase58()}`);
      await send(connection, ixInitialize(p, me), signer);
      break;

    case 'create-ata': {
      const owner = positional[1] ? new PublicKey(positional[1]) : me;
      console.log(`creating ${p.ata(owner).toBase58()} for ${owner.toBase58()}`);
      await send(connection, ixCreateAta(p, me, owner), signer);
      break;
    }

    case 'create-mint': {
      // Devnet only: the real mint already exists and has no mint authority.
      if (!flags['mint-keypair']) throw new Error('create-mint needs --mint-keypair');
      const mintKey = loadKeypair(flags['mint-keypair']);
      if (!mintKey.publicKey.equals(p.mint)) {
        throw new Error(
          `--mint-keypair is ${mintKey.publicKey.toBase58()} but --mint says ${p.mint.toBase58()}; ` +
            'the address is compiled into the program, so these must agree',
        );
      }
      if (await connection.getAccountInfo(p.mint)) {
        console.log(`mint ${p.mint.toBase58()} already exists`);
        break;
      }
      const rent = await connection.getMinimumBalanceForRentExemption(82);
      console.log(`creating mint ${p.mint.toBase58()}, authority ${me.toBase58()}`);
      await send(connection, ixCreateMint(p, me, me, rent), [signer, mintKey]);
      break;
    }

    case 'mint-to': {
      const owner = new PublicKey(positional[1]);
      const amount = parseAmount(positional[2]);
      const ata = p.ata(owner);
      const instructions = [];
      if (!(await connection.getAccountInfo(ata))) instructions.push(ixCreateAta(p, me, owner));
      instructions.push(ixMintTo(p, ata, me, amount));
      console.log(`minting ${fmt(amount)} to ${ata.toBase58()}`);
      await send(connection, instructions, signer);
      break;
    }

    case 'fund': {
      const amount = parseAmount(positional[1]);
      await guardAgainstDoubleFund(connection, p, amount, flags.force === true,
        flags.window ? Number(flags.window) : 900);
      console.log(`funding ${fmt(amount)} from ${source.toBase58()}`);
      await send(connection, ixFund(p, me, source, amount), signer);
      break;
    }

    case 'stake': {
      const amount = parseAmount(positional[1]);
      console.log(`staking ${fmt(amount)} from ${source.toBase58()}`);
      console.log('note: this restarts the hold on the whole position');
      await send(connection, ixStake(p, me, source, amount), signer);
      break;
    }

    case 'claim':
      await send(connection, ixClaim(p, me, destination), signer);
      break;

    case 'unstake':
      await send(connection, ixUnstake(p, me, destination), signer);
      break;

    case 'close':
      await send(connection, ixClose(p, me), signer);
      break;

    default:
      console.error(`unknown command ${JSON.stringify(command)}\n`);
      console.log(USAGE);
      process.exitCode = 1;
      return;
  }

  if (command !== 'initialize' && command !== 'create-ata') {
    console.log('');
    await show(connection, p, me);
  }
}

main().catch((e) => {
  console.error(`error: ${e.message}`);
  if (e.logs) for (const line of e.logs) console.error(`  ${line}`);
  process.exitCode = 1;
});
