// Builds the portable zip from a finished `tauri build`:
//   <exe> + portable.flag (+ a short readme) → target/release/bundle/portable/*.zip
// The flag makes the launcher keep all data in ./MehburMC next to the exe.
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  readSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";

const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const version = conf.version;
const exeName = `${conf.mainBinaryName}.exe`;
const release = join("target", "release");
const out = join(release, "bundle", "portable");
const stage = join(out, "MehburMC Launcher");

rmSync(out, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });
copyFileSync(join(release, exeName), join(stage, exeName));
writeFileSync(join(stage, "portable.flag"), "");
writeFileSync(
  join(stage, "PORTABLE.txt"),
  [
    "MehburMC Launcher - portable",
    "",
    "TR: Tum veriler bu klasordeki MehburMC\\ altinda tutulur. Klasoru USB'ye tasiyabilirsin.",
    "    Guncellemek icin yeni zip'i bu klasorun uzerine cikar. WebView2 gerekir (Windows 11'de hazir).",
    "EN: All data is kept in MehburMC\\ inside this folder. Move the folder anywhere.",
    "    To update, extract the new zip over this folder. Requires WebView2 (built into Windows 11).",
    "",
  ].join("\r\n"),
);

const zip = join(out, `MehburMC-Launcher_${version}_x64-portable.zip`);
// Windows' own bsdtar writes zip archives with -a. A GNU tar earlier on PATH
// (Git Bash) would silently write a plain tar instead, so call it explicitly.
const winTar = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
const tar = process.platform === "win32" && existsSync(winTar) ? winTar : "bsdtar";
const r = spawnSync(tar, ["-a", "-c", "-f", zip, "-C", out, "MehburMC Launcher"], {
  stdio: "inherit",
});
if (r.status !== 0) process.exit(r.status ?? 1);

const magic = Buffer.alloc(2);
readSync(openSync(zip, "r"), magic, 0, 2, 0);
if (magic.toString("latin1") !== "PK") {
  console.error(`${zip} is not a zip archive`);
  process.exit(1);
}
console.log(`portable: ${zip}`);
