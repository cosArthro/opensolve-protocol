// АВАРИЙНОЕ ВОССТАНОВЛЕНИЕ кошелька B в файл ключа для консоли.
//
// Кошелёк B (fDUecC…) выведен из сид-фразы способом Phantom/Solflare
// (BIP44 m/44'/501'/0'/0'). Команда `solana program deploy` умеет подписывать
// только файлом, поэтому в день, когда программу надо будет обновить, фразу
// нужно превратить в файл. Этот скрипт ровно это и делает.
//
//   node recover-to-keypair.mjs <куда-положить-файл.json> [ожидаемый-адрес]
//
// Фраза читается с клавиатуры, не из аргументов — в историю не попадает.
// Сеть не используется. После работы файл ключа удалить.
import crypto from "node:crypto";
import fs from "node:fs";
import readline from "node:readline";
import { createRequire } from "node:module";

// Resolved against this file so the tool works from any working directory.
const require = createRequire(import.meta.url);
const { Keypair } = require("@solana/web3.js");

const outPath = process.argv[2];
const expected = process.argv[3] ?? "fDUecCxXAGviwa5ZDxDH7MgDyDu2AVBptXXC7F9LX6i";

if (!outPath) {
  console.log("Использование: node recover-to-keypair.mjs <файл.json> [адрес]");
  process.exit(1);
}
if (fs.existsSync(outPath)) {
  console.log(`Файл ${outPath} уже существует. Перезаписывать не буду.`);
  process.exit(1);
}

const hmac = (key, data) => crypto.createHmac("sha512", key).update(data).digest();

// BIP44 m/44'/501'/0'/0' по SLIP-0010 для ed25519 — так считают Phantom и Solflare.
function phantomKeypair(phrase) {
  const seed = crypto.pbkdf2Sync(phrase.normalize("NFKD"), "mnemonic", 2048, 64, "sha512");
  let I = hmac(Buffer.from("ed25519 seed"), seed);
  for (const idx of [44, 501, 0, 0]) {
    const d = Buffer.alloc(37);
    I.copy(d, 1, 0, 32);
    d.writeUInt32BE((idx | 0x80000000) >>> 0, 33);
    I = hmac(I.subarray(32), d);
  }
  return Keypair.fromSeed(I.subarray(0, 32));
}

const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
rl.question("Сид-фраза с бумаги, слова через пробел:\n> ", (answer) => {
  rl.close();
  const phrase = answer.trim().toLowerCase().replace(/\s+/g, " ");
  const kp = phantomKeypair(phrase);
  const got = kp.publicKey.toBase58();

  console.log(`\nожидали : ${expected}`);
  console.log(`вышло   : ${got}`);

  if (got !== expected) {
    console.log("\nНЕ СОВПАЛО. Файл не записан. Проверь слова по бумаге.");
    process.exit(1);
  }

  fs.writeFileSync(outPath, JSON.stringify(Array.from(kp.secretKey)));
  console.log(`\nСовпало. Ключ записан в ${outPath}`);
  console.log("Этим файлом можно подписывать: solana program deploy --upgrade-authority <файл>");
  console.log("ПОСЛЕ РАБОТЫ ФАЙЛ УДАЛИТЬ.");
});
