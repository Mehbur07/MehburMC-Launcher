// macOS paketini hazırlar (ARCHITECTURE K106, K109). Mac olmadan derlenemediği
// için uygulama CI'daki macOS işinde derlenir; bu betik o işin dosyalarını
// indirir ve otomatik güncelleme dosyasını BU bilgisayarda imzalar (imza
// anahtarı CI'a hiç verilmez):
//   1. HEAD commit'inin başarılı CI çalışmasını bulur (ya da --run <id>),
//   2. "MehburMC-Launcher-macos" dosyalarını indirir, commit ve sürümü doğrular,
//   3. .dmg / .zip / .app.tar.gz dosyalarını target/macos/ altına koyar,
//   4. .app.tar.gz'yi sürüme bağlı imzalar (.sig; `tauri signer sign --app-version`).
// Anahtar: TAURI_SIGNING_PRIVATE_KEY(_PATH) + TAURI_SIGNING_PRIVATE_KEY_PASSWORD
// ortam değişkenleri; yoksa ~/.tauri/mehburmc-updater.key ve .password.
// Sonra: node scripts/publish-release.mjs (vX.Y.Z-macos ön sürümü + latest.json).
// Kullanım: node scripts/package-macos.mjs [--run <CI çalışma no>]
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

const REPO = "Mehbur07/MehburMC-Launcher";
const ARTIFACT = "MehburMC-Launcher-macos";
const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const version = conf.version;
const base = `MehburMC-Launcher_${version}_universal`;
const FILES = [`${base}.dmg`, `${base}.zip`, `${base}.app.tar.gz`];

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { encoding: "utf8", ...opts });
  if (r.status !== 0)
    throw new Error(`${cmd} ${args[0]} failed${r.stderr ? `: ${r.stderr.trim()}` : ""}`);
  return (r.stdout ?? "").trim();
}

const argRun = process.argv.indexOf("--run");
const head = run("git", ["rev-parse", "HEAD"]);
let runId = argRun > 0 ? process.argv[argRun + 1] : null;
let commit = head;
if (!runId) {
  // Yalnız başarılı ve bu commit'e ait bir çalışma: yayınlanan paket kodla birebir aynı olsun.
  const list = JSON.parse(
    run("gh", ["run", "list", "--repo", REPO, "--workflow", "CI", "--commit", head, "--status", "success",
      "--limit", "1", "--json", "databaseId"]), // prettier-ignore
  );
  if (!list.length)
    throw new Error(
      `no successful CI run for ${head.slice(0, 7)}; push it and wait for CI (or pass --run <id>)`,
    );
  runId = String(list[0].databaseId);
} else {
  const info = JSON.parse(
    run("gh", ["run", "view", runId, "--repo", REPO, "--json", "headSha,conclusion"]),
  );
  if (info.conclusion !== "success")
    throw new Error(`CI run ${runId} did not succeed (${info.conclusion})`);
  commit = info.headSha;
}

const work = mkdtempSync(join(tmpdir(), "mehbur-macos-"));
try {
  run("gh", ["run", "download", runId, "--repo", REPO, "--name", ARTIFACT, "--dir", work]);
  const built = readFileSync(join(work, "commit.txt"), "utf8").trim();
  if (built !== commit)
    throw new Error(`artifact was built from ${built.slice(0, 7)}, expected ${commit.slice(0, 7)}`);
  if (commit !== head)
    console.warn(
      `note: packaging run ${runId} (${commit.slice(0, 7)}), not HEAD (${head.slice(0, 7)})`,
    );
  for (const f of FILES)
    if (!existsSync(join(work, f)))
      throw new Error(`artifact has no ${f} (built for another version?)`);

  const out = join("target", "macos");
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  for (const f of FILES) copyFileSync(join(work, f), join(out, f));

  // İmza: anahtar ve şifre yalnız ortam değişkeniyle geçer (komut satırında görünmez).
  const env = { ...process.env };
  if (!env.TAURI_SIGNING_PRIVATE_KEY && !env.TAURI_SIGNING_PRIVATE_KEY_PATH) {
    const keyPath = join(homedir(), ".tauri", "mehburmc-updater.key");
    if (!existsSync(keyPath))
      throw new Error("no updater signing key (TAURI_SIGNING_PRIVATE_KEY or ~/.tauri)");
    env.TAURI_SIGNING_PRIVATE_KEY_PATH = keyPath;
    const pw = join(homedir(), ".tauri", "mehburmc-updater.password");
    if (env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD === undefined && existsSync(pw))
      env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = readFileSync(pw, "utf8").trim();
  }
  const updater = join(out, `${base}.app.tar.gz`);
  // Tauri CLI doğrudan node ile çalışır (npx Windows'ta kabuk ister, argümanlar kaçışsız kalır).
  const cli = join("node_modules", "@tauri-apps", "cli", "tauri.js");
  run(process.execPath, [cli, "signer", "sign", "--app-version", version, updater], { env });
  if (!existsSync(`${updater}.sig`)) throw new Error(`signing did not produce ${updater}.sig`);

  console.log(`macOS ${version} from CI run ${runId} (${commit.slice(0, 7)}) ready in ${out}:`);
  for (const f of [...FILES, `${base}.app.tar.gz.sig`]) console.log(`  ${f}`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
