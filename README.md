# MehburMC Launcher

Windows için hafif, reklamsız ve telemetrisiz bir Minecraft Java Edition launcher'ı.

## ⬇️ İndir

**[En son sürümü indir (Releases)](https://github.com/Mehbur07/MehburMC-Launcher/releases/latest)**
→ sayfadaki **`MehburMC-Launcher_<sürüm>_x64-setup.exe`** dosyasını indirip çalıştır.

"Source code" dosyaları launcher değildir, kaynak koddur; oynamak için gerekmez. Launcher kendini
güncel tutar: yeni sürüm çıkınca başlık çubuğunda **Güncelle** butonu belirir.

---

Tauri 2 (Rust) + React + TypeScript ile geliştirilmektedir.

- **Kullanım kılavuzu:** [docs/KULLANIM.md](docs/KULLANIM.md)
- **Sorun giderme:** [docs/SORUN_GIDERME.md](docs/SORUN_GIDERME.md)
- **Geliştirici rehberi:** [docs/GELISTIRME.md](docs/GELISTIRME.md)
- Mimari ve kararlar: [ARCHITECTURE.md](ARCHITECTURE.md) · Güvenlik: [SECURITY.md](SECURITY.md) · [CHANGELOG.md](CHANGELOG.md)

## Özellikler

- Offline hesaplar (çoklu hesap)
- İzole instance'lar (her profil kendi `mods`, `saves`, `config` klasörüyle)
- Vanilla, Fabric, Quilt, Legacy Fabric, Forge, NeoForge; OptiFine içe aktarma + Sodium/Iris önerileri
- Modrinth tarayıcısı, `.mrpack` ve CurseForge (kendi API anahtarınızla) modpack içe aktarma
- Otomatik Java (Adoptium Temurin) yönetimi
- Skin/cape yöneticisi, hazır skin/pelerinler, 3B önizlemeli piksel editörü
- Neon cyan × siyah tema, TR/EN arayüz, portable mod

## Geliştirme gereksinimleri

- Rust (stable, `rustup`), MSVC Build Tools (C++ iş yükü)
- Node.js 20+
- WebView2 (Windows 11'de hazır gelir)

```sh
npm install
npm run tauri dev          # uygulamayı geliştirme modunda aç
cargo test --workspace     # Rust testleri (+ src/lib/ipc/bindings TS tiplerini üretir)
cargo clippy --workspace --all-targets -- -D warnings
npm test                   # Vitest
npm run coverage           # frontend kapsamı (Rust: cargo llvm-cov)
npm run lint               # ESLint
cargo run -p launcher-cli -- paths   # çekirdeği UI olmadan dene
npm run package            # imzalı NSIS kurulum + portable zip (aşağıya bakın)
```

### Paketleme ve yayın

`npm run package` → `target/release/bundle/nsis/*-setup.exe` (+ updater için `.sig`) ve
`target/release/bundle/portable/*-portable.zip`. Updater imzası için ortam değişkenleri gerekir:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw "$HOME\.tauri\mehburmc-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content -Raw "$HOME\.tauri\mehburmc-updater.password"
npm run package
```

Yayınlamak için `node scripts/publish-release.mjs`: GitHub sürümüne yalnızca kurulum `.exe`'sini
koyar, güncelleme bilgisini (`latest.json`) `updater` dalına yazar.

### Çekirdek CLI (`mehbur-cli`)

```sh
cargo run -p launcher-cli -- versions --type release --limit 10
cargo run -p launcher-cli -- java list
cargo run -p launcher-cli -- java install 21
cargo run -p launcher-cli -- launch 26.3 --offline Steve            # indir + başlat
cargo run -p launcher-cli -- launch 1.12.2 --offline Steve --dry-run  # yalnızca komut satırı
cargo run -p launcher-cli -- launch 26.3 --offline Steve --exit-when-ready 10  # test: ana menüde kapat
cargo run -p launcher-cli -- launch 26.3 --offline Steve --verify    # tüm dosyaları SHA-1 ile onar
cargo run -p launcher-cli -- loader list forge 1.20.1                 # loader sürümleri
cargo run -p launcher-cli -- launch 1.20.1 --loader forge --exit-when-ready 5
cargo run -p launcher-cli -- launch 26.3 --loader fabric:0.19.5      # belirli loader sürümü
cargo run -p launcher-cli -- loader import-optifine OptiFine_1.20.1_HD_U_I6.jar
cargo run -p launcher-cli -- launch 1.20.1 --loader optifine
cargo run -p launcher-cli -- content search sodium --mc 1.20.1 --loader fabric
cargo run -p launcher-cli -- modpack import paket.mrpack           # yeni profil
cargo run -p launcher-cli -- content scan <profil-id> --updates
```

Veri klasörü: `%APPDATA%\MehburMC\game\mc\`. Exe'nin yanına boş bir `portable.flag` dosyası koyarsanız veri exe klasöründeki `MehburMC\game\mc\` altında tutulur.

## Yasal

MehburMC Launcher, Mojang Studios veya Microsoft ile bağlantılı değildir. Oyun dosyaları yalnızca resmî kaynaklardan indirilir; launcher hiçbir telifli varlık dağıtmaz.
