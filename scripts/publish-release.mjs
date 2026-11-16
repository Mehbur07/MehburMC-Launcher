// Publishes a finished, signed `npm run package` build (ARCHITECTURE K62, K63):
//   1. GitHub release vX.Y.Z with only the setup .exe (notes from CHANGELOG.md),
//   2. pre-release vX.Y.Z-linux with the .deb, if `npm run package:linux` built one,
//   3. pre-release vX.Y.Z-macos with the .dmg, .zip and updater .app.tar.gz, if
//      `npm run package:macos` prepared them (K109),
//   4. latest.json (version, notes, signature, url) on the code-free `updater`
//      branch, which installed launchers poll.
// Needs `gh` logged in with push access. Usage: node scripts/publish-release.mjs [--dry-run]
// --dry-run: nothing is published; prints the releases it would create and latest.json.
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const REPO = "Mehbur07/MehburMC-Launcher";
const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const version = conf.version;
const tag = `v${version}`;
const DRY = process.argv.includes("--dry-run");

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { stdio: "inherit", ...opts });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(" ")} failed`);
}

/** Publishing steps: only printed with --dry-run. */
function publish(cmd, args, opts = {}) {
  if (DRY) {
    const shown = args.map((a) => (a.length > 80 ? `${a.slice(0, 77)}...` : a));
    console.log(`[dry-run] ${cmd} ${shown.join(" ")}`);
    return;
  }
  run(cmd, args, opts);
}

/** Creates a pre-release `releaseTag` with `files` unless it already exists. */
function preRelease(releaseTag, title, notes, files) {
  const exists =
    spawnSync("gh", ["release", "view", releaseTag, "--repo", REPO], { stdio: "ignore" }).status ===
    0;
  if (!exists)
    publish("gh", ["release", "create", releaseTag, "--repo", REPO, "--target", "main", "--prerelease",
      "--latest=false", "--title", title, "--notes", notes, ...files]); // prettier-ignore
}

// Release notes: the CHANGELOG section of this version.
const md = readFileSync("CHANGELOG.md", "utf8");
const start = md.indexOf(`## [${version}]`);
if (start < 0) throw new Error(`CHANGELOG.md has no section for ${version}`);
const next = md.indexOf("\n## [", start + 5);
const notes = md
  .slice(start, next < 0 ? undefined : next)
  .split("\n")
  .slice(1)
  .join("\n")
  .trim();

const built = join(
  "target",
  "release",
  "bundle",
  "nsis",
  `${conf.productName}_${version}_x64-setup.exe`,
);
if (!existsSync(built) || !existsSync(`${built}.sig`)) {
  throw new Error(`missing ${built}(.sig); run npm run package with the signing key first`);
}
// GitHub turns spaces in asset names into dots; publish a space-free name.
const asset = `MehburMC-Launcher_${version}_x64-setup.exe`;
const work = mkdtempSync(join(tmpdir(), "mehbur-release-"));
try {
  copyFileSync(built, join(work, asset));
  const notesFile = join(work, "notes.md");
  writeFileSync(notesFile, `${notes}\n\n**İndir:** \`${asset}\`\n`);
  // Re-runs after a later step failed reuse the existing release.
  const exists =
    spawnSync("gh", ["release", "view", tag, "--repo", REPO], { stdio: "ignore" }).status === 0;
  if (!exists)
    publish("gh", ["release", "create", tag, "--repo", REPO, "--target", "main",
    "--title", `MehburMC Launcher ${tag}`, "--notes-file", notesFile, join(work, asset)]); // prettier-ignore

  const platform = {
    signature: readFileSync(`${built}.sig`, "utf8").trim(),
    url: `https://github.com/${REPO}/releases/download/${tag}/${asset}`,
  };
  const platforms = { "windows-x86_64": platform, "windows-x86_64-nsis": platform };

  // The Linux package gets its own pre-release, so that the normal release
  // keeps only the Windows installer (K63) and MehburOS has a fixed URL.
  const deb = join("target", "linux", `${conf.productName}_${version}_amd64.deb`);
  if (existsSync(deb) && existsSync(`${deb}.sig`)) {
    const linuxTag = `${tag}-linux`;
    const linuxAsset = `MehburMC-Launcher_${version}_amd64.deb`;
    copyFileSync(deb, join(work, linuxAsset));
    preRelease(linuxTag, `MehburMC Launcher ${tag} (Linux)`,
      `Linux paketi (Ubuntu 26.04 / MehburOS). Kurulum: \`sudo apt install ./${linuxAsset}\``,
      [join(work, linuxAsset)]); // prettier-ignore
    platforms["linux-x86_64-deb"] = {
      signature: readFileSync(`${deb}.sig`, "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/${linuxTag}/${linuxAsset}`,
    };
    console.log(
      `${DRY ? "[dry-run] would publish" : "published"} ${linuxTag}: ${platforms["linux-x86_64-deb"].url}`,
    );
  } else {
    console.warn("no Linux package for this version: Linux launchers will not see this update");
  }

  // macOS (K106, K109): Apple hesabı yok, uygulama ad-hoc imzalı. Tek bir universal
  // paket hem Apple Silicon hem Intel Mac'e gider; güncelleme .app.tar.gz ile olur.
  const mac = join("target", "macos");
  const macBase = `MehburMC-Launcher_${version}_universal`;
  const macFiles = [`${macBase}.dmg`, `${macBase}.zip`, `${macBase}.app.tar.gz`];
  const macUpdater = join(mac, `${macBase}.app.tar.gz`);
  if (macFiles.every((f) => existsSync(join(mac, f))) && existsSync(`${macUpdater}.sig`)) {
    const macTag = `${tag}-macos`;
    preRelease(macTag, `MehburMC Launcher ${tag} (macOS)`, macNotes(macBase),
      macFiles.map((f) => join(mac, f))); // prettier-ignore
    const macPlatform = {
      signature: readFileSync(`${macUpdater}.sig`, "utf8").trim(),
      url: `https://github.com/${REPO}/releases/download/${macTag}/${macBase}.app.tar.gz`,
    };
    platforms["darwin-aarch64"] = macPlatform;
    platforms["darwin-x86_64"] = macPlatform;
    console.log(`${DRY ? "[dry-run] would publish" : "published"} ${macTag}: ${macPlatform.url}`);
  } else {
    console.warn("no macOS package for this version: Mac launchers will not see this update");
  }

  const latest = {
    version,
    notes,
    pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
    platforms,
  };

  if (DRY) {
    console.log(`[dry-run] latest.json for the updater branch:
${JSON.stringify(latest, null, 2)}`);
    process.exit(0);
  }

  // The updater branch holds nothing but latest.json.
  const branch = join(work, "updater");
  run("git", ["clone", "--quiet", "--depth", "1", "--branch", "updater",
    `https://github.com/${REPO}.git`, branch]); // prettier-ignore
  writeFileSync(join(branch, "latest.json"), `${JSON.stringify(latest, null, 2)}\n`);
  run("git", ["add", "latest.json"], { cwd: branch });
  // Commit as this repository's configured author (the clone has none).
  const identity = ["user.name", "user.email"].flatMap((k) => [
    "-c",
    `${k}=${spawnSync("git", ["config", k], { encoding: "utf8" }).stdout.trim()}`,
  ]);
  run("git", [...identity, "commit", "--quiet", "-m", `updater: ${tag}`], { cwd: branch });
  run("git", ["push", "--quiet", "origin", "updater"], { cwd: branch });
  console.log(`${DRY ? "[dry-run] would publish" : "published"} ${tag}: ${platform.url}`);
} finally {
  rmSync(work, { recursive: true, force: true });
}

/** Notes of the macOS pre-release: download choice + how to get past Gatekeeper. */
function macNotes(macBase) {
  return `macOS paketi (macOS 11 ve sonrası; Apple Silicon ve Intel Mac'te çalışır).

**İndir:** \`${macBase}.dmg\` (açıp uygulamayı Applications klasörüne sürükle) ya da \`${macBase}.zip\`.

Uygulama Apple'a kayıtlı değil (Apple geliştirici hesabı yok), bu yüzden ilk açılışta macOS "geliştiricisi doğrulanamadı" der. Bir kez şunlardan birini yap:
- **macOS 15 ve sonrası:** Uygulamayı açmayı dene, sonra Sistem Ayarları → Gizlilik ve Güvenlik → en altta **Yine de Aç**.
- **macOS 14 ve öncesi:** Applications'ta uygulamaya **Control tuşuna basılı tutarak tıkla** → **Aç** → **Aç**.
- **Her sürümde:** Terminal'de: \`xattr -cr "/Applications/MehburMC Launcher.app"\`

Uygulamayı Applications klasöründen çalıştır (otomatik güncelleme ancak orada çalışır). Minecraft 1.18.2 ve öncesi Apple Silicon Mac'te Rosetta 2 ile açılır; gerekirse launcher kurulum komutunu gösterir.

---

macOS package (macOS 11+, Apple Silicon and Intel). The app is not notarized: on first launch use System Settings → Privacy & Security → **Open Anyway** (macOS 15+), **Control-click → Open** (macOS 14 and earlier), or run \`xattr -cr "/Applications/MehburMC Launcher.app"\`. Run it from Applications so automatic updates work.`;
}
