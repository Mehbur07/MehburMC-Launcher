# Güvenlik

## Tehdit modeli

MehburMC Launcher internetten dosya indirir, arşiv açar ve Java süreçleri başlatır. Korunan
varlıklar: kullanıcının bilgisayarı (keyfi kod çalıştırma, dosya okuma/yazma) ve kişisel
veriler. Dikkate alınan saldırganlar: ağdaki araya giren taraf, kötü niyetli/bozuk indirme
kaynağı, paylaşılan kötü niyetli profil/modpack arşivi ve — derinlemesine savunma olarak —
webview'da çalışabilecek kötü niyetli içerik (XSS).

## Önlemler

**Ağ**
- Yalnızca HTTPS; her istek ve her yönlendirme adımı bir host izin listesinden geçer
  (`net/allowlist.rs`). Webview hiçbir dış adrese bağlanamaz (CSP `connect-src ipc:`).
- İndirilen her dosya hash ile doğrulanır (Mojang SHA-1, Modrinth SHA-512, Adoptium SHA-256);
  doğrulanmayan dosya kullanılmaz. Belleğe okunan yanıtlar 64 MB ile sınırlıdır.
- Uzak görseller Rust'ta indirilir, yalnızca PNG/JPEG/GIF/WebP `data:` URI olarak verilir (SVG yok).

**Dosya sistemi**
- Zip çıkarma: mutlak yol, `..`, sembolik bağlantı içeren girdiler reddedilir; hedef köke göre denetlenir.
- Profil id'leri ve dosya adları yol geçişine karşı doğrulanır.
- Webview komutlara **dosya yolu gönderemez**; içe/dışa aktarma diyalogları Rust'ta açılır ve
  seçilen yol yalnızca ilgili komut tarafından bir kez kullanılır. Webview'a `dialog`, `fs`,
  `shell`, `http` izni verilmez (`capabilities/default.json`).
- Asset protokolü kapsamı başlangıçta boştur; yalnızca listelenen profilin ekran görüntüsü klasörü açılır.

**Süreç başlatma**
- Shell kullanılmaz; argümanlar dizi olarak verilir.
- Çalıştırılabilecek tek program Java'dır: profil Java yolu `java.exe`/`javaw.exe` adında mutlak bir yol olmalıdır.
- Komut çalıştıran JVM seçenekleri (`-XX:OnError`, `-XX:OnOutOfMemoryError`) reddedilir;
  içe aktarılan profillerden Java yolu ve bu seçenekler silinir.

**Gizlilik**
- Telemetri yok. Microsoft/premium girişi yok; offline hesaplarda parola/token tutulmaz.
- Loglarda token, parola, `x-api-key` ve CurseForge anahtarları maskelenir.
- CurseForge API anahtarı (kullanıcı girerse) `settings.json`'da düz metin durur — yalnızca
  bu bilgisayarda ve yalnızca `api.curseforge.com` isteklerinde kullanılır.

**Güncellemeler**
- Güncellemeler minisign ile imzalanır; açık anahtar uygulamaya gömülüdür, imzasız veya
  değiştirilmiş paket kurulmaz. Özel anahtar repoda değildir.

## Bilinen sınırlamalar

- Kod imzalama sertifikası yok (SmartScreen uyarısı).
- Kullanıcının kendi eklediği JVM argümanları (ör. `-javaagent`) ve modlar kendi
  sorumluluğundadır; modlar Java kodudur ve tam yetkiyle çalışır.
- Bazı Forge kurulum adımları (processor'lar) Forge'un kendi araçlarıyla ağ isteği yapar.
- `cargo audit`: bilinen açık yok; Linux GTK bağımlılıklarında iki uyarı (`glib`
  RUSTSEC-2024-0429, `proc-macro-error` bakımsız) — Windows derlemesinde kullanılmaz.

## Bildirim

Bir güvenlik açığı bulursan lütfen herkese açık issue açma; depo sahibine (GitHub: Mehbur07)
özel olarak bildir. Mümkünse yeniden üretme adımlarını ve etkilenen sürümü ekle.
