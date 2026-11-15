// Android APK derlemesi (arm64).
//
//   node scripts/android-build.mjs            → release APK (küçük, test için debug anahtarıyla imzalı)
//   node scripts/android-build.mjs --debug    → debug APK (büyük, hata ayıklama bilgisiyle)
//
// Neden ayrı bir betik? `tauri android build`, Rust kütüphanesini Android
// projesine sembolik bağlantıyla koyar; Windows bu bağlantıya yalnız
// "Geliştirici Modu" açıkken izin verir. Bu betik aynı adımları kopyalayarak
// yapar: Tauri Rust kısmını derler (bağlantı adımı hata verirse yok sayılır),
// .so dosyası jniLibs'e kopyalanır, Gradle Rust adımı atlanarak çalıştırılır.
// Önce oyun motoru dosyaları hazırlanır (scripts/android-engine.mjs).
//
// Ortam: ANDROID_HOME / NDK_HOME tanımlı değilse %LOCALAPPDATA%\Android\Sdk
// ve oradaki en yeni NDK kullanılır. İmza: ANDROID_KEYSTORE,
// ANDROID_KEYSTORE_PASSWORD, ANDROID_KEY_ALIAS (.env'den ya da ortamdan)
// tanımlıysa o anahtar, yoksa Gradle'ın debug anahtarı kullanılır (yalnız test).
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

const debug = process.argv.includes("--debug");
const profile = debug ? "debug" : "release";
const Profile = debug ? "Debug" : "Release";
const root = resolve(import.meta.dirname, "..");
const rustTarget = "aarch64-linux-android";
const abi = "arm64-v8a";
const androidDir = join(root, "src-tauri", "gen", "android");

function fail(msg) {
  console.error(`\n✖ ${msg}`);
  process.exit(1);
}

function run(cmd, args, opts = {}) {
  console.log(`\n▶ ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, { stdio: "inherit", shell: true, ...opts });
  return r.status ?? 1;
}

// .env içindeki ANDROID_* değişkenlerini yükle (gerçek değerler asla koda yazılmaz).
const envFile = join(root, ".env");
if (existsSync(envFile)) {
  for (const line of readFileSync(envFile, "utf8").split(/\r?\n/)) {
    const m = /^\s*(ANDROID_[A-Z_]+)\s*=\s*(.*)\s*$/.exec(line);
    if (m && !process.env[m[1]]) process.env[m[1]] = m[2].replace(/^["']|["']$/g, "");
  }
}

// Android SDK / NDK konumları
const sdk =
  process.env.ANDROID_HOME || join(process.env.LOCALAPPDATA || homedir(), "Android", "Sdk");
if (!existsSync(sdk)) fail(`Android SDK bulunamadı: ${sdk}`);
let ndk = process.env.NDK_HOME;
if (!ndk) {
  const ndkRoot = join(sdk, "ndk");
  const versions = existsSync(ndkRoot) ? readdirSync(ndkRoot).sort() : [];
  if (versions.length === 0) fail("NDK bulunamadı (sdkmanager ile 'ndk;<sürüm>' kurulmalı).");
  ndk = join(ndkRoot, versions.at(-1));
}
const env = { ...process.env, ANDROID_HOME: sdk, NDK_HOME: ndk };
// RAM'i korumak için derleme iş parçacığı sınırı (kullanıcı ayarı yoksa 4).
env.CARGO_BUILD_JOBS ||= "4";

// 0) Oyun motoru dosyaları (K104): yoksa Amethyst APK'sından alınır.
if (run("node", [JSON.stringify(join(root, "scripts", "android-engine.mjs"))]) !== 0) {
  fail("Oyun motoru dosyaları hazırlanamadı.");
}

// 1) Arayüz + Rust kütüphanesi (Tauri). Yalnız sembolik bağlantı hatası kabul edilir.
const so = join(root, "target", rustTarget, profile, "libmehbur_launcher_lib.so");
const tauriArgs = ["tauri", "android", "build", "--apk", "--target", "aarch64"];
if (debug) tauriArgs.push("--debug");
console.log(`
▶ npx ${tauriArgs.join(" ")}`);
const tauri = spawnSync("npx", tauriArgs, {
  cwd: root,
  env,
  shell: true,
  encoding: "utf8",
  maxBuffer: 256 * 1024 * 1024,
});
process.stdout.write(tauri.stdout ?? "");
process.stderr.write(tauri.stderr ?? "");
if (tauri.status !== 0) {
  // Rust derlendi ama yalnız bağlantı adımı mı başarısız oldu?
  const out = `${tauri.stdout}${tauri.stderr}`;
  // ANSI renk kodlarını (terminal renkleri) temizle.
  // eslint-disable-next-line no-control-regex
  const plain = out.replace(/\u001b\[[0-9;]*m/g, "");
  const onlySymlink = /symbolic link/i.test(plain) && !/^error(\[E\d+\])?:/m.test(plain);
  if (!onlySymlink || !existsSync(so)) fail("Rust derlemesi başarısız (yukarıdaki hatalara bak).");
}

// 2) Kütüphaneyi Android projesine kopyala.
const jniDir = join(androidDir, "app", "src", "main", "jniLibs", abi);
mkdirSync(jniDir, { recursive: true });
copyFileSync(so, join(jniDir, "libmehbur_launcher_lib.so"));

// 3) Gradle (Rust adımı atlanır, kütüphane zaten yerinde).
const gradlew = `"${join(androidDir, process.platform === "win32" ? "gradlew.bat" : "gradlew")}"`;
const gradleStatus = run(
  gradlew,
  [`assembleArm64${Profile}`, "-x", `rustBuildArm64${Profile}`, "--console=plain"],
  { cwd: androidDir, env },
);
if (gradleStatus !== 0) fail("Gradle derlemesi başarısız.");

const outDir = join(androidDir, "app", "build", "outputs", "apk", "arm64", profile);
const version = JSON.parse(
  readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
).version;
const dist = join(root, "target", "android");
mkdirSync(dist, { recursive: true });

if (debug) {
  const target = join(dist, `MehburMC-${version}-arm64-debug.apk`);
  copyFileSync(join(outDir, "app-arm64-debug.apk"), target);
  console.log(`\n✔ ${target}`);
  process.exit(0);
}

// 4) Release: zipalign + imza.
const buildTools = join(sdk, "build-tools");
const btVersion = readdirSync(buildTools).sort().at(-1);
const bt = (name) =>
  join(buildTools, btVersion, process.platform === "win32" ? `${name}.bat` : name);
const zipalign = join(
  buildTools,
  btVersion,
  process.platform === "win32" ? "zipalign.exe" : "zipalign",
);
const unsigned = join(outDir, "app-arm64-release-unsigned.apk");
const aligned = join(dist, "aligned.apk");
if (run(`"${zipalign}"`, ["-f", "-p", "4", `"${unsigned}"`, `"${aligned}"`]) !== 0)
  fail("zipalign başarısız.");

let keystore = process.env.ANDROID_KEYSTORE;
let storePass = process.env.ANDROID_KEYSTORE_PASSWORD;
let alias = process.env.ANDROID_KEY_ALIAS;
let suffix = "";
if (!keystore) {
  // Yalnız test: Gradle'ın her bilgisayarda kendiliğinden oluşturduğu debug anahtarı.
  keystore = join(homedir(), ".android", "debug.keystore");
  storePass = "android";
  alias = "androiddebugkey";
  suffix = "-test";
  console.warn(
    "\n! ANDROID_KEYSTORE tanımlı değil: APK debug anahtarıyla imzalanıyor (yalnız test için).",
  );
}
if (!existsSync(keystore)) fail(`İmza anahtarı bulunamadı: ${keystore}`);
const target = join(dist, `MehburMC-${version}-arm64${suffix}.apk`);
// Şifre komut satırına yazılmaz, ortam değişkeninden okunur (env:).
const signEnv = { ...env, MEHBUR_APK_PASS: storePass };
const signStatus = run(
  `"${bt("apksigner")}"`,
  [
    "sign",
    "--ks",
    `"${keystore}"`,
    "--ks-key-alias",
    alias,
    "--ks-pass",
    "env:MEHBUR_APK_PASS",
    "--out",
    `"${target}"`,
    `"${aligned}"`,
  ],
  { env: signEnv },
);
rmSync(aligned, { force: true });
if (signStatus !== 0) fail("apksigner başarısız.");
console.log(`\n✔ ${target}`);
