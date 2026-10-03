# MehburMC Launcher

Windows öncelikli, hafif, reklamsız ve telemetrisiz bir Minecraft Java Edition launcher'ı.
Tauri 2 (Rust) + React + TypeScript ile geliştirilmektedir.

> **Durum:** Faz 6 — Modrinth mod tarayıcısı (mod, modpack, resource pack, shader), bağımlılık çözümü, güncelleme denetimi, `.mrpack` ve CurseForge (kendi API anahtarınızla) modpack içe aktarma. Loader'lar Faz 4'te; Microsoft girişi kaldırıldı (yalnızca offline hesaplar).

- Mimari ve kararlar: [ARCHITECTURE.md](ARCHITECTURE.md)

## Planlanan özellikler

- Offline hesaplar (çoklu hesap); Microsoft/premium girişi yok
- İzole instance'lar (her profil kendi `mods`, `saves`, `config` klasörüyle)
- Vanilla, Fabric, Quilt, Legacy Fabric, Forge, NeoForge; OptiFine içe aktarma + Sodium/Iris önerileri
- Modrinth tarayıcısı, `.mrpack` ve CurseForge (kendi API anahtarınızla) modpack içe aktarma
- Otomatik Java (Adoptium Temurin) yönetimi
- Skin/cape yöneticisi ve 3B önizleme
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
npm run lint               # ESLint
cargo run -p launcher-cli -- paths   # çekirdeği UI olmadan dene
```

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
