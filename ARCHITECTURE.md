# MehburMC Launcher — Mimari

> Durum: **Onaylandı (2026-10-03). Faz 9 tamamlandı (0.1.0 sürüm adayı); Faz 5 iptal edildi (K39).** Bu belge yaşayan bir belgedir; her fazda alınan kararlar "Karar Kaydı" bölümüne eklenir.

MehburMC Launcher; Windows öncelikli (Linux/macOS'a taşınabilir), açık mimarili, reklamsız, telemetrisiz bir Minecraft Java Edition launcher'ıdır. SKLauncher'ın özellik zenginliğini (offline hesaplar, skin/cape, modpack, loader desteği, portable) ve Legacy Launcher'ın hafifliğini / izole profil yapısını birleştirir.

---

## 1. Teknoloji Seçimi

| Katman | Seçim | Gerekçe |
|---|---|---|
| Kabuk | **Tauri 2** | Sistem WebView2'sini kullanır; ~10 MB kurulum, düşük RAM. Electron'a göre çok daha hafif. |
| Çekirdek | **Rust** (`launcher-core` crate) | İndirme, hash, zip, süreç yönetimi hızlı ve güvenli. UI'dan bağımsız, CLI ile test edilebilir. |
| Async | `tokio`, `tokio-util` (`CancellationToken`) | Paralel indirme, iptal edilebilir görevler, `tokio::process`. |
| HTTP | `reqwest` (rustls, stream) | Stream indirme, `Range` ile devam, redirect denetimi. |
| Serileştirme | `serde`, `serde_json`, `quick-xml` | Mojang/loader JSON'ları, Maven metadata XML. |
| Hash | `sha1`, `sha2` | Mojang SHA1, Modrinth SHA512, Adoptium SHA256. |
| Arşiv | `zip` | Natives, mrpack, Java runtime; zip-slip korumalı sarmalayıcı. |
| Hata | `thiserror` | Türlü hatalar + i18n anahtarı. |
| Log | `tracing`, `tracing-subscriber`, `tracing-appender` | Günlük döndürmeli dosya, token maskeleme katmanı. |
| Yollar | `directories` | Platforma göre veri kökü. |
| Dosya kilidi | `fs4` | Instance ve kurulum kilitleri, boş disk alanı ölçümü. |
| TS tip üretimi | `ts-rs` | Rust DTO'larından TypeScript tipleri; elle senkron tutma yok. |
| Test | `wiremock`, `tempfile`, `insta` (snapshot) | Sahte HTTP, geçici `Paths`, argüman çıktısı snapshot'ları. |
| Frontend | **React 19 + TypeScript + Vite** | |
| Stil | **Tailwind CSS v4** (CSS-first `@theme`) | Token'lar CSS değişkeni, Tailwind bu değişkenlere bağlanır → çalışma anında tema değişimi. |
| Durum | Zustand | Küçük, kalıp gerektirmeyen store'lar. |
| Yönlendirme | Zustand `view` durumu (router yok) | Masaüstünde URL çubuğu yok; düz menü için router gereksiz bağımlılık (K10). |
| Animasyon | Framer Motion (`MotionConfig reducedMotion="user"`) | |
| İkon | lucide-react | |
| i18n | i18next + react-i18next | Anahtar tabanlı TR/EN, varsayılan sistem dili. |
| Liste | `@tanstack/react-virtual` | Sürüm/mod listelerinde sanallaştırma. |
| Sürükle-bırak | `@dnd-kit` | Instance sıralama. |
| 3B skin | `skinview3d` | three.js tabanlı, hazır Minecraft modeli. |
| Font | `@fontsource/rajdhani` (başlık), `@fontsource/orbitron` (yalnızca marka yazısı / OYNA), `@fontsource-variable/inter` (gövde) | npm ile **yerel paketlenir**, CDN yok. Orbitron'da yalnızca latin alt kümesi var (ğ/ş/İ yok), bu yüzden Türkçe başlıklarda Rajdhani (latin-ext) kullanılır (K9). |
| Frontend test | Vitest + Testing Library | |
| Paket | Tauri bundler → **NSIS** installer + portable zip | |
| Güncelleme | `tauri-plugin-updater` (imzalı) | Bkz. Risk R7. |

Tauri eklentileri (en az yetki): `dialog` (dosya seçici; yalnızca Rust'tan çağrılır, K54), `opener` (yalnızca veri klasörünü açma, kapsam sınırlı), `updater`, `single-instance`, `process` (yeniden başlatma). **`shell` eklentisi kullanılmaz**; oyun/Java/installer süreçleri yalnızca Rust tarafından, sabit argüman şablonlarıyla başlatılır.

---

## 2. Depo Yapısı

```
MehburMC-Launcher/
├── Cargo.toml                 # workspace
├── package.json               # frontend (Vite)
├── crates/
│   ├── launcher-core/         # UI'dan bağımsız tüm iş mantığı
│   └── launcher-cli/          # geliştirici CLI'ı: `mehbur-cli launch --version 26.3 --offline Steve`
├── src-tauri/                 # ince sarmalayıcı: komutlar, olay köprüsü, pencere
│   ├── capabilities/          # en az yetkili izin dosyaları
│   └── src/commands/*.rs      # her komut → launcher-core çağrısı
├── src/                       # React uygulaması
│   ├── app/                   # App, router, provider'lar
│   ├── components/            # TitleBar, Button, Card, Badge, Progress, Modal, NeonBackground…
│   ├── features/              # home, instances, wizard, instance-detail, browse, accounts,
│   │                          # skins, downloads, settings, console
│   ├── stores/                # Zustand store'ları
│   ├── lib/ipc/               # tipli invoke/listen sarmalayıcıları (+ ts-rs çıktıları)
│   ├── i18n/                  # tr.json, en.json
│   ├── styles/                # tokens.css, accent presetleri
│   └── assets/                # logo.svg (özgün tasarım)
├── docs/                      # loaders.md, …
└── .github/workflows/         # CI
```

---

## 3. `launcher-core` Modül Şeması

```
launcher-core
├── paths        Paths struct'ı — TEK yol kaynağı (aşağıda §5)
├── settings     settings.json şeması + sürüm göçleri (schema_version)
├── error        CoreError (thiserror) → { code: "net.timeout", params, detail }
├── events       CoreEvent enum + EventSink trait (UI/CLI bağımsız ilerleme yayını)
├── net
│   ├── client     paylaşılan reqwest istemcisi, User-Agent, zaman aşımı
│   ├── allowlist  HTTPS + izinli host (+ yol öneki) kontrolü, redirect'lerde de
│   ├── download   kuyruk, semaphore, .part + Range ile devam, üstel geri çekilme,
│   │              hash doğrulama, atomik taşıma, disk dolu kontrolü, duraklat/iptal
│   └── cache      TTL'li metadata önbelleği (manifest, loader listeleri, ikonlar)
├── version
│   ├── manifest   version_manifest_v2
│   ├── profile    sürüm JSON modelleri (yeni + eski biçimler)
│   ├── merge      inheritsFrom zinciri birleştirme, loader kütüphanesi önceliği
│   └── compare    sürüm karşılaştırma (1.x.y, 26.x, snapshot/pre/rc)
├── rules        os/arch/features kural değerlendirme
├── library      çözümleme, classpath tekilleştirme (group:artifact[:classifier] anahtarı)
├── natives      eski `natives` + yeni classifier biçimi, extract.exclude, zip-slip koruması
├── assets       assetIndex, objects/xx/hash, virtual & map_to_resources düzenleri
├── java         sistem Java taraması, sürüm algılama, Adoptium indirme, major eşleme
├── launch
│   ├── args       placeholder çözümleme; arguments.game/jvm + minecraftArguments
│   ├── logging    log4j config indirme + -Dlog4j.configurationFile
│   ├── cmdline    Windows komut satırı uzunluk sınırı aşımı (bkz. R2)
│   └── process    tokio::process, stdout/stderr akışı, çıkış kodu, crash raporu bulma
├── instance     CRUD, kopyala, içe/dışa aktar, kilit, oynama süresi
├── loader
│   ├── mod.rs       Loader trait + LoaderKind kayıt tablosu
│   ├── vanilla.rs  fabric.rs  quilt.rs  legacy_fabric.rs
│   ├── forge.rs    neoforge.rs  optifine.rs
├── auth
│   ├── offline     ad doğrulama, UUID v3 ("OfflinePlayer:<ad>")
│   └── store       accounts.json (offline hesaplar)
├── content      Modrinth v2, CurseForge (kullanıcı anahtarı), mrpack/CF zip, bağımlılık çözümü
├── skin         kütüphane (library.json + textures/<sha1>.png), PNG doğrulama, slim algılama,
│                varsayılan skinler (client jar), CustomSkinLoader eşitleme (K44, K45, K59)
├── news         Mojang launcher içerik feed'i (hata → boş liste)
└── crash        log/crash-report kalıp eşleme → teşhis ipuçları
```

### Loader trait

```rust
#[async_trait]
pub trait Loader: Send + Sync {
    fn kind(&self) -> LoaderKind;
    /// Bu MC sürümü için kullanılabilir loader sürümleri (latest/recommended işaretli).
    async fn list_versions(&self, ctx: &Ctx, mc: &str) -> Result<Vec<LoaderVersion>>;
    /// Gerekli dosyaları kurar, versions/<id>/<id>.json üretir. İptal edilebilir.
    async fn install(&self, ctx: &Ctx, inst: &Instance, ver: &LoaderVersion,
                     cancel: CancellationToken) -> Result<InstalledProfile>;
    /// Birleştirilmiş, başlatmaya hazır profil (inheritsFrom çözülmüş).
    async fn build_launch_profile(&self, ctx: &Ctx, inst: &Instance) -> Result<LaunchProfile>;
}
```

Yeni loader = `loader/<ad>.rs` + `LoaderKind` enum'una bir satır. `Ctx` paylaşılan `Paths`, HTTP istemcisi, `EventSink` ve ayarları taşır.

### `src-tauri` (ince sarmalayıcı)

- Her `#[tauri::command]` en fazla birkaç satır: argümanı doğrula → `launcher-core` çağır → `UiError`'a dönüştür.
- `TauriEventSink` → `CoreEvent`'leri `app.emit("core://…")` ile yayar. Oyun logları **50 ms'lik paketler** halinde gönderilir (IPC taşmasını önlemek için).
- Uzun görevler `TaskId` döndürür; iptal `cancel_task(id)` ile yapılır.

---

## 4. Veri Akışı

### 4.1 Başlatma ("OYNA")

```
UI: OYNA ──invoke(launch_instance)──▶ src-tauri ──▶ core::launch::prepare
   1. instance kilidi al (zaten çalışıyorsa → hata "instance.already_running")
   2. hesap: offline → UUID v3
   3. profil: loader.build_launch_profile → inheritsFrom birleştir
   4. doğrula/indir: libraries + natives + assetIndex/objects + client.jar + log4j config
        └─ download kuyruğu ──CoreEvent::Progress──▶ UI ilerleme çubuğu
   5. Java: instance override > ayar eşlemesi > runtime/java{major} > Adoptium'dan indir
   6. args: JVM (RAM, ön ayar, kullanıcı) + main class + game args, placeholder çözümle
   7. process::spawn (CREATE_NO_WINDOW) → stdout/stderr ──CoreEvent::GameLog──▶ Konsol
   8. çıkış kodu ≠ 0 → crash-reports/en yeni + log → crash::analyze → ipuçları
   9. kilit bırak, oynama süresini kaydet
```

### 4.2 Çevrimdışı mod

- Ağ yoksa manifest/loader listeleri `cache/`'den okunur (süresi geçmiş olsa bile).
- Kurulu instance: yalnızca dosyaların varlığı ve boyutu kontrol edilir, indirme denenmez.

### 4.3 Hata modeli

`CoreError` → `{ code, params, detail }`. `code`, i18n anahtarıdır (`errors.net.timeout`). UI çevrilmiş mesajı ve **"Detayları kopyala"** butonunu gösterir (`detail` + kod + sürüm bilgisi). `detail` içinde token asla bulunmaz.

---

## 5. Klasör Yapısı ve `Paths`

```
%APPDATA%\MehburMC\                (Linux: ~/.local/share/MehburMC, macOS: ~/Library/Application Support/MehburMC)
└── game\
    └── mc\
        ├── launcher\        settings.json, accounts.json (token YOK), redirect.json (opsiyonel)
        ├── versions\        <id>\<id>.json, <id>.jar, natives\
        ├── libraries\
        ├── assets\          indexes\, objects\, virtual\legacy\, log_configs\
        ├── runtime\         java8\, java17\, java21\, java25\ … (dinamik: java{major})
        ├── instances\       <instance-id>\ instance.json, mods, config, saves, resourcepacks,
        │                    shaderpacks, screenshots, logs, crash-reports, resources (map_to_resources)
        ├── loaders\         forge\, neoforge\ installer önbelleği; fabric\, quilt\ meta
        ├── skins\           skin ve cape önbelleği
        ├── modpacks\        .mrpack / CurseForge zip önbelleği
        ├── cache\           TTL'li metadata, ikon önbelleği, (portable'da) webview\
        └── logs\            launcher-YYYY-MM-DD.log (7 gün tutulur)
```

**Kurallar ve kararlar**

- `MehburMC\` kökünde yalnızca `game\`, `game\` içinde yalnızca `mc\` bulunur.
- **Karar K1:** Launcher ayarları ve hesap metadatası `game\mc\launcher\` altındadır (`settings.json`, `accounts.json`). Böylece kök temiz kalır. Bu klasör özgün ağaca eklenen tek klasördür.
- **Karar K2:** Tüm yollar `Paths` struct'ından gelir (`paths.version_json(id)`, `paths.library(&coord)`, `paths.asset_object(hash)`, `paths.instance_dir(id)`, `paths.runtime(major)` …). Kodda sabit yol yazılmaz; testler `Paths::at(tempdir)` kullanır.
- **Karar K3, çözümleme sırası:**
  1. Exe yanında `portable.flag` varsa → `<exe_dir>\MehburMC\game\mc\`.
  2. Yoksa → `%APPDATA%\MehburMC\game\mc\`.
  3. `launcher\redirect.json` varsa → içerik klasörleri o konuma yönlendirilir ("Veri klasörünü taşı"). `launcher\` her zaman varsayılan konumda kalır; aksi halde taşındıktan sonra ayarların nerede olduğunu bulamayız.
- **Karar K4, veri taşıma:** kopyala → boyut/sayı doğrula → `redirect.json` yaz → eski içeriği sil. Farklı diskler arası çalışır. Taşıma sırasında oyun/indirme çalışamaz.
- **Karar K5, WebView2 verisi:** normal modda Tauri bunu `%LOCALAPPDATA%\<identifier>\EBWebView` altına yazar (Roaming değil, `MehburMC` kökü etkilenmez). Portable modda `game\mc\cache\webview\` altına yönlendirilir. Tauri'nin `app_data_dir` gibi kendi klasör oluşturan API'leri **kullanılmaz**.
- İlk açılışta eksik klasörler oluşturulur. Yazma izni hatası → anlaşılır mesaj + "Portable moda geç" / "Farklı klasör seç" önerisi.
- **Karar K6, instance oyun dizini:** `instances\<id>\` doğrudan `${game_directory}`'dir (`.minecraft` alt klasörü yok); metadata `instance.json`.
- **Karar K7, paylaşılan sürüm dosyaları:** `versions\`, `libraries\`, `assets\` tüm instance'lar arasında paylaşılır (disk tasarrufu). Natives `versions\<id>\natives\` altına çıkarılır (salt okunur kullanım).

---

## 6. Ağ ve İndirme

- **Allowlist (host + isteğe bağlı yol öneki), yalnızca HTTPS, redirect'lerde de uygulanır:**
  - Mojang: `piston-meta.mojang.com`, `piston-data.mojang.com`, `launchermeta.mojang.com`, `launcher.mojang.com`, `libraries.minecraft.net`, `resources.download.minecraft.net`, `launchercontent.mojang.com`
  - Fabric / Quilt / Legacy Fabric: `meta.fabricmc.net`, `maven.fabricmc.net`, `meta.quiltmc.org`, `maven.quiltmc.org`, `meta.legacyfabric.net`, `maven.legacyfabric.net` (→ `repo.legacyfabric.net`'e yönlendirir)
  - Forge / NeoForge: `files.minecraftforge.net`, `maven.minecraftforge.net`, `maven.neoforged.net`, `repo1.maven.org` (eski Forge bağımlılıkları)
  - Modrinth: `api.modrinth.com`, `cdn.modrinth.com` (+ mrpack spec'inin izin verdiği `github.com`, `raw.githubusercontent.com`, `gitlab.com` — tamamı, bkz. K42)
  - CurseForge: `api.curseforge.com`, `edge.forgecdn.net`, `mediafilez.forgecdn.net`
  - Adoptium: `api.adoptium.net`, `github.com/adoptium/*` → `objects.githubusercontent.com` / `release-assets.githubusercontent.com` (doğrulandı: Temurin ikilileri GitHub Releases'tan gelir)
- **Hash doğrulama:** Mojang SHA1; Fabric/Quilt profillerindeki `sha1`/`sha256`/`sha512` (Fabric meta'da mevcut, doğrulandı); Maven'da hash yoksa `<url>.sha1` yan dosyası; Modrinth SHA512; Adoptium SHA256. Hiçbir hash bulunamazsa dosya indirilir ve uyarı loglanır (yalnızca Maven kütüphaneleri için).
- **Dayanıklılık:** `.part` + `Range` ile devam; üstel geri çekilme + jitter (5 deneme); `rename` ile atomik taşıma; indirme öncesi boş alan kontrolü + `StorageFull` / Windows `ERROR_DISK_FULL` yakalama; eşzamanlılık `Semaphore` ile sınırlı (varsayılan 8, ayarlanabilir); görev başına duraklat/devam/iptal.

---

## 7. Kimlik Doğrulama

- **Offline:** ad `^[A-Za-z0-9_]{3,16}$`; UUID = MD5 tabanlı v3 (`OfflinePlayer:<ad>`), Java `UUID.nameUUIDFromBytes` ile birebir; birim testiyle doğrulanır. UI premium/offline ayrımı yapmaz (K59, K61); ad değiştirilebilir (K61).
- **Microsoft girişi yok** (K39): kullanıcı isteğiyle kaldırıldı. Launcher yalnızca offline hesaplarla çalışır; çevrimiçi (online-mode) sunucular desteklenmez.
- **Log maskeleme:** `tracing` katmanı token, `accessToken`, `--accessToken <x>` ve JWT kalıplarını `***` ile değiştirir. Oyun komut satırı loglanırken token argümanı maskelenir.

---

## 8. Tema ve UI

- Token'lar `src/styles/tokens.css` içinde CSS değişkeni olarak tanımlanır, Tailwind v4 `@theme` ile bağlanır:
  `--bg #05070A`, `--surface-1 #0B1117`, `--surface-2 #101A22`, `--accent #00F0FF`, `--accent-2 #00B8D4`, `--accent-glow #7DF9FF`, `--warn #FFB800`, `--danger #FF3B5C`, `--success #00FFA3`.
- Vurgu presetleri: `cyan` (varsayılan), `magenta`, `green`, `purple`. `<html data-accent="…">` ile değişir. Her preset için kontrast WCAG AA'ya göre kontrol edilir.
- Frameless pencere (`decorations: false`, `shadow: true`); özel başlık çubuğunda `data-tauri-drag-region`, küçült/büyüt/kapat.
- Glow'lar ölçülü kullanılır: aktif öğede ince parlayan kenarlık, ilerleme çubuğunda akan ışık, OYNA butonunda nabız efekti. Hepsi `prefers-reduced-motion` ile kapanır.
- Arka plan ızgara/parçacık efekti tek bir `<canvas>` üzerinde çizilir (`requestAnimationFrame`, pencere gizliyken durur, ayardan kapatılabilir).
- Logo: özgün SVG (neon "M" + izometrik küp). Mojang/Minecraft varlığı kullanılmaz.

---

## 9. Güvenlik

- Tauri capabilities en az yetkiyle tanımlanır; `shell` eklentisi yoktur. `opener` yalnızca `Paths` köküne kapsamlıdır.
- CSP: `default-src 'self'; img-src 'self' asset: data:; connect-src ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'`. Uzak görseller (mod ikonları, haber görselleri) Rust tarafından `cache\`'e indirilir ve `asset:` protokolüyle sunulur. Asset kapsamı yalnızca `cache\` ve `skins\` klasörleridir. Webview hiçbir dış adrese doğrudan bağlanmaz.
- Zip çıkarma: her girdi normalleştirilir; mutlak yol, `..` ve sembolik bağlantı içeren girdiler reddedilir; hedef köke göre `starts_with` kontrolü yapılır.
- Süreç başlatma: yalnızca Java ikilileri (`java(w).exe` adı doğrulanır, K55), argümanlar dizi olarak verilir (shell yorumlaması yok); komut çalıştıran JVM seçenekleri reddedilir.
- Dosya yolları webview'dan gelmez: diyaloglar Rust'ta açılır, seçilen yol amaç bazında saklanıp komutça bir kez tüketilir (K54). Webview'a `dialog`/`fs`/`shell`/`http` izni yok.
- Belleğe okunan HTTP yanıtları 64 MB ile sınırlı (K56). Ayrıntılı tehdit modeli: [SECURITY.md](SECURITY.md).
- Tek uygulama örneği (`single-instance`); instance başına dosya kilidi; loader kurulumları global kilitle sıraya alınır.
- Telemetri yok, varsayılan olarak hiçbir veri gönderilmez.

---

## 10. Test ve Kalite

- **Birim (core):** rules, inheritsFrom birleştirme, argüman üretimi (snapshot), sürüm karşılaştırma (`1.8.9` < `1.21.11` < `26.1` < `26.3`, snapshot/pre/rc), NeoForge↔MC eşleme, offline UUID, `Paths` çözümleme (normal/portable/redirect), zip-slip, allowlist.
- **Entegrasyon:** `wiremock` ile manifest/indirici/retry/devam senaryoları; loader başına komut satırı üretimi.
- **Uçtan uca (yerel, yarı otomatik):** her loader için kur → başlat → log'da ana menü kalıbını (ör. `Sound engine started` / `Created: …atlas`) bekle → kapat.
- **Entegrasyon (çekirdek, `crates/launcher-core/tests/`):** sahte Mojang'a karşı tam OYNA hazırlığı (manifest → jar/kütüphane/asset → komut satırı, önbellekten ikinci başlatma, çevrimdışı), Modrinth kurulumu (bağımlılık, hash reddi), Adoptium Java kurulumu.
- **Frontend:** Vitest + Testing Library (store'lar, olay köprüsü, Ana sayfa, Hesaplar, Profiller, Ayarlar, çöküş penceresi, veri taşıma, güncelleme şeridi).
- **Kapsam (Faz 9 sonu):** çekirdek satır %79,8 (`cargo llvm-cov`), frontend satır %33 (`npm run coverage`).
- **Lint:** `cargo fmt --check`, `cargo clippy -- -D warnings`, ESLint, Prettier.
- **CI (GitHub Actions, windows-latest):** lint → test → `tauri build` (NSIS + portable zip artefaktları).

---

## 11. Risk Listesi

| # | Risk | Etki | Önlem |
|---|---|---|---|
| R1 | **Yeni sürüm şeması:** Mojang yıl tabanlı sürümlere geçti (güncel release `26.3`, snapshot `26.4-snapshot-2`). NeoForge `26.3.0.x` biçiminde. | Sürüm karşılaştırma ve NeoForge↔MC eşlemesi kırılabilir. | Karşılaştırıcı iki şemayı da destekler. Eşleme: major ≤ 21 ise `X.Y.z → 1.X.Y` (`Y=0` → `1.X`); major ≥ 26 ise `X.Y.Z.b → X.Y[.Z]`. Gerçek metadata ile test edilir. |
| R2 | **Windows komut satırı sınırı (32.767 karakter):** Forge/NeoForge classpath'leri bu sınırı aşabilir. | Oyun başlamaz. | Java ≥ 9 → `@argfile`. Java 8 → sınır aşılırsa "pathing jar" (manifest `Class-Path`). |
| R3 | **Java 25 gereksinimi:** 26.x sürümleri `javaVersion.majorVersion = 25` istiyor. | Eski runtime ile çökme. | `runtime\java{major}` dinamik; Adoptium'da 8/11/17/21/25 LTS mevcut (doğrulandı). Windows ARM64'te Java 8 yoksa x64'e geri düşülür. |
| R4 | ~~**Microsoft girişi Mojang onayı gerektirir.**~~ (K39 ile geçersiz: Microsoft girişi yok) | Onay olmadan `login_with_xbox` 403 döner. | K8: client_id dışarıdan alınır; offline mod tam işlevseldir. |
| R5 | **Forge installer çeşitliliği:** 1.5–1.12 arası eski installer'ların headless modu yok; modern installer mc kökünde `launcher_profiles.json` ister. | Kurulum başarısız olur. | Eski installer'larda `install_profile.json` (spec 0) elle ayrıştırılır, universal jar `libraries\`'e çıkarılır. Modern installer'da geçici `launcher_profiles.json` oluşturulur, kurulum sonrası silinir, kurulum global kilitle yapılır. |
| R6 | **OptiFine'ın API'si yok.** | Otomatik kurulum yapılamaz. | Yalnızca kullanıcının kendi indirdiği jar içe aktarılır (Forge → `mods\`). Modern sürümlerde Sodium+Iris / Embeddium+Oculus tek tıkla önerilir. Ayrıntılar Faz 4'te araştırılır. |
| R7 | **Private repo + auto-update:** updater, `latest.json`'a herkese açık erişim ister. | Depo private iken güncelleme çalışmaz. | Updater altyapısı Faz 8'de kuruldu (K51). Depo public olana kadar (ya da ayrı bir public release deposu açılana kadar) kanal erişilemez ve UI bunu "ulaşılamadı" olarak gösterir. İmzalama anahtarı GitHub Secrets'ta ve geliştiricinin `~/.tauri/` klasöründe durur, repoya girmez. |
| R8 | **Kod imzalama sertifikası yok.** | SmartScreen uyarısı, antivirüs yanlış pozitifi. | README'de belgelenir; ileride sertifika alınabilir. |
| R9 | **Roaming profil boyutu:** büyük oyun dosyaları `%APPDATA%` (Roaming) altında duruyor (gereksinim gereği). | Domain profillerinde senkron yükü. | Gereksinim olduğu için kabul edildi; "Veri klasörünü taşı" ile çözülebilir. |
| R10 | **Çok eski sürümler (alpha/beta, < 1.6):** skin/ses sunucuları kapalı, `map_to_resources` gerekiyor. | Ses/skin eksik olabilir. | Asset düzenleri desteklenir; skin proxy'si kapsam dışıdır (bilinen kısıt). |
| R11 | **CurseForge:** API anahtarı olmadan mod indirilemez; bazı yazarlar üçüncü taraf dağıtımı kapatır. | Paket kısmen kurulur. | Anahtar gerekli uyarısı; indirilemeyen dosyalar listelenir ve elle indirme bağlantısı verilir. |
| R12 | **Offline skin** vanilla istemcide görünmez. | Kullanıcı beklentisi karşılanmaz. | Launcher içi önizleme + isteğe bağlı CustomSkinLoader kurulumu (Modrinth'ten indirilir, launcher'a gömülmez). |
| R13 | **Geliştirme ortamı eksik:** bu makinede Rust ve MSVC Build Tools kurulu değil. | Faz 1 derlenemez. | Faz 1 öncesi kullanıcı onayıyla `rustup` + VS Build Tools kurulur. |
| R14 | **GitHub Actions (private):** Windows runner'ları dakika kotasını 2 kat harcar. | CI kotası tükenebilir. | CI yalnızca PR ve `main` push'larında çalışır; derleme önbelleği (`rust-cache`) kullanılır. |
| R15 | **Lisans henüz seçilmedi.** | Public yapmadan önce gerekli. | Repo public yapılmadan önce karar verilecek (açık karar A1). |
| R16 | **Arkadaşlar sunucusu (Supabase ücretsiz katman):** kota (500 MB veritabanı, 1 GB depolama, aylık trafik) ve uzun hareketsizlikte projenin duraklatılması; anonim hesaplarla kötüye kullanım (spam, depolama doldurma). | Arkadaşlar özelliği geçici olarak çalışmaz; launcher'ın geri kalanı etkilenmez. | Özellik varsayılan kapalı; hata `friends.*`/`net.*` olarak gösterilir, sayfa çevrimdışı moda düşer. Dakikada 20 mesaj sınırı, mesaj ≤2000 karakter, mod dosyası ≤50 MB, RLS ile yalnızca arkadaşlar okur; gerekirse Supabase'te anonim kayıt kapatılarak yeni kimlik alımı durdurulur (`friends.unavailable`). |

---

## 12. Yol Haritası (özet)

| Faz | Kapsam |
|---|---|
| 0 | Hazırlık: ortam kontrolü, bu belge, git + private repo |
| 1 | Temel: workspace, Tauri+React, tema, frameless pencere, `Paths`, i18n, log, hata modeli |
| 2 | Core/Vanilla: manifest, indirici, kütüphane/asset/native, Java, offline başlatma (CLI) |
| 3 | Instance sistemi + ana ekranlar + konsol + indirme kuyruğu |
| 4 | Loader'lar: Fabric → Quilt → Legacy Fabric → Forge → NeoForge → OptiFine/Iris |
| 5 | ~~Microsoft hesapları~~ — kullanıcı isteğiyle kaldırıldı (K39); çoklu offline hesap Faz 3'te mevcut |
| 6 | Modrinth / mrpack / CurseForge, bağımlılık çözümü, güncelleme |
| 7 | Skin/Cape yöneticisi + skinview3d |
| 8 | Cila: animasyon, haberler, crash analizi, portable, updater, installer |
| 9 | Sertleştirme: güvenlik, performans, test kapsamı, dokümantasyon |
| 10 | Skin stüdyosu (hazır skinler, piksel editörü); premium kavramı kaldırıldı |
| 11 | Arkadaşlar: arkadaş kodu, mesajlaşma, mod listesi paylaşma/kurma (Supabase) |
| 12 | Profil fotoğrafı (skin kafası varsayılan, .png seçme, arkadaşlara gösterme) |
| 13 | Çevrimiçi göstergesi (neon lime daire, arkadaş durumu) |
| 14 | Tüm kullanıcılar arasında benzersiz hesap adı |

**Her faz sonu:** derle → test → çalıştır → doğrula → commit (Conventional Commits) → hassas bilgi taraması → push → rapor (ne çalışıyor / ne eksik / bilinen sorunlar) → onay.

---

## 13. Açık Kararlar

- **A1:** Lisans (MIT / GPL-3.0 / …). Public yapmadan önce seçilecek.

## 14. Karar Kaydı

| Tarih | Karar |
|---|---|
| 2026-10-03 | Microsoft `client_id` env/ayar üzerinden alınır, koda gömülmez (K8). |
| 2026-10-03 | Offline skin: önizleme + isteğe bağlı CustomSkinLoader (R12). |
| 2026-10-03 | Ayarlar `game\mc\launcher\` altında (K1). Veri taşıma `redirect.json` ile (K3). |
| 2026-10-03 | GitHub deposu `MehburMC-Launcher`, private. |
| 2026-10-03 | K9: Başlık fontu Rajdhani; Orbitron yalnızca marka yazısında (Türkçe glif eksikliği). |
| 2026-10-03 | K10: React Router yerine Zustand tabanlı ekran durumu. |
| 2026-10-03 | K11: TypeScript `~6.0`'a sabitlendi; typescript-eslint henüz TS 7'yi desteklemiyor (`<6.1`). |
| 2026-10-03 | K12: Uygulama komutları `build.rs` → `AppManifest` ile listelenir; webview yalnızca capability'de izin verilen komutları çağırabilir. |
| 2026-10-03 | K13: Ana pencere `tauri.conf.json` yerine Rust'ta oluşturulur (portable modda WebView2 veri klasörünü ayarlayabilmek için). |
| 2026-10-03 | K14: TS tipleri `ts-rs` ile `cargo test` sırasında `src/lib/ipc/bindings/`'e üretilir ve repoya commit'lenir (frontend, Rust olmadan derlenebilsin). |
| 2026-10-03 | K15: Natives — eski `natives` haritası: classifier jar `extract.exclude`'a uyularak düzen korunarak çıkarılır. Yeni `natives-*` classifier'ları: Mojang gibi tüm mimariler (ör. `natives-windows` + `natives-windows-arm64`, aynı kuralla gelir) classpath'e girer; yalnızca makinenin mimarisine uyan çıkarılır ve düz (flatten) yazılır, aksi halde aynı adlı DLL'ler çakışır. 26.x, `${natives_directory}/{java,jna,lwjgl,netty}` alt klasörlerini kullanır; LWJGL kendini `SharedLibraryExtractPath`'e çıkarır. |
| 2026-10-03 | K16: Doğrulama modları — her başlatmada `Quick` (varlık + boyut); "onar" için `Full` (SHA-1). Yeni indirilen her dosya her zaman hash'lenir. Tam önbellekli 26.3 hazırlığı ≈0,3 sn. |
| 2026-10-03 | K17: Offline hesap — `accessToken="0"`, `userType="legacy"`, `auth_session="-"`. Modern sürümlerin logladığı `401 /player/attributes` beklenen gürültüdür. |
| 2026-10-03 | K18: Komut satırı sınırı — yalnızca `-cp` çifti Java `@argfile`'a taşınır (dosyada asla token yok; oyun bitince silinir). Java 8'de sınır aşılırsa `launch.commandTooLong` hatası; "pathing jar" gerekirse Faz 4'te (Forge). |
| 2026-10-03 | K19: Java seçimi — instance override → `runtime/java{N}` → sistemde tam eşleşen major → Adoptium'dan indir. Adoptium Windows ARM64'te Java 8 sunmuyor → x64'e düşülür. Linux/macOS `tar.gz` runtime'ları henüz desteklenmiyor. |
| 2026-10-03 | K20: Çevrimdışı — manifest alınamazsa yerel `versions/<id>/<id>.json` güvenilir kabul edilir; yerel dosya SHA-1'i manifestle uyuşuyorsa yeniden indirilmez. |
| 2026-10-03 | Not (Faz 3): modern istemciler log4j **XML olayları** basar (`<log4j:Event …>`); konsol bunları ayrıştırıp düz satıra çevirmeli. → K25 ile çözüldü. |
| 2026-10-03 | K21: `launcher/state.json` — tercih olmayan UI durumu (seçili instance, sürükle-bırak sırası). `settings.json`'dan ayrı tutulur. |
| 2026-10-03 | K22: Faz 3'te OYNA'nın çalışması için `accounts.json` + offline hesap ekle/sil/seç eklendi (yalnızca metadata). Microsoft hesap türü modelde var, girişi Faz 5'te. |
| 2026-10-03 | K23: Görevler (`tasks.rs`) — "duraklat" = iptal + `.part` dosyaları korunur; "devam et" görevi yeniden başlatır ve indirme `Range` ile kaldığı yerden sürer. Aynı instance için yeni görev eski bitmiş kaydı değiştirir; en fazla 30 bitmiş görev tutulur. |
| 2026-10-03 | K24: "Oyun açılınca kapat" davranışı şimdilik "küçült" gibi çalışır (oyunu launcher'dan bağımsız başlatma Faz 8'de). → K49 ile değişti. |
| 2026-10-03 | K25: Oyun çıktısı core'da log4j XML'den {metin, seviye, zaman, thread} olaylarına çevrilir; köprü logları 50 ms'de bir toplu gönderir (flush başına ≤2000 satır), ilerleme olaylarını birleştirir. UI'da instance başına 5000 satırlık halka tampon + sanal liste. |
| 2026-10-03 | K26: Ekran görüntüleri asset protokolüyle gösterilir; kapsam başlangıçta boştur ve yalnızca listelenen instance'ın `screenshots/` klasörü çalışma anında izinlenir. |
| 2026-10-03 | K27: Instance id'leri `slug(ad)-<6 hex>` biçiminde üretilir ve her komutta `[a-z0-9._-]` + `..` yok kuralıyla doğrulanır; dosya adları ayrıca ayırıcı/`..`/`:` içeremez (yol geçişi koruması). |
| 2026-10-03 | K28: Loader'lar trait yerine `LoaderKind` üzerinden `match` ile dağıtılır (async trait bağımlılığı gereksiz). Her loader `versions/<id>/<id>.json` üretir; id yalnızca spec'ten türetilir (`fabric-loader-<v>-<mc>`, `quilt-loader-…`, `legacyfabric-loader-…`, `forge-<maven sürümü>`, `neoforge-<v>`, `optifine-<mc>_<edition>`) → kurulu loader ağsız tespit edilir. Profil JSON'u en son ve atomik yazılır; yarım kurulum "kurulu" sayılmaz. Kurulumlar global kilitle sıralanır. |
| 2026-10-03 | K29: Loader sürümü seçilmezse ilk OYNA'da önerilen sürüm kurulur ve `instance.json`'a sabitlenir (profil kendiliğinden güncellenmez). "Onar" loader'ı yeniden kurar. |
| 2026-10-03 | K30: Forge/NeoForge — installer indirilir (`.sha1` yan dosyasıyla doğrulanır), üç nesil desteklenir: eski `versionInfo` (1.7.10), `version.json` + `maven/` (1.12.2), spec 1 + processor'lar (1.13+, NeoForge). Processor'lar vanilla sürümün Java'sıyla, `CREATE_NO_WINDOW` ile çalışır; çıktılar SHA-1 ile doğrulanır, geçerliyse atlanır. Bazı processor'lar (ör. 1.20.1 `DOWNLOAD_MOJMAPS`) kendi ağ isteklerini yapar; bunlar allowlist dışında Forge'un aracıdır. |
| 2026-10-03 | K31: Paylaşılan `versions/<mc>/<mc>.jar` yüzünden modern Forge'un `-DignoreList=…${version_name}.jar` kalıbı vanilla jar'ı kaçırıyordu; jar dosya adı listeye eklenir. Natives artık artifact başına tekilleştirilir (Legacy Fabric LWJGL'i vanilla'nınkini ezer). Loader profili eski (`minecraftArguments`) bir ebeveyne yalnızca birkaç JVM argümanı eklerse `-cp`/`java.library.path` yine eklenir. |
| 2026-10-03 | K32: OptiFine — API ve yeniden dağıtım izni yok; kullanıcı jar'ı seçer, `loaders/optifine/`'a kopyalanır, `changelog.txt` ilk satırından sürüm okunur. Kurulum HMCL yöntemiyle: `optifine.Patcher` vanilla jar'a karşı `optifine:OptiFine` kütüphanesini üretir, gömülü `launchwrapper-of` çıkarılır, `--tweakClass optifine.OptiFineTweaker`. Forge ile OptiFine = jar'ı `mods/`'a koymak. |
| 2026-10-03 | K33: "Shader desteği" — Modrinth'ten Iris + Sodium (Fabric/Quilt/NeoForge) veya Oculus + Embeddium (Forge), zorunlu bağımlılıklarla (sabitlenmiş sürüm önceliklidir), SHA-512 doğrulamalı. Aynı projenin jar'ı zaten varsa atlanır. Tam Modrinth istemcisi Faz 6'da. |
| 2026-10-03 | K34: NeoForge erken yükleme penceresi bazı sürücülerde devirde native çöküyor (`0xC000041D`, RTX 5060'ta 26.3 ile görüldü). NeoForge'un resmi önerisi uygulanır: 90 sn içinde negatif (NTSTATUS) çıkış kodu → `config/fml.toml` `earlyWindowControl=false` → bir kez otomatik yeniden başlatma. |
| 2026-10-03 | K39: Microsoft (premium) girişi kullanıcı isteğiyle tamamen kaldırıldı: device code akışı, keyring, `msaClientId` ayarı, Xbox/Microsoft giriş host'ları (allowlist) ve UI. `AccountKind` yalnızca `offline`; eski `accounts.json` içindeki Microsoft kayıtları okunurken atlanır. Uygulama git geçmişinde (`467ba34`) duruyor. |
| 2026-10-04 | K40: İçerik (`content`) — Modrinth arama/sürüm/hash API'leri. Kurulum: tür → klasör (mod/resourcepack/shader), sürüm filtresi profilin MC sürümü + loader'ı (Quilt'te Fabric modları da), en yeni release yoksa beta. Modlarda zorunlu bağımlılıklar özyinelemeli kurulur (sabitlenmiş `version_id` önceliklidir), zaten kurulu projeler hash + ad ile atlanır, `incompatible` bildirimleri raporlanır. Bilinmeyen proje (404) "bulunamadı" hatasına çevrilir. |
| 2026-10-04 | K41: Kurulu içerik SHA-1 ile Modrinth'te tanınır (`version_files`); hash'ler `cache/content-hashes.json`'da yol+boyut+mtime anahtarıyla önbelleklenir. Güncelleme `version_files/update` ile; yeni dosya indirilip doğrulanınca eskisi silinir, `.disabled` durumu korunur. Çevrimdışıyken liste metadata'sız gösterilir. |
| 2026-10-04 | K42: Modpack — `.mrpack`: `client: unsupported` dosyalar atlanır, yollar zip-slip kontrolünden geçer, SHA-512/SHA-1 zorunlu, önce `overrides/` sonra `client-overrides/` (üzerine yazarak). CurseForge `.zip`: kullanıcının kendi API anahtarı (`settings.curseforgeApiKey`), `/v1/mods/files` + `/v1/mods` (classId → klasör); yazarın engellediği dosyalar sayfa bağlantısıyla listelenir. Loader sürümü gerçek listeden eşlenir (`47.2.0` → `1.20.1-47.2.0`). Başarısız içe aktarma profili siler; içe aktarma bir `Install` görevi olarak İndirmeler'de görünür. mrpack spec'i GitHub/GitLab aynalarına izin verdiği için `github.com` artık yol öneki olmadan izinli (dosyalar hash ile doğrulanır). |
| 2026-10-04 | K43: Proje ikonları CSP nedeniyle webview'da yüklenmez; Rust indirir (allowlist), `cache/icons/`'a yazar, yalnızca PNG/JPEG/GIF/WebP'yi (`data:` URI) döndürür — SVG reddedilir. Modrinth/CurseForge sayfaları yalnızca bu alan adları için Rust tarafında tarayıcıda açılır. |
| 2026-10-04 | K44: Skin kütüphanesi — dokular içerik adreslidir (`skins/textures/<sha1>.png`), metadata ve hesap→skin/pelerin atamaları `skins/library.json`'da (`accounts.json` değişmez). PNG tamamen çözülür (`png` crate); skin 64×64 / 64×32 ve HD katları (≤1024), pelerin 64×32 / 22×17 ve katları, dosya ≤2 MB. Kol modeli verilmezse kolun dış sütunundan (şeffaf veya opak siyah) slim algılanır. Hesap silinince ataması da silinir. |
| 2026-10-04 | K45: Oyunda gösterim — OYNA'da instance'ın `mods/` klasöründe etkin bir CustomSkinLoader jar'ı varsa seçili hesabın dokuları `CustomSkinLoader/MehburMC/{slim,classic,capes}/<ad>.png`'ye yazılır (yalnızca bu klasöre dokunulur; aynı içerikse mtime korunur). İki `ExtraList` girdisi (`MehburMC-Slim`/`-Classic`, tür `Legacy`, sabit model) CSL tarafından yükleme listesinin başına eklenir, böylece Mojang hesabı olan adlarda Mojang skini yerine yerel skin gelir; girdiler CSL config'inde zaten varsa tekrar yazılmaz. Eşitleme hatası başlatmayı engellemez. CSL (Modrinth `idMHQ4n2`, GPL-3.0) launcher'a gömülmez; mevcut içerik kurucusuyla indirilir. |
| 2026-10-04 | ~~K46~~ (K59 ile kaldırıldı): Premium oyuncudan skin alma — `api.minecraftservices.com/minecraft/profile/lookup/name/<ad>` → `sessionserver.mojang.com/session/minecraft/profile/<uuid>` → `textures.minecraft.net` (profilde `http://` olarak gelen adres HTTPS'e çevrilir). Giriş gerekmez; yeni allowlist host'u yok. 3B önizleme skinview3d (three.js) ile, sayfa lazy yüklenir; webview'a dokular `data:` URI olarak verilir (CSP/asset kapsamı değişmedi). |
| 2026-10-04 | K47: Haberler — `launchercontent.mojang.com/v2/news.json`, yalnızca Java girdileri (`category` veya `newsType: Java`), en yeni 12; `cache/news.json` 1 saat TTL, çevrimdışında bayat kopya, her hata boş liste. Görseller ikon hattından (`content_icon`, `data:` URI), bağlantılar yalnızca `https` ve opener'da `minecraft.net` izinli. |
| 2026-10-04 | K48: Crash analizi (`crash`) — sıfır olmayan ve launcher'ın öldürmediği çıkışta crash report + `hs_err_pid*.log` + oturumda yazılmış `logs/latest.log` + bellekte tutulan son 400 çıktı satırı taranır (dosyalar en fazla son 4 MB). 12 teşhis türü (bellek, heap ayırma, eski/yeni Java, eksik bağımlılık, uyumsuz/çift mod, Mixin, eksik sınıf, GPU sürücüsü, yerel çökme, bozuk dosya); özel teşhis varsa "eksik sınıf" gizlenir; GPU DLL'inde yerel çökme → sürücü teşhisi; ipucu yoksa negatif NTSTATUS kodu yerel çökme sayılır. Sonuç `GameCrashed` olayıyla UI'a gider; Logs sekmesinden eski raporlar da analiz edilir. |
| 2026-10-04 | K49: "Oyun açılınca kapat" artık gerçek: oyun ana menü satırına (`is_ready_line`) ulaşınca launcher `app.exit(0)` ile kapanır; ana menü satırı hiç gelmezse 120 sn sonra. Windows çocuk süreci öldürmez ve launcher çıkışta oyunu sonlandırmaz; kapalı stdout'a yazımları Java yutar. Bu oturumun oynama süresi kaydedilmez. |
| 2026-10-04 | K50: Veri klasörünü taşı (K4 uygulandı) — yalnızca içerik klasörleri; hedef mutlak, boş (varsayılan konuma geri dönüşte içerik klasörleri boş), iç içe değil; boş alan kontrolü; kopya → dosya sayısı + bayt doğrulama (başarısızsa yeni kopya silinir) → `redirect.json` yaz/sil → eski kopyayı sil (silinemeyenler, ör. açık log, raporlanır) → yeniden başlat. Portable WebView2 profili (`cache/webview`) kilitli olduğundan kopyalanmaz. Oyun/indirme çalışırken reddedilir. |
| 2026-10-04 | K51: Updater — `tauri-plugin-updater` (Rust tarafından çağrılır, webview'a izin yok), uç nokta `github.com/Mehbur07/MehburMC-Launcher/releases/latest/download/latest.json`  (K63 ile `updater` dalına taşındı), minisign açık anahtarı `tauri.conf.json`'da, NSIS `passive` kurulum. Ağ erişimi eklentinin kendi istemcisiyle yapılır (sabit GitHub adresi ve imza doğrulaması). Ulaşılamayan kanal hata değil `unavailable`; portable kopyada `portable` (zip'i elle değiştir). Açılışta denetim ayardan kapatılabilir (`checkUpdates`). |
| 2026-10-04 | K52: Paketleme/CI — `npm run package` = `tauri build` (NSIS + `.sig`) + `scripts/package-portable.mjs` (exe + `portable.flag` + PORTABLE.txt → zip). `ci.yml`: main push/PR'da fmt, clippy, test, üretilmiş TS tiplerinin commit'li olduğu, prettier/eslint/tsc/vitest. `release.yml`: `v*` tag'i veya elle; `tauri-action` ile taslak release + `latest.json`, portable zip eklenir. Tam derleme her push'ta yapılmaz (R14). |
| 2026-10-04 | K53: Animasyon — liste girişleri CSS `rise-in` (`--i` ile kademeli, `animation-fill-mode: backwards` → dnd-kit'in inline `transform`'unu ezmez), sayaç rozeti `pop-in`, butonlarda basma ölçeği, crash/haber/güncelleme bileşenlerinde Framer Motion. Hepsi `prefers-reduced-motion` ile kapanır. Ayarlara "Oyun" bölümü (başlatma davranışı, varsayılan RAM, eşzamanlı indirme) eklendi. |
| 2026-10-04 | K54: Dosya diyalogları Rust'ta — `pick_path(purpose)` (modpack, profil arşivi, OptiFine, skin, pelerin, Java, veri klasörü, dışa aktarma hedefleri) sabit filtrelerle native diyalog açar; seçilen yol `AppState.picks`'te amaç anahtarıyla saklanır ve ilgili komut `take_pick` ile bir kez alır. Komutlar artık yol parametresi almaz; webview capability'sinden `dialog:allow-open/save` kaldırıldı, JS `plugin-dialog` bağımlılığı silindi. Kaydetme önerisindeki dosya adı temizlenir. |
| 2026-10-04 | K55: Çalıştırılabilir doğrulaması — profil `javaPath` yalnızca mutlak ve adı `java.exe`/`javaw.exe` (Unix: `java`) olan yol; oluşturma, güncelleme ve başlatmada (`java::resolve`) denetlenir. `-XX:OnError`/`-XX:OnOutOfMemoryError` (büyük/küçük harf duyarsız) reddedilir. İçe aktarılan profil arşivlerinde `javaPath` silinir, bu seçenekler ayıklanır. |
| 2026-10-04 | K56: `Http::get_bytes`/`request_json` yanıtları `read_limited` ile en fazla 64 MB (önce `Content-Length`, sonra akış sırasında); aşımda `net.tooLarge`. Büyük dosyalar zaten akışlı indiriciden geçer. Log maskelemesi `x-api-key`, `api_key`, `curseforgeApiKey` ve bcrypt biçimli CurseForge anahtarlarını kapsar. |
| 2026-10-04 | K57: Performans — Ana sayfa dışındaki tüm ekranlar ve sihirbaz `React.lazy` ile ilk kullanımda yüklenir; paylaşılan küçük yardımcılar (`loaderLabels.ts`, `console/lineClass.ts`) sayfa modüllerinden ayrıldı. Başlangıç JS'i 645 KB → ~504 KB (gzip 200 → 163 KB). Önbellekli OYNA hazırlığı ölçüldü: 0,2–0,3 sn (26.2 / 1.20.1, CLI `--dry-run`); çekirdekte iyileştirme gerekmedi. |
| 2026-10-04 | K58: Test altyapısı — Vitest kurulumunda global `cleanup` (globals kapalı), `src/test/fixtures.ts`; `@vitest/coverage-v8` + `npm run coverage`. `cargo audit`: açık yok (2 uyarı, yalnızca Linux GTK bağımlılıkları). Belgeler: `docs/KULLANIM.md`, `docs/SORUN_GIDERME.md`, `docs/GELISTIRME.md`, `SECURITY.md`, `CHANGELOG.md`. |
| 2026-10-04 | K59: Faz 10 — "premium" kavramı tamamen kaldırıldı: "premium değil" rozeti ve premium oyuncudan skin kopyalama (`skin/mojang.rs`, `import_player_skin`, `PlayerNotFound`, `Endpoints.mojang_services/session_server`, allowlist'ten `textures.minecraft.net`, `sessionserver.mojang.com`, `api.minecraftservices.com`). Hazır skinler: oyunun varsayılanları çalışma anında kurulu en yeni client jar'dan (`assets/minecraft/textures/entity/player/{wide,slim}/*.png`, eski jar'larda `entity/{steve,alex}.png`) okunur, launcher Mojang görseli dağıtmaz; MehburMC koleksiyonu (11 skin, 10 pelerin) `src/features/skins/presets` içinde kodla, deterministik çizilir. Editör ve hazırlar kütüphaneye `add_skin_bytes` (base64 PNG, ≤2 MB, aynı doğrulama/dedupe) ile yazar. Tasarım editörü saf mantığı `editor/ops.ts` + `layout.ts` (UV kutuları, ayna eşleme, yüz içinde kova), canlı 3B önizleme rAF ile; taslak yalnızca localStorage'ta. Bedrock bölümü kullanıcı isteğiyle faz dışı bırakıldı. |
| 2026-10-04 | K60: Yerleşik MehburMC skin ve pelerini — `mehbur` presetlerinden PNG olarak `crates/launcher-core/assets/builtin/` altına dışa aktarılır, `include_bytes!` ile gömülür; `list_skins` her çağrıda `seed_builtins` ile eksikleri bir kez ekler, eklenenleri `library.json` → `builtin` anahtarlarıyla hatırlar (silinen geri gelmez). `builtin.test.ts` PNG'lerin presetle piksel piksel aynı olduğunu doğrular. |
| 2026-10-04 | K61: Hesap yeniden adlandırma — `AccountStore::rename(id, ad)`: aynı ad kuralı (`validate_name`), başka hesapta aynı ad (büyük/küçük harf duyarsız) → `account.nameTaken`; ad ve UUID (`OfflinePlayer:<yeni ad>`, kullanıcı tercihi) güncellenir, `id` sabit kalır (skin atamaları ve seçim korunur). Başlatma `LaunchAccount::offline(ad)` ile yeni adı/UUID'yi kullanır; CSL dosyası yeni ada yazılır. Hesaplar ekranında "offline" ifadeleri kaldırıldı ("Yeni hesap oluştur"). Wiremock testleri havuz dışı `MockServer::builder().start()` kullanır (havuzdan gelen geç istekler `expect` sayımını bozuyordu). |
| 2026-10-04 | K62: v0.2.0 — repo herkese açık (öncesinde tüm git geçmişi gizli bilgi için tarandı; commit e-postaları GitHub noreply). İlk GitHub sürümü `v0.2.0`: `MehburMC-Launcher_<v>_x64-setup.exe` (+`.sig`), portable zip ve `latest.json` (`windows-x86_64` ve `windows-x86_64-nsis`, notlar CHANGELOG bölümünden; boşluksuz dosya adı, GitHub boşlukları noktaya çevirir). Güncelleme şeridi yerine başlık çubuğunda Güncelle butonu (indir → NSIS `passive` kurulum → yeniden başlat). "Yenilikler" görünümü paketlenmiş CHANGELOG'u (`?raw`) sürüm bölümlerine ayırır; `launcher/last-version` işareti güncellemeden sonraki ilk açılışı bildirir (`Bootstrap.updatedFrom`; işaret yok ama `settings.json` var → 0.2 öncesi kurulum sayılır). NSIS `installerIcon` = uygulama ikonu. |
| 2026-10-04 | K63: Sürüm sayfasında yalnızca kurulum `.exe`'si (kullanıcı isteği: indirenler karışıyordu). Updater uç noktası `raw.githubusercontent.com/Mehbur07/MehburMC-Launcher/updater/latest.json` — kod içermeyen `updater` dalında tek dosya; imza `latest.json` içinde olduğundan `.sig` yüklenmez. Portable zip yayınlanmaz (yerelde derlenmeye devam eder). Eski uç noktayı kullanan ≤0.2.1 kurulumlar 0.2.2'yi bir kez elle kurar. README başında İndir bölümü. |
| 2026-10-05 | K64: Faz 11 — Arkadaşlar. Sunucu Supabase (proje `rcaifwsiutoxilpkceht`); şema `supabase/schema.sql`: tablolar `profiles`, `friendships`, `messages`, `shared_lists`; `security definer` RPC'ler `ensure_profile`, `send_request`, `respond_request`, `remove_friend`, `block_user`, `mark_read`, `my_friends`, `delete_me`; RLS ile mesajı yalnızca iki taraf, listeyi sahibi + kabul edilmiş arkadaşları okur; dakikada 20 mesaj trigger'ı; özel `mods` kovası ≤50 MB, yol `<sahip>/<sha1>.jar`. Kimlik **anonim Supabase hesabı** (parola yok); oturum `launcher/friends.json`'da (`write_json_atomic`), erişim token'ı yenileme token'ıyla Rust'ta yenilenir. Anon anahtarı koda gömülüdür (tasarım gereği herkese açık, yetkiyi RLS verir; `service_role` anahtarı hiçbir yerde yok). Tüm istekler Rust'ta; allowlist'te yalnızca bu host'un `/auth/v1/`, `/rest/v1/`, `/storage/v1/` yolları; webview CSP'si değişmedi. Paylaşım: profilin `mods/` jar'ları SHA-1 → Modrinth `version_files`; eşleşenler Modrinth CDN adresiyle, eşleşmeyenler kovaya (`Http::request_bytes`) yüklenir, >50 MB'ler `tooLarge` olarak listelenir, listeden çıkan yüklemeler silinir. Kurulum: Modrinth URL'si veya kısa ömürlü imzalı Storage URL'si → indirici + SHA-1 doğrulaması (eşleşmeyen dosya kurulmaz), `checked_name`, aynı SHA-1 zaten varsa atlanır; doğrulanmamış (Modrinth dışı) dosyalar UI'da ayrıca onaylanır. Özellik varsayılan kapalı, açıklamalı onay ekranından sonra açılır; "kapat ve verilerimi sil" = `delete_me` + yüklemeleri silme + yerel oturumu silme. Log maskelemesine `refreshToken`/`authorization`/`bearer` eklendi. UI: kenar çubuğunda Arkadaşlar + okunmamış rozeti (20 sn yoklama, açık sohbette 3 sn). Profil fotoğrafı ve çevrimiçi durumu kullanıcı isteğiyle sonraya bırakıldı. Uygulama logosu yeni "MMC" rozetiyle değiştirildi (`tauri icon`, uygulama içi `src/assets/logo.png`). |
| 2026-10-05 | K65: Faz 12 — Profil fotoğrafı. Hesap fotoğrafı `launcher/avatars/<hesap id>.png` (`auth::avatar::AvatarStore`): yalnızca PNG, dosya ≤10 MB, kenar ≤4096; ortadan kare kırpılıp alfa ağırlıklı alan ortalamasıyla 256×256'ya küçültülür (yeni bağımlılık yok, `skin::image::decode_limited`/`encode`). Seçim Rust tarafında (`PickPurpose::Avatar`, K54); hesap silinince fotoğraf da silinir. Fotoğraf yoksa UI skin kafasını çizer; skin atanmamışsa oyunun varsayılanı: kurulu en yeni client jar'ından okunan skinlerden `floorMod(UUID.hashCode(), 18)` (1.19.3+, önce 9 slim sonra 9 wide, alfabetik) veya eski jar'larda en düşük bit ile Steve/Alex (`skin::defaults::for_uuid`). Arkadaşlara: seçili hesabın fotoğrafı ya da skin kafası (`skin::image::head`, yüz + şapka katmanı) 128 px PNG olarak özel `avatars` kovasına `<uid>/<sha1>.png` adıyla yüklenir (sürüm başına ayrı dosya → önbellek sorunu yok, eskiler silinir), `profiles.avatar_sha1` RPC `set_avatar` ile yazılır; `my_friends()` yalnızca kabul edilmiş arkadaşlar için özeti döndürür, kova RLS'i okumayı sahip + arkadaşlarla sınırlar. Arkadaş fotoğrafı indirilince SHA-1 ve PNG doğrulanır, `cache/avatars/<uid>-<sha1>.png`'de tutulur. Eşitleme arkadaşlar açıkken hesap ekleme/silme/seçme/yeniden adlandırma, fotoğraf ve skin değişiminde arka planda yapılır; hatası profili bozmaz. "Verilerimi sil" fotoğrafları Storage API ile siler. Sunucu değişikliği `supabase/phase12.sql` (schema.sql'e de eklendi). Paylaşılan mod listelerinde Modrinth sürüm adı yerine proje adı gösterilir (`/v2/projects`). |
| 2026-10-06 | K66: Faz 13 — Çevrimiçi durumu. `profiles.last_seen`; RPC `heartbeat()` (`now()`) ve `go_offline()` (`null`); `my_friends()` kabul edilmiş arkadaşlar için `online = last_seen > now() - 90 sn` döndürür (engellenen/bekleyen için hep false). Tauri `setup`'ta arka plan döngüsü arkadaşlar açıkken 60 sn'de bir `heartbeat` gönderir (`friends::presence`); sonucu `FriendsClient.online` (AtomicBool) tutar → `friends_online` komutu ağsız okur, `FriendsStatus.online`. `enable` hemen bir heartbeat atar; `disable_and_delete` çevrimdışı yapar. Uygulama kapanırken (`RunEvent::Exit`) `go_offline` en fazla 2 sn beklenir (ölçüm: kapanış ~0,4 sn, sunucuda `last_seen` null). UI: `Avatar online` → sağ altta neon lime daire (`--mc-online #b6ff1a`, `online-dot` yardımcı sınıfı: koyu halka + yumuşak parıltı nabzı, hareket azaltmada durur; `role=img` + "Çevrimiçi" etiketi); kendi aktif hesabında (Hesaplar, ana ekran) yalnızca arkadaşlar açık ve son heartbeat başarılıysa, arkadaşlarda listede ve sohbet başlığında; çevrimiçi arkadaşlar listenin başına sıralanır. Sunucu değişikliği `supabase/phase13.sql` (schema.sql'e eklendi). |
| 2026-10-06 | K67: Faz 14 — Hesap adları tüm MehburMC kullanıcıları arasında benzersiz. `account_names(name_lower pk, name, owner → auth.users on delete cascade)`; RLS açık, politika yok, `anon`/`authenticated` izinleri kaldırıldı → yalnızca `security definer` RPC'ler: `claim_name` (aynı sahipte idempotent, kimlik başına ≤10, `account.nameTaken/nameLimit/nameInvalid`), `release_name`, `rename_name` (tek işlem; yeni ad alınamazsa eskisi geri gelir), `claim_names` (açılış toplu talebi: ok/taken/limit/invalid). Kimlik ayrımı: `friends.json` oturumu artık **kurulum kimliği**; `friendsEnabled` bayrağı ayrı (eski dosyalarda varsayılan true). Ad işlemleri gerekirse anonim kimliği arkadaşlar kapalıyken oluşturur (`ensure_identity`). "Arkadaşları kapat ve verilerimi sil" artık `delete_friend_data()` (profil + kaskad) ve dosyaları siler, kimlik ve adlar kalır; tam silme `delete_identity()` (`delete_me`, adlar dahil) yalnızca testlerde kullanılır — kullanıcı arayüzünde yok, çünkü yerel hesaplar sonraki açılışta adları yeniden talep ederdi; ad hesap silinince bırakılır. Akış (`Launcher::create_account/rename_account/remove_account`): yerel kural + yerel çakışma → sunucu talebi → yerel kayıt (başarısızsa geri bırakma/geri adlandırma). Çevrimdışı → `account.nameCheckOffline`, anonim kayıt kapalı → `account.nameServiceUnavailable`; silmede çevrimdışı bırakma `launcher/pending-releases.json` kuyruğuna girer, açılışta (`sync_account_names`, Tauri setup) yerelde hâlâ kullanılmayanlar bırakılır, sonra tüm yerel adlar talep edilir; başkasındaki adlar `name_conflicts` olur → kartta "yeniden adlandır" rozeti (oyun engellenmez; çakışan hesabı silmek başkasının adını bırakmaz). `errors.account.nameTaken` = "Bu kullanıcı adı alındı. Lütfen başka bir ad seçiniz." (yerel çakışmada da). CLI (`mehbur-cli`) hesapları yalnızca yerelde yönetir (geliştirici aracı). Sunucu değişikliği `supabase/phase14.sql` (schema.sql'e eklendi). |
| 2026-10-06 | K68: Faz 15 — Sunucular sayfası. `launcher-core::servers`: `ServerAddress` (host[:port], IPv6 köşeli parantez, varsayılan 25565, ad/IPv4 doğrulama; ana makine küçük harf), favoriler `launcher/servers.json` (id = normalize adresin SHA-1'inin ilk 6 baytı → aynı sunucu bir kez; ≤200), oyunun listesi `<instance>/servers.dat` için kendi küçük NBT okuyucu/yazıcısı (`servers/nbt.rs`, sıkıştırmasız, Java modified UTF-8, derinlik/uzunluk sınırları; bilinmeyen alanlar ve `hidden` girdiler korunur → ekle+sil sonrası dosya bayt bayt aynı; yazmadan önce `servers.dat_old` yedeği; bozuk dosyanın üstüne yazılmaz; silme index + adres eşleşmesiyle; oyun çalışırken yazma `InstanceBusy`). Ping: Server List Ping 1.7+ (handshake protokol -1 + durum + ping/pong; ping yanıtı yoksa durum turu gecikme sayılır), toplam 6 sn zaman aşımı, paket ≤1 MiB, SRV `_minecraft._tcp` yalnızca port yazılmamışsa (`hickory-resolver`, sistem DNS ayarı), IPv4 önce; MOTD (`servers/motd.rs`) sohbet bileşeni + `§` kodları → renkli span'ler (≤512 karakter, ≤128 span, derinlik 16), favicon yalnızca `data:image/png;base64` ve ≤128 KiB. Ham TCP olduğu için HTTPS allowlist'ine girmez; yalnızca kullanıcının girdiği/listelediği adreslere gider. Hatalar `server.unreachable/timeout/badResponse/addressInvalid/notFound/listInvalid`. Bağlan: `LaunchOptions.join_server` → sürüm JSON'unda `--quickPlayMultiplayer` varsa o (1.20+), yoksa `--server/--port`; `Launcher::start_with`, komut `join_server`. CLI: `mehbur-cli ping <adres>`, `launch --server`. Tauri izinleri için `src-tauri/tests/permissions.rs` (generate_handler ↔ build.rs ↔ capabilities). |
| 2026-10-06 | K69: Faz 16 — Mod Tarayıcı'da kurulu içerik. Sekmenin klasörü (`folderFor`: mod→mods, resourcepack→resourcePacks, shader→shaderPacks, modpack→yok) hedef profilde mevcut `scan_content(checkUpdates=false)` ile taranır (SHA-1 → Modrinth, önbellekli); dosyalar `projectId`'ye göre gruplanır (`installedByProject`). Kurulu sonuçta **Kurulu ▾** (`InstalledMenu`; kapalı dosya varsa "Kurulu (kapalı)") → **Sil** → `ConfirmDialog` "Silmek istediğinize emin misiniz?" (Sil/İptal, silinecek dosya adları listelenir) → projenin tüm dosyaları (`.disabled` dahil) `delete_instance_file` ile silinir, ardından yeniden tarama; kurulumdan sonra da yeniden taranır. Çevrimdışıyken tarama sessizce boş kalır (rozet yok). `delete_instance_file` artık `Launcher::delete_instance_file`: mods/resourcePacks/shaderPacks oyun çalışırken `instance.busy` (Windows jar kilidi yerine anlaşılır hata); diğer klasörler değişmedi. Log filtresi `hickory_*=error` (SRV sorgusunda araya giren DNS yanıtları için gereksiz uyarılar). |
