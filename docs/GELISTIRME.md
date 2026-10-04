# Geliştirici Rehberi

Mimari ve tüm kararlar: [ARCHITECTURE.md](../ARCHITECTURE.md). Bu belge günlük çalışmayı anlatır.

## Gereksinimler

- Rust stable (`rustup`), MSVC Build Tools (C++ iş yükü)
- Node.js 22+
- WebView2 (Windows 11'de hazır)
- İsteğe bağlı: `cargo install cargo-audit cargo-llvm-cov` (+ `rustup component add llvm-tools-preview`)

## Yapı

```
crates/launcher-core   UI'dan bağımsız tüm iş mantığı (indirme, sürümler, loader'lar, içerik, skin, crash…)
crates/launcher-cli    geliştirici CLI'ı (mehbur-cli) — çekirdeği arayüz olmadan dener
src-tauri              ince Tauri katmanı: komutlar (commands/*.rs), olay köprüsü, pencere
src                    React arayüzü (features/*, stores/*, lib/ipc)
scripts                paketleme yardımcıları
```

Kurallar:
- İş mantığı yalnızca `launcher-core`'da; Tauri komutları doğrula → çağır → hatayı çevir.
- Tüm yollar `Paths`'ten gelir; testlerde `Paths::at(tempdir)`.
- Rust DTO'larından TypeScript tipleri `ts-rs` ile `cargo test` sırasında
  `src/lib/ipc/bindings/`'e üretilir ve **commit'lenir** (CI kontrol eder).
- Yeni komut: `src-tauri/build.rs` listesine + `capabilities/default.json`'a `allow-…` +
  `lib.rs` `generate_handler!` + `src/lib/ipc/index.ts` sarmalayıcısı.
- Webview hiçbir komuta **dosya yolu göndermez**: dosya seçimi `pick_path(purpose)` ile Rust'ta
  yapılır, komut seçilen yolu `take_pick` ile alır (bkz. `commands/dialogs.rs`, K54).
- Hata kodları `CoreError::code()` → `errors.<kod>` (tr.json ve en.json; anahtar eşitliği test edilir).

## Komutlar

```sh
npm install
npm run tauri dev                  # geliştirme (Vite :1420 + debug exe)
cargo run -p mehbur-launcher       # Vite zaten çalışıyorsa yalnızca uygulama

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace             # birim + entegrasyon (wiremock) + TS tipleri
cargo llvm-cov -p launcher-core --summary-only
cargo audit

npm run lint && npx tsc --noEmit && npm run format:check
npm test                           # Vitest + Testing Library
npm run coverage                   # frontend kapsamı

npm run package                    # imzalı NSIS + portable zip (README'ye bakın)
```

Çekirdek CLI örnekleri README'de. CLI aynı veri klasörünü kullanır; `--dry-run` oyunu başlatmadan
komut satırını üretir.

## Testler

| Katman | Nerede | Not |
|---|---|---|
| Çekirdek birim | `crates/launcher-core/src/**` (`#[cfg(test)]`) | `tempfile`, sahte HTTP için `Allowlist::with_loopback()` |
| Çekirdek entegrasyon | `crates/launcher-core/tests/` | `launch_pipeline` (manifest→jar/lib/asset→komut satırı, çevrimdışı), `content_install` (Modrinth + bağımlılık + hash), `java_install` (Adoptium) |
| Tauri | `src-tauri/src/**` | dosya adı temizleme vb. |
| Frontend | `src/**/*.test.ts(x)` | IPC `vi.mock("…/lib/ipc")` ile sahte; `src/test/fixtures.ts` |

Arayüzü elle test ederken: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`
ile başlatıp Chrome DevTools Protocol üzerinden ekran görüntüsü alın; başka pencereleri öne
getirmeyin. Tek-örnek kilidi nedeniyle **çalışan başka bir launcher varken test kopyası
başlatmayın** (o pencere öne gelir).

## Sürüm yayınlama

1. `src-tauri/tauri.conf.json`, `Cargo.toml` (workspace) ve `package.json` sürümünü artır,
   `CHANGELOG.md`'yi güncelle.
2. `vX.Y.Z` tag'ini push'la → `Release` iş akışı taslak release oluşturur (NSIS, `.sig`,
   `latest.json`, portable zip).
3. Taslağı yayınla. (Güncelleme kanalı yalnızca depo herkese açıkken çalışır.)

İmza anahtarı repoda **yoktur**: GitHub Secrets (`TAURI_SIGNING_PRIVATE_KEY`,
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) ve geliştiricinin `~/.tauri/` klasörü.
Anahtar kaybolursa mevcut kurulumlar yeni güncellemeleri doğrulayamaz.
