# Независимое ревью стейкинг-контракта SOLVE

Ты — аудитор Solana-программ. Ниже полный исходник нативной программы (без
Anchor), которая через день-два будет выложена в mainnet и будет держать
токены посторонних людей. Внешнего платного аудита не будет: твоё ревью — одна
из двух проверок, которые вообще состоятся.

## Правила, важнее самой задачи

1. **Не верь комментариям в коде.** Они описывают замысел автора, а не то, что
   делает код. Если комментарий утверждает свойство — проверь его по коду.
2. **Не верь тому, что тесты существуют.** Наличие теста не доказывает, что он
   проверяет то, что написано в его имени.
3. **Не верь фактам из раздела «Контекст» ниже.** Они даны как утверждения
   автора. Про каждое скажи, проверяемо ли оно из кода, и что будет, если оно
   окажется ложным.
4. **Не пересказывай код.** Мне не нужно описание того, что делает программа.
   Нужны конкретные дефекты: путь вызова, состояние, результат.
5. **Если дефектов нет — так и скажи.** Выдуманная находка хуже, чем честное
   «в этой области чисто». Но прежде чем сказать «чисто», покажи, что именно ты
   проверил в этой области.
6. Для каждой находки укажи: серьёзность, конкретный сценарий (кто, в каком
   порядке, с какими значениями), и что именно ломается — деньги, учёт,
   доступность.

## Что программа обещает

- Люди вносят токен SOLVE (Token-2022, 6 знаков) и получают долю наград.
- Награды поступают **только** снаружи: владелец проекта раз в неделю заливает
  купленное количество токенов. Фиксированной ставки нет.
- Каждый залив размазывается ровно на семь суток; недоплаченный остаток
  предыдущего окна подмешивается, а не теряется.
- Доля стейкера = его вклад / сумма всех вкладов, посекундно.
- Свежий стейк нельзя забрать три дня. Пополнение перезапускает выдержку на
  весь баланс. Награды можно забирать когда угодно, включая после вывода.
- **Ключевое обещание: изъять принципал или награды не может никто, включая
  автора программы.** Нет инструкций вывода, паузы, смены минта, смены
  хранилищ.
- Выход только целиком, частичного вывода нет.

## Контекст деплоя — это утверждения автора, а не факты

- Адрес минта вкомпилирован; у минта, по утверждению автора, нет transfer fee,
  transfer hook, freeze authority и mint authority, а расширения минта
  фиксируются при его создании.
- `FUNDING_AUTHORITY` вкомпилирован, инструкции его смены нет.
- Программа остаётся обновляемой: upgrade authority сохраняется у владельца.
- Потолка TVL нет ни в каком виде.
- Тот же ключ сейчас является и funding authority, и upgrade authority.
- Минимальный вклад 100 000 SOLVE, выдержка 3 суток, окно 7 суток.

## Вопросы, на которые нужен ответ

Отвечай по каждому пункту отдельно.

1. **Сохранность средств.** Существует ли путь, при котором кто-либо получает
   больше, чем ему причитается, или забирает чужое? Учитывай неверные аккаунты,
   подставные аккаунты, повторный вход (reentrancy), порядок операций внутри
   транзакции.
2. **Порядок учёта.** Обновляется ли глобальный аккумулятор наград до каждого
   изменения суммарного веса, а расчёт позиции — до изменения её суммы?
   Найди хотя бы один путь, где это нарушено, или покажи, что таких нет.
3. **Арифметика.** Округления, переполнения, деление на ноль, потеря точности.
   В какую сторону режет каждое округление и может ли сумма выплат превысить
   сумму залитого?
4. **Проверки аккаунтов.** Для каждой инструкции: все ли переданные аккаунты
   проверены на принадлежность программе, на вывод из правильных seed'ов, на
   совпадение с ожидаемыми адресами? Можно ли подсунуть чужой аккаунт, другую
   токен-программу, другой минт?
5. **CPI.** В каком порядке идут запись состояния и вызов токен-программы?
   Что произойдёт при повторном входе в программу во время перевода?
6. **Создание и закрытие аккаунтов.** Можно ли помешать созданию чужой позиции,
   заранее отправив на её адрес лампорты или как-то иначе? Безопасно ли
   закрытие позиции (возврат ренты), нельзя ли воскресить закрытый аккаунт?
7. **Время.** Что будет при движении часов валидатора назад, при длительном
   простое, при заливе ровно в момент окончания окна?
8. **Отказ в обслуживании.** Может ли один участник помешать работе остальных
   или помешать владельцу залить награды?
9. **Что ломается, если утверждения из раздела «Контекст» неверны** — по
   каждому отдельно.
10. **Чего в этой программе нет, но должно быть**, учитывая, что она держит
    чужие деньги и что потолка TVL нет.

## Формат ответа

Сначала список находок, самые тяжёлые сверху. Для каждой:

- серьёзность: критическая / высокая / средняя / низкая / замечание
- где: имя функции и строка
- сценарий: последовательность действий, приводящая к проблеме
- последствие: что теряется
- уверенность: подтверждено кодом / вероятно / требует проверки в сети

В конце — отдельным разделом: что ты проверил и не нашёл проблем. Без этого
раздела ответ не принимается.

---

## Исходный код целиком

Файл `src/lib.rs`, нативная Solana-программа, без Anchor.

```rust
#![deny(unsafe_code)]

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    declare_id,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction as system_instruction;

declare_id!("GyCrZ9JQq1LAWJ3fWqn52uEnAFUmg52NHQvcatq2mcDe");

#[cfg(not(feature = "devnet-mint"))]
const SOLVE_MINT: Pubkey = pubkey!("GwyWFsDKW9a2ref1EWqdUS7B37Toii433zrAh9Dipump");
/// A throwaway Token-2022 mint for devnet, six decimals and no extensions.
/// The real SOLVE mint cannot be recreated on another cluster: its address is
/// a pump.fun vanity keypair nobody outside pump.fun holds, and an account can
/// only be created at an address whose key can sign for it. Cloning works on a
/// local validator, which writes account state directly; a public cluster has
/// no such door. Its keypair lives beside the program keypair under `.keys/`.
#[cfg(feature = "devnet-mint")]
const SOLVE_MINT: Pubkey = pubkey!("9dybdAGgG1w4yZS4oBzgY424pxHscQFQkWr9qobRQvFH");
const TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
#[cfg(not(feature = "test-authority"))]
const FUNDING_AUTHORITY: Pubkey = pubkey!("ACbZ5vajyFFseZoYTdrzcxLSJbnnf4pt3MQoZ7XtDrws");
/// A deliberately public keypair, used only by the integration suite so it can
/// sign as the funding authority. A binary built with this feature announces
/// itself on every initialize, and `scripts/build.ps1` refuses to hand over a
/// program that contains the marker string.
#[cfg(feature = "test-authority")]
const FUNDING_AUTHORITY: Pubkey = pubkey!("7BRPh4s6sva7zH4sRHxNvf2cmtFLjoiVuBjgNvf2xFrr");
const POOL_SEED: &[u8] = b"pool";
const POSITION_SEED: &[u8] = b"position";
const PRINCIPAL_VAULT_SEED: &[u8] = b"vault_principal";
const REWARD_VAULT_SEED: &[u8] = b"vault_reward";
/// The window each `fund` is spread over. Not a calendar epoch: funding is
/// allowed at any time, and whatever the previous funding has not yet paid out
/// is folded into the new rate rather than lost. Funding every few days
/// therefore leaves a smoothing buffer in the vault instead of a dry spell.
#[cfg(not(feature = "fast-clock"))]
const WEEK: i64 = 7 * 24 * 60 * 60;
/// Seven minutes, so a whole lifecycle can be driven against a real validator
/// in one sitting instead of over a real week. See `fast-clock` in Cargo.toml.
#[cfg(feature = "fast-clock")]
const WEEK: i64 = 7 * 60;
const SCALE: u128 = 1_000_000_000_000_000_000;
const POOL_LEN: usize = 149;
const POSITION_LEN: usize = 77;
const POOL_MAGIC: &[u8; 4] = b"SLVP";
const POSITION_MAGIC: &[u8; 4] = b"SLVS";
/// SOLVE has six decimals and a mint cannot change them, so TransferChecked
/// can carry a compiled-in value instead of reading the mint account.
const SOLVE_DECIMALS: u8 = 6;
/// The SOLVE mint carries only metadata extensions, and mint extensions are
/// fixed at initialization, so a vault never needs account extension space.
const VAULT_LEN: usize = 165;
/// One hundred thousand SOLVE. Keeps `total_weight` far away from the tiny
/// values that would make `reward_per_weight` grow quickly, and keeps a
/// position worth far more than the rent its account costs: at the 2026-08-24
/// price the rent is about fourteen cents against a six-dollar stake.
const MIN_STAKE: u64 = 100_000_000_000;
/// How long a stake must sit before it can leave. Deliberately short: rewards
/// accrue per second, so this is not a reward-timing device — it only stops a
/// wallet from jumping in and out around a funding transaction. Topping up
/// restarts it for the whole position, otherwise the minimum could be dodged
/// by seeding a position once and adding to it later.
#[cfg(not(feature = "fast-clock"))]
const MIN_HOLD: i64 = 3 * 24 * 60 * 60;
/// Three minutes under `fast-clock`. Nothing in the program reads how long the
/// hold is; it only compares `now` against `unlock_at`, so a shortened
/// constant exercises the same code path the production one does.
#[cfg(feature = "fast-clock")]
const MIN_HOLD: i64 = 3 * 60;

#[cfg(not(feature = "no-entrypoint"))]
use solana_program::entrypoint;
#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let (&tag, args) = data.split_first().ok_or(StakingError::BadInstruction)?;
    match tag {
        0 if args.is_empty() => initialize(program_id, accounts),
        1 if args.len() == 8 => fund(program_id, accounts, read_u64(args)?),
        2 if args.len() == 8 => stake(program_id, accounts, read_u64(args)?),
        3 if args.is_empty() => claim(program_id, accounts),
        4 if args.is_empty() => unstake(program_id, accounts),
        5 if args.is_empty() => close_position(program_id, accounts),
        _ => Err(StakingError::BadInstruction.into()),
    }
}

fn initialize(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let pool_info = next_account_info(&mut it)?;
    let principal_vault = next_account_info(&mut it)?;
    let reward_vault = next_account_info(&mut it)?;
    let mint = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    require_signer(payer)?;
    require_key(payer, &FUNDING_AUTHORITY)?;
    require_key(mint, &SOLVE_MINT)?;
    require_key(system_program, &system_program::id())?;
    require_key(token_program, &TOKEN_2022)?;

    let (expected_pool, bump) = Pubkey::find_program_address(&[POOL_SEED], program_id);
    require_key(pool_info, &expected_pool)?;
    create_pda(
        payer,
        pool_info,
        system_program,
        program_id,
        POOL_LEN,
        &[POOL_SEED, &[bump]],
    )?;

    // Both vaults are program addresses created here rather than accounts
    // supplied by the caller. Nothing outside this program can pre-set a
    // delegate or a close authority on them, and the program has no
    // instruction that would set either one later.
    create_vault(payer, principal_vault, mint, system_program, token_program,
                 program_id, PRINCIPAL_VAULT_SEED, &expected_pool)?;
    create_vault(payer, reward_vault, mint, system_program, token_program,
                 program_id, REWARD_VAULT_SEED, &expected_pool)?;

    Pool {
        bump,
        principal_vault: *principal_vault.key,
        reward_vault: *reward_vault.key,
        total_weight: 0,
        reward_rate_scaled: 0,
        reward_per_weight: 0,
        rollover_scaled: 0,
        last_update: 0,
        period_finish: 0,
    }
    .store(&mut pool_info.try_borrow_mut_data()?)?;
    #[cfg(feature = "test-authority")]
    msg!("BUILT-WITH-TEST-AUTHORITY-DO-NOT-DEPLOY");
    #[cfg(feature = "fast-clock")]
    msg!("BUILT-WITH-FAST-CLOCK-DO-NOT-DEPLOY");
    #[cfg(feature = "devnet-mint")]
    msg!("BUILT-WITH-DEVNET-MINT-DO-NOT-DEPLOY");
    msg!("Pool initialized");
    Ok(())
}

fn fund(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    if amount == 0 {
        return Err(StakingError::BadAmount.into());
    }
    let mut it = accounts.iter();
    let funder = next_account_info(&mut it)?;
    let source = next_account_info(&mut it)?;
    let pool_info = next_account_info(&mut it)?;
    let reward_vault = next_account_info(&mut it)?;
    let mint = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    require_signer(funder)?;
    require_pool(program_id, pool_info)?;
    require_key(mint, &SOLVE_MINT)?;
    require_key(token_program, &TOKEN_2022)?;

    let now = Clock::get()?.unix_timestamp;
    let mut pool = Pool::load(&pool_info.try_borrow_data()?)?;
    require_key(funder, &FUNDING_AUTHORITY)?;
    require_key(reward_vault, &pool.reward_vault)?;
    // Rewards can only be funded into a pool that has someone to pay. Timing
    // is otherwise free: see the leftover fold below.
    if pool.total_weight == 0 {
        return Err(StakingError::PoolEmpty.into());
    }
    pool.update(now)?;
    token_transfer(source, mint, reward_vault, funder, token_program, amount, None)?;
    let mut funded_scaled = (amount as u128)
        .checked_mul(SCALE)
        .ok_or(StakingError::Overflow)?
        .checked_add(pool.rollover_scaled)
        .ok_or(StakingError::Overflow)?;
    // Funding before the previous window ran out must not strand the part that
    // has not been paid yet. Those tokens are already in the reward vault, so
    // folding them into the new rate redistributes them rather than promising
    // anything new, and total payouts still cannot exceed total funded.
    if now < pool.period_finish {
        let remaining = (pool.period_finish - now) as u128;
        let leftover = remaining
            .checked_mul(pool.reward_rate_scaled)
            .ok_or(StakingError::Overflow)?;
        funded_scaled = funded_scaled.checked_add(leftover).ok_or(StakingError::Overflow)?;
    }
    pool.reward_rate_scaled = funded_scaled / (WEEK as u128);
    pool.rollover_scaled = funded_scaled % (WEEK as u128);
    pool.last_update = now;
    pool.period_finish = now.checked_add(WEEK).ok_or(StakingError::Overflow)?;
    pool.store(&mut pool_info.try_borrow_mut_data()?)?;
    msg!("fund {} spread over seven days from now", amount);
    Ok(())
}

fn stake(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    if amount < MIN_STAKE {
        return Err(StakingError::BadAmount.into());
    }
    let mut it = accounts.iter();
    let owner = next_account_info(&mut it)?;
    let source = next_account_info(&mut it)?;
    let pool_info = next_account_info(&mut it)?;
    let position_info = next_account_info(&mut it)?;
    let principal_vault = next_account_info(&mut it)?;
    let mint = next_account_info(&mut it)?;
    let system_program = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    require_signer(owner)?;
    require_pool(program_id, pool_info)?;
    require_key(mint, &SOLVE_MINT)?;
    require_key(system_program, &system_program::id())?;
    require_key(token_program, &TOKEN_2022)?;

    let (expected_position, position_bump) =
        Pubkey::find_program_address(&[POSITION_SEED, owner.key.as_ref()], program_id);
    require_key(position_info, &expected_position)?;
    if position_info.data_is_empty() {
        create_pda(
            owner,
            position_info,
            system_program,
            program_id,
            POSITION_LEN,
            &[POSITION_SEED, owner.key.as_ref(), &[position_bump]],
        )?;
        Position::new(*owner.key, position_bump).store(&mut position_info.try_borrow_mut_data()?)?;
    } else if position_info.owner != program_id {
        return Err(StakingError::WrongAccount.into());
    }

    let now = Clock::get()?.unix_timestamp;
    let mut pool = Pool::load(&pool_info.try_borrow_data()?)?;
    require_key(principal_vault, &pool.principal_vault)?;
    pool.update(now)?;
    let mut position = Position::load(&position_info.try_borrow_data()?)?;
    require_owner(&position, owner.key)?;
    settle(&pool, &mut position)?;

    position.amount = position.amount.checked_add(amount).ok_or(StakingError::Overflow)?;
    position.unlock_at = now.checked_add(MIN_HOLD).ok_or(StakingError::Overflow)?;
    pool.total_weight = pool
        .total_weight
        .checked_add(amount as u128)
        .ok_or(StakingError::Overflow)?;
    token_transfer(source, mint, principal_vault, owner, token_program, amount, None)?;
    pool.store(&mut pool_info.try_borrow_mut_data()?)?;
    position.store(&mut position_info.try_borrow_mut_data()?)?;
    msg!("stake {} until {}", amount, position.unlock_at);
    Ok(())
}

fn claim(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let mut it = accounts.iter();
    let owner = next_account_info(&mut it)?;
    let destination = next_account_info(&mut it)?;
    let pool_info = next_account_info(&mut it)?;
    let position_info = next_account_info(&mut it)?;
    let reward_vault = next_account_info(&mut it)?;
    let mint = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    require_signer(owner)?;
    require_pool(program_id, pool_info)?;
    require_key(mint, &SOLVE_MINT)?;
    require_key(token_program, &TOKEN_2022)?;

    let now = Clock::get()?.unix_timestamp;
    let mut pool = Pool::load(&pool_info.try_borrow_data()?)?;
    require_key(reward_vault, &pool.reward_vault)?;
    pool.update(now)?;
    let mut position = Position::load(&position_info.try_borrow_data()?)?;
    require_owner_and_pda(program_id, &position, owner.key, position_info)?;
    settle(&pool, &mut position)?;
    let reward = position.accrued;
    if reward == 0 {
        return Err(StakingError::NothingToClaim.into());
    }
    position.accrued = 0;
    let bump = [pool.bump];
    let seeds: &[&[u8]] = &[POOL_SEED, &bump];
    token_transfer(reward_vault, mint, destination, pool_info, token_program, reward, Some(seeds))?;
    pool.store(&mut pool_info.try_borrow_mut_data()?)?;
    position.store(&mut position_info.try_borrow_mut_data()?)?;
    msg!("claim {}", reward);
    Ok(())
}

fn unstake(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let mut it = accounts.iter();
    let owner = next_account_info(&mut it)?;
    let destination = next_account_info(&mut it)?;
    let pool_info = next_account_info(&mut it)?;
    let position_info = next_account_info(&mut it)?;
    let principal_vault = next_account_info(&mut it)?;
    let mint = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    require_signer(owner)?;
    require_pool(program_id, pool_info)?;
    require_key(mint, &SOLVE_MINT)?;
    require_key(token_program, &TOKEN_2022)?;

    let now = Clock::get()?.unix_timestamp;
    let mut pool = Pool::load(&pool_info.try_borrow_data()?)?;
    require_key(principal_vault, &pool.principal_vault)?;
    pool.update(now)?;
    let mut position = Position::load(&position_info.try_borrow_data()?)?;
    require_owner_and_pda(program_id, &position, owner.key, position_info)?;
    settle(&pool, &mut position)?;
    if now < position.unlock_at || position.amount == 0 {
        return Err(StakingError::LockedOrEmpty.into());
    }
    let amount = position.amount;
    pool.total_weight = pool
        .total_weight
        .checked_sub(amount as u128)
        .ok_or(StakingError::Overflow)?;
    position.amount = 0;
    let bump = [pool.bump];
    let seeds: &[&[u8]] = &[POOL_SEED, &bump];
    token_transfer(principal_vault, mint, destination, pool_info, token_program, amount, Some(seeds))?;
    pool.store(&mut pool_info.try_borrow_mut_data()?)?;
    position.store(&mut position_info.try_borrow_mut_data()?)?;
    msg!("unstake {} unclaimed {}", amount, position.accrued);
    Ok(())
}

/// Returns the rent of an emptied position to its owner. A position holds no
/// pool state once it is empty, so this needs neither the pool account nor a
/// settlement: `settle` on a zero amount can only earn zero.
fn close_position(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let mut it = accounts.iter();
    let owner = next_account_info(&mut it)?;
    let position_info = next_account_info(&mut it)?;
    require_signer(owner)?;
    let position = Position::load(&position_info.try_borrow_data()?)?;
    require_owner_and_pda(program_id, &position, owner.key, position_info)?;
    if position.amount != 0 || position.accrued != 0 {
        return Err(StakingError::PositionNotEmpty.into());
    }

    let refund = position_info.lamports();
    **position_info.try_borrow_mut_lamports()? = 0;
    **owner.try_borrow_mut_lamports()? = owner
        .lamports()
        .checked_add(refund)
        .ok_or(StakingError::Overflow)?;
    position_info.resize(0)?;
    position_info.assign(&system_program::id());
    msg!("close position, refund {} lamports", refund);
    Ok(())
}

/// A position's weight is simply its staked amount: rewards are split in plain
/// proportion to what each wallet put in, with no lock multiplier of any kind.
fn settle(pool: &Pool, position: &mut Position) -> Result<(), ProgramError> {
    let delta = pool.reward_per_weight.checked_sub(position.reward_per_weight_paid)
        .ok_or(StakingError::Overflow)?;
    let earned = (position.amount as u128).checked_mul(delta).ok_or(StakingError::Overflow)? / SCALE;
    let earned_u64 = u64::try_from(earned).map_err(|_| StakingError::Overflow)?;
    position.accrued = position.accrued.checked_add(earned_u64).ok_or(StakingError::Overflow)?;
    position.reward_per_weight_paid = pool.reward_per_weight;
    Ok(())
}

fn create_pda<'a>(
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    owner: &Pubkey,
    space: usize,
    seeds: &[&[u8]],
) -> ProgramResult {
    if !account.data_is_empty() || account.owner != &system_program::id() {
        return Err(StakingError::AlreadyInitialized.into());
    }
    let lamports = Rent::get()?.minimum_balance(space);
    let missing = lamports.saturating_sub(account.lamports());
    if missing > 0 {
        invoke(
            &system_instruction::transfer(payer.key, account.key, missing),
            &[payer.clone(), account.clone(), system_program.clone()],
        )?;
    }
    invoke_signed(
        &system_instruction::allocate(account.key, space as u64),
        &[account.clone(), system_program.clone()],
        &[seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(account.key, owner),
        &[account.clone(), system_program.clone()],
        &[seeds],
    )
}

fn token_transfer<'a>(
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: Option<&[&[u8]]>,
) -> ProgramResult {
    let mut data = Vec::with_capacity(10);
    data.push(12); // Token-2022 TransferChecked discriminator.
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(SOLVE_DECIMALS);
    let ix = Instruction {
        program_id: TOKEN_2022,
        accounts: vec![
            AccountMeta::new(*source.key, false),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data,
    };
    let infos = [source.clone(), mint.clone(), destination.clone(), authority.clone(), token_program.clone()];
    match signer_seeds {
        Some(seeds) => invoke_signed(&ix, &infos, &[seeds]),
        None => invoke(&ix, &infos),
    }
}

/// Creates one of the two pool-owned Token-2022 vaults at its program address
/// and initializes it with the pool PDA as authority.
#[allow(clippy::too_many_arguments)]
fn create_vault<'a>(
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    program_id: &Pubkey,
    seed: &[u8],
    authority: &Pubkey,
) -> ProgramResult {
    let (expected, bump) = Pubkey::find_program_address(&[seed], program_id);
    require_key(vault, &expected)?;
    create_pda(payer, vault, system_program, &TOKEN_2022, VAULT_LEN, &[seed, &[bump]])?;

    let mut data = Vec::with_capacity(33);
    data.push(18); // Token-2022 InitializeAccount3 discriminator.
    data.extend_from_slice(authority.as_ref());
    invoke(
        &Instruction {
            program_id: TOKEN_2022,
            accounts: vec![
                AccountMeta::new(*vault.key, false),
                AccountMeta::new_readonly(*mint.key, false),
            ],
            data,
        },
        &[vault.clone(), mint.clone(), token_program.clone()],
    )
}

fn require_pool(program_id: &Pubkey, info: &AccountInfo) -> ProgramResult {
    let (expected, _) = Pubkey::find_program_address(&[POOL_SEED], program_id);
    require_key(info, &expected)?;
    if info.owner != program_id {
        return Err(StakingError::WrongAccount.into());
    }
    Ok(())
}

fn require_owner(position: &Position, owner: &Pubkey) -> ProgramResult {
    if &position.owner != owner { Err(StakingError::WrongAccount.into()) } else { Ok(()) }
}

fn require_owner_and_pda(
    program_id: &Pubkey,
    position: &Position,
    owner: &Pubkey,
    info: &AccountInfo,
) -> ProgramResult {
    require_owner(position, owner)?;
    if info.owner != program_id {
        return Err(StakingError::WrongAccount.into());
    }
    let (expected, _) = Pubkey::find_program_address(&[POSITION_SEED, owner.as_ref()], program_id);
    require_key(info, &expected)
}

fn require_key(info: &AccountInfo, expected: &Pubkey) -> ProgramResult {
    if info.key != expected { Err(StakingError::WrongAccount.into()) } else { Ok(()) }
}

fn require_signer(info: &AccountInfo) -> ProgramResult {
    if !info.is_signer { Err(ProgramError::MissingRequiredSignature) } else { Ok(()) }
}

fn read_u64(data: &[u8]) -> Result<u64, ProgramError> {
    Ok(u64::from_le_bytes(data[..8].try_into().map_err(|_| StakingError::BadInstruction)?))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Pool {
    bump: u8,
    principal_vault: Pubkey,
    reward_vault: Pubkey,
    total_weight: u128,
    reward_rate_scaled: u128,
    reward_per_weight: u128,
    rollover_scaled: u128,
    last_update: i64,
    period_finish: i64,
}

impl Pool {
    fn update(&mut self, now: i64) -> ProgramResult {
        let applicable = now.min(self.period_finish);
        if applicable <= self.last_update {
            return Ok(());
        }
        let elapsed = (applicable - self.last_update) as u128;
        if self.total_weight > 0 {
            let increment = elapsed
                .checked_mul(self.reward_rate_scaled)
                .ok_or(StakingError::Overflow)?
                .checked_div(self.total_weight)
                .ok_or(StakingError::Overflow)?;
            self.reward_per_weight = self.reward_per_weight.checked_add(increment)
                .ok_or(StakingError::Overflow)?;
        } else {
            let missed = elapsed.checked_mul(self.reward_rate_scaled)
                .ok_or(StakingError::Overflow)?;
            self.rollover_scaled = self.rollover_scaled.checked_add(missed)
                .ok_or(StakingError::Overflow)?;
        }
        self.last_update = applicable;
        Ok(())
    }

    fn load(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() != POOL_LEN || &data[..4] != POOL_MAGIC {
            return Err(StakingError::WrongAccount.into());
        }
        let mut r = Reader::new(&data[4..]);
        Ok(Self {
            bump: r.u8()?,
            principal_vault: r.pubkey()?,
            reward_vault: r.pubkey()?,
            total_weight: r.u128()?,
            reward_rate_scaled: r.u128()?,
            reward_per_weight: r.u128()?,
            rollover_scaled: r.u128()?,
            last_update: r.i64()?,
            period_finish: r.i64()?,
        })
    }

    fn store(&self, data: &mut [u8]) -> ProgramResult {
        if data.len() != POOL_LEN { return Err(StakingError::WrongAccount.into()); }
        data.fill(0);
        data[..4].copy_from_slice(POOL_MAGIC);
        let mut w = Writer::new(&mut data[4..]);
        w.u8(self.bump)?;
        w.pubkey(&self.principal_vault)?;
        w.pubkey(&self.reward_vault)?;
        w.u128(self.total_weight)?;
        w.u128(self.reward_rate_scaled)?;
        w.u128(self.reward_per_weight)?;
        w.u128(self.rollover_scaled)?;
        w.i64(self.last_update)?;
        w.i64(self.period_finish)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Position {
    bump: u8,
    owner: Pubkey,
    amount: u64,
    reward_per_weight_paid: u128,
    accrued: u64,
    unlock_at: i64,
}

impl Position {
    fn new(owner: Pubkey, bump: u8) -> Self { Self { owner, bump, ..Self::default() } }

    fn load(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() != POSITION_LEN || &data[..4] != POSITION_MAGIC {
            return Err(StakingError::WrongAccount.into());
        }
        let mut r = Reader::new(&data[4..]);
        Ok(Self {
            bump: r.u8()?, owner: r.pubkey()?, amount: r.u64()?,
            reward_per_weight_paid: r.u128()?, accrued: r.u64()?,
            unlock_at: r.i64()?,
        })
    }

    fn store(&self, data: &mut [u8]) -> ProgramResult {
        if data.len() != POSITION_LEN { return Err(StakingError::WrongAccount.into()); }
        data.fill(0);
        data[..4].copy_from_slice(POSITION_MAGIC);
        let mut w = Writer::new(&mut data[4..]);
        w.u8(self.bump)?; w.pubkey(&self.owner)?; w.u64(self.amount)?;
        w.u128(self.reward_per_weight_paid)?; w.u64(self.accrued)?;
        w.i64(self.unlock_at)
    }
}

struct Reader<'a> { data: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self { Self { data, offset: 0 } }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ProgramError> {
        let end = self.offset.checked_add(N).ok_or(StakingError::Overflow)?;
        let out = self.data.get(self.offset..end).ok_or(StakingError::WrongAccount)?
            .try_into().map_err(|_| StakingError::WrongAccount)?;
        self.offset = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, ProgramError> { Ok(self.take::<1>()?[0]) }
    fn u64(&mut self) -> Result<u64, ProgramError> { Ok(u64::from_le_bytes(self.take()?)) }
    fn u128(&mut self) -> Result<u128, ProgramError> { Ok(u128::from_le_bytes(self.take()?)) }
    fn i64(&mut self) -> Result<i64, ProgramError> { Ok(i64::from_le_bytes(self.take()?)) }
    fn pubkey(&mut self) -> Result<Pubkey, ProgramError> { Ok(Pubkey::new_from_array(self.take()?)) }
}

struct Writer<'a> { data: &'a mut [u8], offset: usize }
impl<'a> Writer<'a> {
    fn new(data: &'a mut [u8]) -> Self { Self { data, offset: 0 } }
    fn put(&mut self, bytes: &[u8]) -> ProgramResult {
        let end = self.offset.checked_add(bytes.len()).ok_or(StakingError::Overflow)?;
        self.data.get_mut(self.offset..end).ok_or(StakingError::WrongAccount)?.copy_from_slice(bytes);
        self.offset = end;
        Ok(())
    }
    fn u8(&mut self, v: u8) -> ProgramResult { self.put(&[v]) }
    fn u64(&mut self, v: u64) -> ProgramResult { self.put(&v.to_le_bytes()) }
    fn u128(&mut self, v: u128) -> ProgramResult { self.put(&v.to_le_bytes()) }
    fn i64(&mut self, v: i64) -> ProgramResult { self.put(&v.to_le_bytes()) }
    fn pubkey(&mut self, v: &Pubkey) -> ProgramResult { self.put(v.as_ref()) }
}

#[repr(u32)]
enum StakingError {
    BadInstruction = 1,
    WrongAccount,
    AlreadyInitialized,
    BadAmount,
    PoolEmpty,
    NothingToClaim,
    LockedOrEmpty,
    PositionNotEmpty,
    Overflow,
}

impl From<StakingError> for ProgramError {
    fn from(value: StakingError) -> Self { ProgramError::Custom(value as u32) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrips() {
        let pool = Pool {
            bump: 7,
            principal_vault: Pubkey::new_unique(), reward_vault: Pubkey::new_unique(),
            total_weight: 12, reward_rate_scaled: 34, reward_per_weight: 56,
            rollover_scaled: 67,
            last_update: 78, period_finish: 90,
        };
        let mut bytes = vec![0; POOL_LEN];
        pool.store(&mut bytes).unwrap();
        assert_eq!(Pool::load(&bytes).unwrap(), pool);

        let position = Position {
            bump: 4, owner: Pubkey::new_unique(), amount: 123,
            reward_per_weight_paid: 789, accrued: 22, unlock_at: 333,
        };
        let mut bytes = vec![0; POSITION_LEN];
        position.store(&mut bytes).unwrap();
        assert_eq!(Position::load(&bytes).unwrap(), position);
    }

    #[test]
    fn rewards_follow_amount_and_time() {
        let mut pool = Pool {
            total_weight: 2_000_000,
            reward_rate_scaled: (604_800_000u128 * SCALE) / WEEK as u128,
            last_update: 100,
            period_finish: 100 + WEEK,
            ..Pool::default()
        };
        pool.update(100 + WEEK / 2).unwrap();
        let mut position = Position { amount: 1_000_000, ..Position::default() };
        settle(&pool, &mut position).unwrap();
        assert_eq!(position.accrued, 151_200_000);
    }

    #[test]
    fn rewards_split_in_plain_proportion_to_amount() {
        // The whole reward policy in one assertion: twice the stake earns
        // exactly twice the reward, with no lock multiplier able to change it.
        let mut pool = Pool {
            total_weight: 300_000,
            reward_rate_scaled: 3 * SCALE,
            last_update: 0,
            period_finish: 100_000,
            ..Pool::default()
        };
        pool.update(100_000).unwrap();

        let mut small = Position { amount: 100_000, ..Position::default() };
        let mut large = Position { amount: 200_000, ..Position::default() };
        settle(&pool, &mut small).unwrap();
        settle(&pool, &mut large).unwrap();

        assert_eq!(small.accrued, 100_000);
        assert_eq!(large.accrued, 200_000);
        assert_eq!(large.accrued, 2 * small.accrued);
        // Everything the rate emitted reached a staker, nothing was invented.
        assert_eq!(small.accrued + large.accrued, 300_000);
    }

    #[test]
    fn epoch_never_accrues_past_finish() {
        let mut pool = Pool {
            total_weight: 1,
            reward_rate_scaled: SCALE,
            last_update: 0,
            period_finish: 10,
            ..Pool::default()
        };
        pool.update(99).unwrap();
        assert_eq!(pool.reward_per_weight, 10 * SCALE);
        assert_eq!(pool.last_update, 10);
    }

    #[test]
    fn empty_pool_rewards_roll_into_next_epoch() {
        let mut pool = Pool {
            reward_rate_scaled: 3 * SCALE,
            last_update: 10,
            period_finish: 20,
            ..Pool::default()
        };
        pool.update(20).unwrap();
        assert_eq!(pool.reward_per_weight, 0);
        assert_eq!(pool.rollover_scaled, 30 * SCALE);
    }

    /// A build meant for a cluster must carry the real policy. `fast-clock`
    /// exists so a lifecycle can be driven in twenty minutes rather than three
    /// days, and this is the assertion that stops a shortened build from
    /// passing itself off as a production one.
    #[test]
    #[cfg(not(feature = "fast-clock"))]
    fn production_timings_are_what_the_policy_says() {
        assert_eq!(MIN_HOLD, 3 * 24 * 60 * 60, "the hold must be three days");
        assert_eq!(WEEK, 7 * 24 * 60 * 60, "the smoothing window must be seven days");
        assert_eq!(MIN_STAKE, 100_000_000_000, "the minimum stake must be 100,000 SOLVE");
    }

    #[test]
    fn state_lengths_match_their_fields() {
        assert_eq!(POOL_LEN, 4 + 1 + 32 + 32 + 16 * 4 + 8 + 8);
        assert_eq!(POSITION_LEN, 4 + 1 + 32 + 8 + 16 + 8 + 8);
    }

    #[test]
    fn settling_twice_pays_once() {
        let pool = Pool { reward_per_weight: 5 * SCALE, ..Pool::default() };
        let mut position = Position { amount: 3, ..Position::default() };
        settle(&pool, &mut position).unwrap();
        assert_eq!(position.accrued, 15);
        settle(&pool, &mut position).unwrap();
        assert_eq!(position.accrued, 15);
    }

    #[test]
    fn a_stale_pool_does_not_accrue_before_its_first_funding() {
        // period_finish is zero until the first fund, so a late first call
        // must not read the whole Unix epoch as elapsed time.
        let mut pool = Pool { total_weight: 1_000_000, ..Pool::default() };
        pool.update(1_800_000_000).unwrap();
        assert_eq!(pool.reward_per_weight, 0);
        assert_eq!(pool.rollover_scaled, 0);
        assert_eq!(pool.last_update, 0);
    }

    /// Mirrors `fund` on the bare state, so the invariant test exercises the
    /// same rate, leftover and rollover arithmetic the instruction uses.
    fn fund_state(pool: &mut Pool, now: i64, amount: u64) {
        assert!(pool.total_weight > 0);
        pool.update(now).unwrap();
        let mut funded_scaled = (amount as u128) * SCALE + pool.rollover_scaled;
        if now < pool.period_finish {
            funded_scaled += ((pool.period_finish - now) as u128) * pool.reward_rate_scaled;
        }
        pool.reward_rate_scaled = funded_scaled / (WEEK as u128);
        pool.rollover_scaled = funded_scaled % (WEEK as u128);
        pool.last_update = now;
        pool.period_finish = now + WEEK;
    }

    #[test]
    fn funding_early_folds_the_leftover_instead_of_dropping_it() {
        // Funding on the owner's own rhythm rather than on the contract's is
        // the point of the flexible window, so the half of the first pour that
        // has not been paid out yet must survive the second pour.
        let mut pool = Pool { total_weight: 1, ..Pool::default() };
        let per_pour: u64 = WEEK as u64; // makes the rate exactly one per second

        fund_state(&mut pool, 0, per_pour);
        assert_eq!(pool.reward_rate_scaled, SCALE);
        assert_eq!(pool.period_finish, WEEK);

        // Halfway in, with half the pour still undelivered.
        fund_state(&mut pool, WEEK / 2, per_pour);
        assert_eq!(pool.reward_rate_scaled, SCALE + SCALE / 2, "leftover was dropped");
        assert_eq!(pool.period_finish, WEEK / 2 + WEEK);

        // Let the second window run out, then read what a lone staker earned.
        pool.update(WEEK / 2 + WEEK).unwrap();
        let mut position = Position { amount: 1, ..Position::default() };
        settle(&pool, &mut position).unwrap();
        assert_eq!(
            position.accrued,
            2 * per_pour,
            "every funded token must reach the staker"
        );
    }

    #[test]
    fn payouts_never_exceed_what_was_funded() {
        // Deterministic xorshift: a fixed pseudo-random walk over stake,
        // unstake, claim and fund, checking after every step that the pool
        // has never promised more than the reward vault received.
        let mut rng: u64 = 0x5EED_1234_ABCD_0001;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };

        let mut pool = Pool { last_update: 1_000, ..Pool::default() };
        let mut positions = [Position::default(); 6];
        let mut funded: u128 = 0;
        let mut claimed: u128 = 0;
        let mut now: i64 = 1_000;

        for step in 0..4_000u32 {
            now += 1 + (next() % 90_000) as i64;
            let who = (next() % positions.len() as u64) as usize;

            match next() % 4 {
                0 => {
                    let amount = MIN_STAKE + next() % 900_000_000_000;
                    let p = &mut positions[who];
                    pool.update(now).unwrap();
                    settle(&pool, p).unwrap();
                    p.amount += amount;
                    p.unlock_at = now + MIN_HOLD;
                    pool.total_weight += amount as u128;
                }
                1 => {
                    let p = &mut positions[who];
                    pool.update(now).unwrap();
                    settle(&pool, p).unwrap();
                    if now < p.unlock_at || p.amount == 0 {
                        continue;
                    }
                    pool.total_weight -= p.amount as u128;
                    p.amount = 0;
                }
                2 => {
                    let p = &mut positions[who];
                    pool.update(now).unwrap();
                    settle(&pool, p).unwrap();
                    claimed += p.accrued as u128;
                    p.accrued = 0;
                }
                _ => {
                    // Funding is no longer gated on the window running out, so
                    // the walk now also exercises pouring into a live window.
                    if pool.total_weight == 0 {
                        continue;
                    }
                    let amount = 1 + next() % 5_000_000_000_000;
                    fund_state(&mut pool, now, amount);
                    funded += amount as u128;
                }
            }

            // Settle every position at the current time to see the pool's
            // full outstanding liability, then compare it with the deposits.
            let mut shadow = positions;
            let mut probe = pool;
            probe.update(now).unwrap();
            let mut outstanding: u128 = 0;
            for p in shadow.iter_mut() {
                settle(&probe, p).unwrap();
                outstanding += p.accrued as u128;
            }
            assert!(
                claimed + outstanding <= funded,
                "step {step}: owed {} > funded {}",
                claimed + outstanding,
                funded
            );
        }

        assert!(funded > 0 && claimed > 0, "the walk must actually fund and claim");
    }

    #[test]
    fn a_lone_dust_staker_cannot_blow_up_reward_per_weight() {
        // The worst case the minimum stake still allows: one staker holding
        // exactly MIN_STAKE while the entire SOLVE supply is paid out weekly.
        const SUPPLY: u64 = 1_000_000_000_000_000;
        let mut pool = Pool {
            total_weight: MIN_STAKE as u128,
            last_update: 0,
            ..Pool::default()
        };
        let mut now = 0i64;
        for _ in 0..1_000 {
            fund_state(&mut pool, now, SUPPLY);
            now += WEEK;
            pool.update(now).unwrap();
        }
        // Still five orders of magnitude below u128::MAX after a thousand
        // whole-supply epochs, so the accumulator cannot overflow in practice.
        assert!(pool.reward_per_weight < u128::MAX / 10_000);
    }
}
```
