// Android oyun motoru dosyaları (K104).
//
//   node scripts/android-engine.mjs        → gerekirse indirir ve yerleştirir
//   node scripts/android-engine.mjs --force
//
// Minecraft'ı telefonda çalıştıran hazır parçalar (Java köprüsü, grafik
// çevirmenleri, Android LWJGL, Caciocavallo...) Amethyst'in resmi 1.1.7
// APK'sından alınır. Bu kütüphaneler LGPL/MIT/BSD lisanslıdır; kaynakları
// Amethyst deposunda ve README'deki bağlantılardadır. APK'nın SHA-256 parmak
// izi sabittir: GitHub'daki dosya değişirse betik durur.
//
// Çıktılar git'e girmez (.gitignore):
//   src-tauri/gen/android/mcengine/src/main/jniLibs/arm64-v8a/*.so
//   src-tauri/gen/android/mcengine/src/main/assets/engine/**
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cpSync,
  createReadStream,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const AMETHYST = {
  version: "1.1.7",
  url: "https://github.com/AngelAuraMC/Amethyst-Android/releases/download/1.1.7/Amethyst.apk",
  sha256: "6b5579e93f50a62f4d96279683bb447325e5a4de9015deae787eee56c248211f",
};
// Paketleme düzeni değişince artırılır: telefondaki kopya yenilenir.
const LAYOUT = 1;

const LIBS = [
  "libpojavexec.so", // Java'yı başlatan ve girdi/pencere köprüsü (LGPL-3.0)
  "libpojavexec_awt.so",
  "libexithook.so",
  "libbytehook.so", // exithook'un bağımlılığı (MIT)
  "libawt_headless.so",
  "libawt_xawt.so",
  "libopenal.so", // ses (LGPL-2.0)
  "libjnidispatch.so", // JNA 7.0.0 (Apache-2.0/LGPL-2.1)
  "libmobileglues.so", // 1.17+ grafik çevirmeni (LGPL-2.1)
  "libng_gl4es.so", // eski sürümler için grafik çevirmeni (MIT)
];
const COMPONENTS = [
  "lwjgl3/3.3.3",
  "lwjgl3/3.4.1",
  "lwjgl-3.3.3-natives/arm64-v8a",
  "lwjgl-3.4.1-natives/arm64-v8a",
  "caciocavallo",
  "caciocavallo17",
  "methods_injector_agent",
  "security",
];

const root = resolve(import.meta.dirname, "..");
const engine = join(root, "src-tauri", "gen", "android", "mcengine", "src", "main");
const jniDir = join(engine, "jniLibs", "arm64-v8a");
const assetDir = join(engine, "assets", "engine");
const cacheDir = join(root, "target", "android", "engine-cache");
const apk = join(cacheDir, `Amethyst-${AMETHYST.version}.apk`);
const stampFile = join(assetDir, "version");
const stamp = `amethyst-${AMETHYST.version}-${AMETHYST.sha256.slice(0, 8)}-layout${LAYOUT}`;

function fail(msg) {
  console.error(`\n✖ ${msg}`);
  process.exit(1);
}

function sha256(file) {
  return new Promise((ok, bad) => {
    const h = createHash("sha256");
    createReadStream(file)
      .on("data", (d) => h.update(d))
      .on("end", () => ok(h.digest("hex")))
      .on("error", bad);
  });
}

const force = process.argv.includes("--force");
const complete =
  existsSync(stampFile) &&
  readFileSync(stampFile, "utf8").trim() === stamp &&
  LIBS.every((l) => existsSync(join(jniDir, l)));
if (complete && !force) {
  console.log(`Motor dosyaları hazır (${stamp}).`);
  process.exit(0);
}

mkdirSync(cacheDir, { recursive: true });
if (!existsSync(apk) || (await sha256(apk)) !== AMETHYST.sha256) {
  console.log(`Amethyst ${AMETHYST.version} indiriliyor (~156 MB)…`);
  const res = await fetch(AMETHYST.url, { redirect: "follow" });
  if (!res.ok) fail(`İndirme başarısız: HTTP ${res.status}`);
  writeFileSync(apk, Buffer.from(await res.arrayBuffer()));
}
const got = await sha256(apk);
if (got !== AMETHYST.sha256) {
  rmSync(apk, { force: true });
  fail(`APK parmak izi uyuşmuyor (beklenen ${AMETHYST.sha256}, gelen ${got}).`);
}

// APK bir zip dosyası; Windows'ta yerleşik bsdtar, diğerlerinde unzip açar.
const work = join(tmpdir(), `mehbur-engine-${process.pid}`);
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const members = [
  ...LIBS.map((l) => `lib/arm64-v8a/${l}`),
  ...COMPONENTS.map((c) => `assets/components/${c}/*`),
  "assets/resolv.conf",
  "assets/licenses/*",
];
const r =
  process.platform === "win32"
    ? spawnSync(join(process.env.SystemRoot || "C:\\Windows", "System32", "tar.exe"), [
        "-xf",
        apk,
        "-C",
        work,
        ...members,
      ])
    : spawnSync("unzip", ["-q", "-o", apk, ...members, "-d", work]);
if (r.status !== 0) fail(`APK açılamadı: ${r.stderr?.toString() || r.error}`);

rmSync(jniDir, { recursive: true, force: true });
rmSync(assetDir, { recursive: true, force: true });
mkdirSync(jniDir, { recursive: true });
for (const l of LIBS) {
  const from = join(work, "lib", "arm64-v8a", l);
  if (!existsSync(from)) fail(`APK'da ${l} yok.`);
  cpSync(from, join(jniDir, l));
}
for (const c of COMPONENTS) {
  const from = join(work, "assets", "components", c);
  if (!existsSync(from)) fail(`APK'da components/${c} yok.`);
  cpSync(from, join(assetDir, "components", c), { recursive: true });
}
cpSync(join(work, "assets", "resolv.conf"), join(assetDir, "components", "resolv.conf"));
cpSync(join(work, "assets", "licenses"), join(assetDir, "licenses"), { recursive: true });
writeFileSync(stampFile, `${stamp}\n`);
rmSync(work, { recursive: true, force: true });
console.log(`Motor dosyaları yerleştirildi (${stamp}).`);
