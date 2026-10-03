# MehburMC Launcher — Mimari

> Durum: **Onaylandı (2026-10-03). Faz 3 tamamlandı.** Bu belge yaşayan bir belgedir; her fazda alınan kararlar "Karar Kaydı" bölümüne eklenir.

MehburMC Launcher; Windows öncelikli (Linux/macOS'a taşınabilir), açık mimarili, reklamsız, telemetrisiz bir Minecraft Java Edition launcher'ıdır. SKLauncher'ın özellik zenginliğini (offline + premium, skin/cape, modpack, loader desteği, portable) ve Legacy Launcher'ın hafifliğini / izole profil yapısını birleştirir.

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
| Gizli anahtar | `keyring` | Windows Credential Manager / macOS Keychain / Secret Service. |
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

Tauri eklentileri (en az yetki): `dialog` (dosya seçici), `opener` (yalnızca veri klasörünü açma, kapsam sınırlı), `updater`, `single-instance`, `process` (yeniden başlatma). **`shell` eklentisi kullanılmaz**; oyun/Java/installer süreçleri yalnızca Rust tarafından, sabit argüman şablonlarıyla başlatılır.

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
│   ├── microsoft   device code → XBL → XSTS → MC services → profil/sahiplik
│   └── store       accounts.json (yalnızca meta) + keyring (refresh token)
├── content      Modrinth v2, CurseForge (kullanıcı anahtarı), mrpack/CF zip, bağımlılık çözümü
├── skin         doğrulama (64x64 / 64x32), Mojang skin/cape API, CustomSkinLoader kurulumu
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
   2. hesap: offline → UUID v3 | microsoft → keyring'den refresh → access token (yalnızca bellekte)
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
- Microsoft hesabı çevrimdışıyken son bilinen profil + offline benzeri başlatma (yalnızca tek oyunculu); UI bunu açıkça belirtir.

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
  - Mojang: `piston-meta.mojang.com`, `piston-data.mojang.com`, `launchermeta.mojang.com`, `launcher.mojang.com`, `libraries.minecraft.net`, `resources.download.minecraft.net`, `launchercontent.mojang.com`, `textures.minecraft.net`, `sessionserver.mojang.com`, `api.minecraftservices.com`
  - Auth: `login.microsoftonline.com`, `user.auth.xboxlive.com`, `xsts.auth.xboxlive.com`
  - Fabric / Quilt / Legacy Fabric: `meta.fabricmc.net`, `maven.fabricmc.net`, `meta.quiltmc.org`, `maven.quiltmc.org`, `meta.legacyfabric.net`, `maven.legacyfabric.net`
  - Forge / NeoForge: `files.minecraftforge.net`, `maven.minecraftforge.net`, `maven.neoforged.net`, `repo1.maven.org` (eski Forge bağımlılıkları)
  - Modrinth: `api.modrinth.com`, `cdn.modrinth.com` (+ mrpack spec'inin izin verdiği `github.com`, `raw.githubusercontent.com`, `gitlab.com`)
  - CurseForge: `api.curseforge.com`, `edge.forgecdn.net`, `mediafilez.forgecdn.net`
  - Adoptium: `api.adoptium.net`, `github.com/adoptium/*` → `objects.githubusercontent.com` / `release-assets.githubusercontent.com` (doğrulandı: Temurin ikilileri GitHub Releases'tan gelir)
- **Hash doğrulama:** Mojang SHA1; Fabric/Quilt profillerindeki `sha1`/`sha256`/`sha512` (Fabric meta'da mevcut, doğrulandı); Maven'da hash yoksa `<url>.sha1` yan dosyası; Modrinth SHA512; Adoptium SHA256. Hiçbir hash bulunamazsa dosya indirilir ve uyarı loglanır (yalnızca Maven kütüphaneleri için).
- **Dayanıklılık:** `.part` + `Range` ile devam; üstel geri çekilme + jitter (5 deneme); `rename` ile atomik taşıma; indirme öncesi boş alan kontrolü + `StorageFull` / Windows `ERROR_DISK_FULL` yakalama; eşzamanlılık `Semaphore` ile sınırlı (varsayılan 8, ayarlanabilir); görev başına duraklat/devam/iptal.

---

## 7. Kimlik Doğrulama

- **Offline:** ad `^[A-Za-z0-9_]{3,16}$`; UUID = MD5 tabanlı v3 (`OfflinePlayer:<ad>`), Java `UUID.nameUUIDFromBytes` ile birebir; birim testiyle doğrulanır. UI "premium değil" rozeti + çevrimiçi sunucu kısıtı açıklaması gösterir.
- **Microsoft:** OAuth2 device code (`/consumers/oauth2/v2.0/devicecode`, scope `XboxLive.signin offline_access`) → XBL → XSTS → `login_with_xbox` → `/entitlements/mcstore` + `/minecraft/profile`.
  - **Karar K8:** `client_id` sırasıyla `MEHBURMC_MSA_CLIENT_ID` ortam değişkeninden, sonra `settings.json → auth.msaClientId`'den okunur. **Koda ve repoya gömülmez.** Tanımlı değilse Microsoft girişi butonu devre dışıdır ve nedeni açıklanır. Mojang onayı alınana kadar offline mod tam çalışır.
  - Refresh token → `keyring` (servis: `MehburMC Launcher`, kullanıcı: hesap UUID'si). Access token yalnızca bellekte tutulur.
  - Sahiplik doğrulanamayan hesapta skin/cape API özellikleri kapatılır.
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
- Süreç başlatma: yalnızca bilinen Java ikilileri, argümanlar dizi olarak verilir (shell yorumlaması yok).
- Tek uygulama örneği (`single-instance`); instance başına dosya kilidi; loader kurulumları global kilitle sıraya alınır.
- Telemetri yok, varsayılan olarak hiçbir veri gönderilmez.

---

## 10. Test ve Kalite

- **Birim (core):** rules, inheritsFrom birleştirme, argüman üretimi (snapshot), sürüm karşılaştırma (`1.8.9` < `1.21.11` < `26.1` < `26.3`, snapshot/pre/rc), NeoForge↔MC eşleme, offline UUID, `Paths` çözümleme (normal/portable/redirect), zip-slip, allowlist.
- **Entegrasyon:** `wiremock` ile manifest/indirici/retry/devam senaryoları; loader başına komut satırı üretimi.
- **Uçtan uca (yerel, yarı otomatik):** her loader için kur → başlat → log'da ana menü kalıbını (ör. `Sound engine started` / `Created: …atlas`) bekle → kapat.
- **Frontend:** Vitest + Testing Library (sihirbaz, hesap ekleme, ayarlar).
- **Lint:** `cargo fmt --check`, `cargo clippy -- -D warnings`, ESLint, Prettier.
- **CI (GitHub Actions, windows-latest):** lint → test → `tauri build` (NSIS + portable zip artefaktları).

---

## 11. Risk Listesi

| # | Risk | Etki | Önlem |
|---|---|---|---|
| R1 | **Yeni sürüm şeması:** Mojang yıl tabanlı sürümlere geçti (güncel release `26.3`, snapshot `26.4-snapshot-2`). NeoForge `26.3.0.x` biçiminde. | Sürüm karşılaştırma ve NeoForge↔MC eşlemesi kırılabilir. | Karşılaştırıcı iki şemayı da destekler. Eşleme: major ≤ 21 ise `X.Y.z → 1.X.Y` (`Y=0` → `1.X`); major ≥ 26 ise `X.Y.Z.b → X.Y[.Z]`. Gerçek metadata ile test edilir. |
| R2 | **Windows komut satırı sınırı (32.767 karakter):** Forge/NeoForge classpath'leri bu sınırı aşabilir. | Oyun başlamaz. | Java ≥ 9 → `@argfile`. Java 8 → sınır aşılırsa "pathing jar" (manifest `Class-Path`). |
| R3 | **Java 25 gereksinimi:** 26.x sürümleri `javaVersion.majorVersion = 25` istiyor. | Eski runtime ile çökme. | `runtime\java{major}` dinamik; Adoptium'da 8/11/17/21/25 LTS mevcut (doğrulandı). Windows ARM64'te Java 8 yoksa x64'e geri düşülür. |
| R4 | **Microsoft girişi Mojang onayı gerektirir.** | Onay olmadan `login_with_xbox` 403 döner. | K8: client_id dışarıdan alınır; offline mod tam işlevseldir. |
| R5 | **Forge installer çeşitliliği:** 1.5–1.12 arası eski installer'ların headless modu yok; modern installer mc kökünde `launcher_profiles.json` ister. | Kurulum başarısız olur. | Eski installer'larda `install_profile.json` (spec 0) elle ayrıştırılır, universal jar `libraries\`'e çıkarılır. Modern installer'da geçici `launcher_profiles.json` oluşturulur, kurulum sonrası silinir, kurulum global kilitle yapılır. |
| R6 | **OptiFine'ın API'si yok.** | Otomatik kurulum yapılamaz. | Yalnızca kullanıcının kendi indirdiği jar içe aktarılır (Forge → `mods\`). Modern sürümlerde Sodium+Iris / Embeddium+Oculus tek tıkla önerilir. Ayrıntılar Faz 4'te araştırılır. |
| R7 | **Private repo + auto-update:** updater, `latest.json`'a herkese açık erişim ister. | Depo private iken güncelleme çalışmaz. | Updater altyapısı Faz 8'de kurulur. Depo public olana kadar (ya da ayrı bir public release deposu açılana kadar) güncelleme kanalı devre dışıdır. İmzalama anahtarı yalnızca GitHub Secrets'ta durur. |
| R8 | **Kod imzalama sertifikası yok.** | SmartScreen uyarısı, antivirüs yanlış pozitifi. | README'de belgelenir; ileride sertifika alınabilir. |
| R9 | **Roaming profil boyutu:** büyük oyun dosyaları `%APPDATA%` (Roaming) altında duruyor (gereksinim gereği). | Domain profillerinde senkron yükü. | Gereksinim olduğu için kabul edildi; "Veri klasörünü taşı" ile çözülebilir. |
| R10 | **Çok eski sürümler (alpha/beta, < 1.6):** skin/ses sunucuları kapalı, `map_to_resources` gerekiyor. | Ses/skin eksik olabilir. | Asset düzenleri desteklenir; skin proxy'si kapsam dışıdır (bilinen kısıt). |
| R11 | **CurseForge:** API anahtarı olmadan mod indirilemez; bazı yazarlar üçüncü taraf dağıtımı kapatır. | Paket kısmen kurulur. | Anahtar gerekli uyarısı; indirilemeyen dosyalar listelenir ve elle indirme bağlantısı verilir. |
| R12 | **Offline skin** vanilla istemcide görünmez. | Kullanıcı beklentisi karşılanmaz. | Launcher içi önizleme + isteğe bağlı CustomSkinLoader kurulumu (Modrinth'ten indirilir, launcher'a gömülmez). |
| R13 | **Geliştirme ortamı eksik:** bu makinede Rust ve MSVC Build Tools kurulu değil. | Faz 1 derlenemez. | Faz 1 öncesi kullanıcı onayıyla `rustup` + VS Build Tools kurulur. |
| R14 | **GitHub Actions (private):** Windows runner'ları dakika kotasını 2 kat harcar. | CI kotası tükenebilir. | CI yalnızca PR ve `main` push'larında çalışır; derleme önbelleği (`rust-cache`) kullanılır. |
| R15 | **Lisans henüz seçilmedi.** | Public yapmadan önce gerekli. | Repo public yapılmadan önce karar verilecek (açık karar A1). |

---

## 12. Yol Haritası (özet)

| Faz | Kapsam |
|---|---|
| 0 | Hazırlık: ortam kontrolü, bu belge, git + private repo |
| 1 | Temel: workspace, Tauri+React, tema, frameless pencere, `Paths`, i18n, log, hata modeli |
| 2 | Core/Vanilla: manifest, indirici, kütüphane/asset/native, Java, offline başlatma (CLI) |
| 3 | Instance sistemi + ana ekranlar + konsol + indirme kuyruğu |
| 4 | Loader'lar: Fabric → Quilt → Legacy Fabric → Forge → NeoForge → OptiFine/Iris |
| 5 | Microsoft hesapları, keyring, çoklu hesap |
| 6 | Modrinth / mrpack / CurseForge, bağımlılık çözümü, güncelleme |
| 7 | Skin/Cape yöneticisi + skinview3d |
| 8 | Cila: animasyon, haberler, crash analizi, portable, updater, installer |
| 9 | Sertleştirme: güvenlik, performans, test kapsamı, dokümantasyon |

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
| 2026-10-03 | K24: "Oyun açılınca kapat" davranışı şimdilik "küçült" gibi çalışır (oyunu launcher'dan bağımsız başlatma Faz 8'de). |
| 2026-10-03 | K25: Oyun çıktısı core'da log4j XML'den {metin, seviye, zaman, thread} olaylarına çevrilir; köprü logları 50 ms'de bir toplu gönderir (flush başına ≤2000 satır), ilerleme olaylarını birleştirir. UI'da instance başına 5000 satırlık halka tampon + sanal liste. |
| 2026-10-03 | K26: Ekran görüntüleri asset protokolüyle gösterilir; kapsam başlangıçta boştur ve yalnızca listelenen instance'ın `screenshots/` klasörü çalışma anında izinlenir. |
| 2026-10-03 | K27: Instance id'leri `slug(ad)-<6 hex>` biçiminde üretilir ve her komutta `[a-z0-9._-]` + `..` yok kuralıyla doğrulanır; dosya adları ayrıca ayırıcı/`..`/`:` içeremez (yol geçişi koruması). |
