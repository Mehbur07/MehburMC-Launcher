# MehburMC Launcher

Windows öncelikli, hafif, reklamsız ve telemetrisiz bir Minecraft Java Edition launcher'ı.
Tauri 2 (Rust) + React + TypeScript ile geliştirilmektedir.

> **Durum:** Faz 2 — çekirdek: vanilla sürümler CLI ile offline başlatılabiliyor. Arayüzden başlatma Faz 3'te.

- Mimari ve kararlar: [ARCHITECTURE.md](ARCHITECTURE.md)

## Planlanan özellikler

- Offline mod + Microsoft (premium) girişi, çoklu hesap
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
```

Veri klasörü: `%APPDATA%\MehburMC\game\mc\`. Exe'nin yanına boş bir `portable.flag` dosyası koyarsanız veri exe klasöründeki `MehburMC\game\mc\` altında tutulur.

## Yasal

MehburMC Launcher, Mojang Studios veya Microsoft ile bağlantılı değildir. Oyun dosyaları yalnızca resmî kaynaklardan indirilir; launcher hiçbir telifli varlık dağıtmaz.
