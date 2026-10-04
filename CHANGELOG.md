# Değişiklik Günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/) temellidir; sürümler
[SemVer](https://semver.org/lang/tr/) izler.

## [Yayınlanmadı] — 0.1.0

İlk sürüm adayı. Geliştirme fazları ve kararlar için [ARCHITECTURE.md](ARCHITECTURE.md).

### Eklendi
- **Temel (Faz 1):** Tauri 2 + React kabuğu, neon tema ve vurgu renkleri, çerçevesiz pencere,
  TR/EN arayüz, `Paths` (standart / portable / yönlendirme), dönen loglar, hata modeli.
- **Vanilla başlatma (Faz 2):** sürüm manifesti, paralel ve devam ettirilebilir indirici,
  kütüphane/native/asset kurulumu, otomatik Java (Temurin), offline başlatma, eski sürüm desteği.
- **Profiller (Faz 3):** izole profiller, sürükle-bırak sıralama, kopyala/içe/dışa aktar,
  canlı konsol, indirme kuyruğu (duraklat/devam), offline hesaplar, oynama süresi.
- **Mod yükleyiciler (Faz 4):** Fabric, Quilt, Legacy Fabric, Forge (1.7–güncel), NeoForge,
  OptiFine içe aktarma, tek tıkla shader desteği (Iris/Sodium, Oculus/Embeddium).
- **İçerik (Faz 6):** Modrinth tarayıcısı (mod, modpack, resource pack, shader), zorunlu
  bağımlılık çözümü, güncelleme denetimi, `.mrpack` ve CurseForge içe aktarma.
- **Skin & Cape (Faz 7):** skin/pelerin kütüphanesi, 3B önizleme, CustomSkinLoader ile oyunda
  gösterim.
- **Cila (Faz 8):** Minecraft haberleri, çöküş analizi ve çözüm önerileri, gerçek "oyun
  açılınca kapat", veri klasörünü taşıma, imzalı otomatik güncelleme, NSIS kurulum + portable
  zip, CI iş akışları, animasyonlar, Ayarlar'da Oyun bölümü.
- **Sertleştirme (Faz 9):** dosya diyalogları Rust'a taşındı (webview dosya yolu gönderemez,
  `dialog` izni kaldırıldı); Java yolu ve tehlikeli JVM argümanı doğrulaması; içe aktarılan
  profillerin temizlenmesi; ağ yanıtı boyut sınırı; API anahtarı log maskelemesi; sayfa bazlı
  kod bölme (başlangıç JS'i −%19); uçtan uca hazırlık/içerik/Java entegrasyon testleri;
  kullanım kılavuzu, sorun giderme, geliştirici rehberi, güvenlik politikası.

- **Skin stüdyosu (Faz 10):** hazır skin ve pelerinler (oyunun varsayılan skinleri kurulu
  sürümden okunur + 11 skin / 10 pelerinlik özgün MehburMC koleksiyonu); canlı 3B önizlemeli
  piksel editörü (kalem, silgi, kova, damlalık, çizgi, ayna modu, katmanlar, geri al/ileri al).
  MehburMC skin'i ve pelerini her kullanıcının kütüphanesine hazır gelir.

### Kaldırıldı
- Premium kavramı (Faz 10): "premium değil" rozeti ve oyuncu adından skin kopyalama.
- Microsoft girişi — kullanıcı isteğiyle (yalnızca offline hesaplar).
