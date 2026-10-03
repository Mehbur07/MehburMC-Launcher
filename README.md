# MehburMC Launcher

Windows öncelikli, hafif, reklamsız ve telemetrisiz bir Minecraft Java Edition launcher'ı.
Tauri 2 (Rust) + React + TypeScript ile geliştirilmektedir.

> **Durum:** Faz 0 — mimari tasarım. Henüz çalıştırılabilir bir sürüm yok.

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

Derleme ve geliştirme komutları Faz 1'de eklenecek.

## Yasal

MehburMC Launcher, Mojang Studios veya Microsoft ile bağlantılı değildir. Oyun dosyaları yalnızca resmî kaynaklardan indirilir; launcher hiçbir telifli varlık dağıtmaz.
