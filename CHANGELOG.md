# Değişiklik Günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/tr/1.1.0/) temellidir; sürümler
[SemVer](https://semver.org/lang/tr/) izler.

## [0.5.0] — 2026-10-06

### Eklendi
- **Çevrimiçi göstergesi:** launcher açıkken arkadaşların seni çevrimiçi görür. Çevrimiçi
  olanların profil fotoğrafının sağ altında neon lime bir daire yanar ve listenin en üstüne
  çıkarlar. Kendi aktif hesabının fotoğrafındaki daire, arkadaşlarına çevrimiçi göründüğünü
  gösterir; launcher'ı kapatınca çevrimdışı olursun.

## [0.4.0] — 2026-10-05

### Eklendi
- **Profil fotoğrafı:** Hesaplar'da her hesabın fotoğrafı var. Başta skininin kafası (skin
  yoksa oyunun o hesaba verdiği varsayılan skin); sol alttaki **+** ile bilgisayarından `.png`
  seçebilirsin. **Skin kafasına dön** ile geri alırsın.
- Arkadaşların seçili hesabının fotoğrafını Arkadaşlar listesinde ve sohbette görür.

### Düzeltildi
- Arkadaşın paylaştığı Modrinth modları artık sürüm adı yerine mod adıyla görünüyor.

## [0.3.0] — 2026-10-05

### Eklendi
- **Arkadaşlar:** arkadaş koduyla (`MEHBUR-XXXX`) arkadaş ekle, mesajlaş, okunmamış mesajları
  kenar çubuğunda gör. İsteğe bağlıdır; ilk açışta onay istenir.
- **Mod paylaşma:** bir profilin mod listesini arkadaşlarınla paylaş; arkadaşının modlarını
  seçip kendi profiline tek tıkla kur. Modrinth'teki modlar Modrinth'ten indirilir, diğerleri
  arkadaşının yüklediği dosyadan doğrulanarak gelir.
- Arkadaşları kapatınca tüm verilerin sunucudan silinir.

### Değişti
- Yeni MehburMC logosu (uygulama ikonu, kurulum ve başlık çubuğu).

## [0.2.2] — 2026-10-04

### Değişti
- İndirme sayfası sadeleşti: sürümlerde yalnızca kurulum dosyası (`setup.exe`) var.
- Güncelleme bilgisi artık ayrı bir yerden okunuyor. **0.2.1 ve öncesinden** gelenler bu sürümü
  bir kez elle kurmalı; sonraki güncellemeler yine Güncelle butonuyla gelir.

## [0.2.1] — 2026-10-04

### Değişti
- Başlık çubuğunda "MehburMC Launcher" yazısının yanında yüklü sürüm görünüyor; **Güncelle**
  butonu sürümün hemen yanında.

## [0.2.0] — 2026-10-04

### Eklendi
- **Güncelle butonu:** yeni sürüm çıktığında başlık çubuğunda, "MehburMC Launcher" yazısının
  yanında görünür. Tıklayınca güncelleme indirilir, kurulum açılır ve launcher yeni sürümle
  yeniden başlar.
- **Yenilikler sekmesi:** güncellemeden sonraki ilk açılışta bu sayfa kendiliğinden açılır;
  önceki sürümlerin notları da burada.
- **Skin stüdyosu:** hazır skin ve pelerinler (oyunun varsayılan skinleri kurulu sürümden
  okunur + 12 skin / 10 pelerinlik özgün MehburMC koleksiyonu) ve canlı 3B önizlemeli piksel
  editörü (kalem, silgi, kova, damlalık, çizgi, ayna modu, katmanlar, geri al/ileri al).
- **MehburMC skin'i ve pelerini** her kullanıcının kütüphanesine hazır gelir.
- **Hesap adını değiştirme:** Hesaplar ekranında kalem simgesiyle; oyundaki ad yeni ada göre
  değişir, skin atamaları korunur.

### Değişti
- Hesaplar ekranında "Offline hesap" yazıları kaldırıldı; kutu artık "Yeni hesap oluştur".
- Kurulum dosyası (setup .exe) artık MehburMC logosunu taşıyor.
- Proje GitHub'da herkese açık; otomatik güncelleme bu sürümle çalışmaya başladı.

### Kaldırıldı
- Premium kavramı: "premium değil" rozeti ve oyuncu adından skin kopyalama.

## [0.1.0] — 2026-10-04

İlk sürüm. Geliştirme fazları ve kararlar için [ARCHITECTURE.md](ARCHITECTURE.md).

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

### Kaldırıldı
- Microsoft girişi — kullanıcı isteğiyle (yalnızca offline hesaplar).
