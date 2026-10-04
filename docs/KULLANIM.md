# MehburMC Launcher — Kullanım Kılavuzu

MehburMC Launcher, Minecraft: Java Edition için hafif, reklamsız ve telemetrisiz bir launcher'dır.
Yalnızca **offline hesaplarla** çalışır.

## İçindekiler

1. [Kurulum](#1-kurulum)
2. [İlk açılış: hesap ekle](#2-ilk-açılış-hesap-ekle)
3. [Profil (instance) oluşturma](#3-profil-instance-oluşturma)
4. [Oynamak](#4-oynamak)
5. [Mod, resource pack, shader ve modpack](#5-mod-resource-pack-shader-ve-modpack)
6. [Skin ve pelerin](#6-skin-ve-pelerin)
7. [Profil ayrıntıları](#7-profil-ayrıntıları)
8. [Ayarlar](#8-ayarlar)
9. [Veri klasörü, taşıma ve portable kullanım](#9-veri-klasörü-taşıma-ve-portable-kullanım)
10. [Güncellemeler](#10-güncellemeler)

---

## 1. Kurulum

İki seçenek vardır:

| | Kurulum (`…_x64-setup.exe`) | Portable (`…_x64-portable.zip`) |
|---|---|---|
| Nereye kurulur | `%LOCALAPPDATA%\MehburMC Launcher` (yönetici izni gerekmez) | Zip'i çıkardığın klasör |
| Oyun verileri | `%APPDATA%\MehburMC\game\mc` | Exe'nin yanındaki `MehburMC\` klasörü |
| Güncelleme | Launcher içinden, otomatik | Yeni zip'i klasörün üzerine çıkar |

Gereksinim: Windows 10/11 ve WebView2 (Windows 11'de hazır gelir). Java'yı kurmana gerek yoktur;
launcher, sürümün istediği Java'yı (8, 17, 21, 25 …) Eclipse Temurin'den kendisi indirir.

> İlk çalıştırmada Windows SmartScreen "tanınmayan uygulama" uyarısı verebilir (kod imzalama
> sertifikası yok). **Ek bilgi → Yine de çalıştır** ile devam edebilirsin.

## 2. İlk açılış: hesap ekle

**Hesaplar** ekranında bir oyuncu adı yaz (3–16 karakter; harf, rakam, `_`) ve **Ekle**'ye bas.
Birden fazla hesap ekleyip **Kullan** ile aralarında geçiş yapabilirsin.

Offline hesaplar:
- Tek oyunculu oyunda ve `online-mode=false` olan sunucularda çalışır.
- Çevrimiçi (online-mode) sunuculara giremez.

## 3. Profil (instance) oluşturma

**Profiller → Profil oluştur**:

1. **Sürüm** seç (sürümler, snapshot'lar, eski alpha/beta sürümler filtrelenebilir).
2. **Mod yükleyici** seç: Vanilla, Fabric, Quilt, Legacy Fabric, Forge, NeoForge veya OptiFine.
   - OptiFine'ın resmî bir indirme API'si yoktur: jar'ı optifine.net'ten kendin indirip
     **OptiFine jar'ı seç…** ile göster.
3. İsteğe bağlı: ad, simge, Java, RAM, JVM argümanları, çözünürlük.

Her profil kendi `mods`, `saves`, `config`, `resourcepacks` klasörleriyle **izole** çalışır;
sürüm dosyaları ve kütüphaneler profiller arasında paylaşılır (disk tasarrufu).

## 4. Oynamak

**Oyna** ekranında profili seç ve **OYNA**'ya bas. İlk başlatmada eksik dosyalar indirilir
(ilerleme **İndirmeler** ekranında); sonraki başlatmalar saniyeler içinde hazırdır.

- **Konsol**: oyunun çıktısını canlı gösterir; seviyeye göre filtreleyebilir, arayabilir,
  kopyalayabilir veya dosyaya kaydedebilirsin.
- **Durdur**: oyunu kapatır.
- **Onar**: tüm oyun dosyalarını SHA-1 ile doğrular, bozukları yeniden indirir.

### Oyun çökerse

Oyun beklenmedik şekilde kapanırsa bir **çöküş penceresi** açılır. Launcher; crash raporunu,
oyun logunu ve Java'nın hata dosyasını inceleyip olası nedeni ve çözümü gösterir (ör. "bellek
yetmedi → RAM'i artır", "eksik bağımlılık → Fabric API'yi kur", "ekran kartı sürücüsü").
Pencereden doğrudan profil ayarlarına, mod listesine veya **Onar**'a gidebilirsin.
Eski crash raporları için: profil ayrıntıları → **Loglar** → rapor → **Analiz et**.

## 5. Mod, resource pack, shader ve modpack

**Mod Tarayıcı** Modrinth'te arama yapar. Hedef profili seçip **Kur**'a bas:
- Profilin Minecraft sürümüne ve mod yükleyicisine uygun en yeni sürüm seçilir.
- Modların **zorunlu bağımlılıkları** otomatik kurulur; zaten kurulu olanlar atlanır.
- Tüm dosyalar SHA-512 ile doğrulanır.

Profil ayrıntılarındaki **Modlar** sekmesinde modları açıp kapatabilir, silebilir ve
**güncellemeleri** denetleyip tek tıkla güncelleyebilirsin.

**Modpack'ler**:
- Mod Tarayıcı'da tür olarak **Modpack** seçip kur, ya da
- **Profiller → Modpack içe aktar** ile `.mrpack` veya CurseForge `.zip` dosyası seç.
  CurseForge paketleri için kendi API anahtarın gerekir (**Ayarlar → Gelişmiş**).

**Shader desteği**: Fabric/Quilt/NeoForge'da Iris + Sodium, Forge'da Oculus + Embeddium tek
tıkla kurulur (profil → Genel → **Tek tıkla kur**).

## 6. Skin ve pelerin

**Skin & Cape** ekranı:
- **Dosyadan ekle**: 64×64 veya 64×32 (ya da HD katları) PNG skin, 64×32 pelerin.
- Kütüphanende **MehburMC** skin'i ve pelerini hazır gelir (silersen geri eklenmez).
- **Hazırlar**: oyunun varsayılan skinleri (Steve, Alex… — kurulu bir sürümün oyun dosyasından
  okunur) ve MehburMC koleksiyonundaki özgün skin/pelerinler. Tıkla önizle, **+** ile kütüphaneye ekle.
- **Tasarla**: kendi skin'ini (64×64) veya pelerinini (64×32) çiz; soldaki 3B önizleme anında
  güncellenir. Araçlar: kalem (B), silgi (E), kova (G), damlalık (I), çizgi (L), ayna modu (M);
  sağ tık siler, Alt+tık renk alır, Ctrl+Z / Ctrl+Y geri/ileri alır. **Katman** ile yalnızca
  taban veya dış katmana (şapka, ceket, kol/paça) çizebilirsin. Kütüphanedeki ya da hazır bir
  skin'i fırça simgesiyle editörde açıp değiştirebilirsin; **Kütüphaneye kaydet** ile eklenir.
- Karta tıklayınca 3B önizlemede görürsün (yürüme/koşma animasyonu, pelerin/elytra);
  **… için uygula** ile seçili hesaba atarsın. Kol modeli (Klasik/İnce) otomatik algılanır,
  karttan değiştirilebilir.

**Oyunda görünmesi için**: offline skin'ler vanilla oyunda görünmez. Fabric, Quilt, Forge veya
NeoForge profillerine aynı ekrandaki **CustomSkinLoader kur** düğmesiyle modu ekle; launcher her
OYNA'da seçili hesabın skin'ini ve pelerinini oyuna aktarır. Diğer oyuncular skin'ini yalnızca
onlarda da CustomSkinLoader ve aynı dosya varsa görür.

## 7. Profil ayrıntıları

Profile çift tıkla: **Genel** (oynama süresi, son oynama, loader), **Modlar**, **Resource Pack**,
**Shader Pack**, **Dünyalar**, **Ekran görüntüleri**, **Loglar** ve **Ayarlar** (ad, simge,
Java, RAM, JVM argümanları, çözünürlük).

Profil menüsünden (⋮) kopyalama, `.zip` olarak dışa aktarma ve silme yapılabilir; başka bir
bilgisayara taşımak için **Profiller → İçe aktar**.

> Güvenlik: içe aktarılan profillerde Java yolu sıfırlanır ve komut çalıştırabilen JVM
> seçenekleri (`-XX:OnError`, `-XX:OnOutOfMemoryError`) kaldırılır. Profillerde Java yolu
> olarak yalnızca `java.exe` / `javaw.exe` kabul edilir.

## 8. Ayarlar

- **Görünüm**: dil (sistem / Türkçe / English), vurgu rengi, arka plan efektleri.
- **Oyun**: oyun açılınca launcher'ın ne yapacağı (küçült / kapat / açık bırak), varsayılan
  RAM, eşzamanlı indirme sayısı.
  - "Kapat" seçiliyse launcher oyun ana menüye gelince kapanır, oyun çalışmaya devam eder
    (o oturumun oynama süresi kaydedilmez).
- **Veri klasörü**: konum, klasörü aç, taşı (aşağıda).
- **Gelişmiş**: hata ayıklama günlüğü, CurseForge API anahtarı.
- **Hakkında**: sürüm, otomatik güncelleme denetimi.

## 9. Veri klasörü, taşıma ve portable kullanım

Varsayılan konum `%APPDATA%\MehburMC\game\mc\`:

```
launcher\    settings.json, accounts.json, state.json (hiç taşınmaz)
versions\  libraries\  assets\  runtime\ (Java)
instances\   profiller
skins\  modpacks\  cache\  logs\
```

**Veri klasörünü taşı** (Ayarlar → Veri klasörü): oyun dosyalarını başka bir diske/klasöre
taşır. Dosyalar kopyalanır, doğrulanır, eski konumdan silinir; ardından launcher yeniden
başlar. Hedef klasör boş olmalıdır. Oyun veya indirme çalışırken taşıma yapılamaz.
**Varsayılan konuma geri taşı** ile geri dönebilirsin.

**Portable**: portable zip'te exe'nin yanında `portable.flag` dosyası bulunur; bu dosya
olduğu sürece tüm veriler exe'nin yanındaki `MehburMC\` klasöründe tutulur. Klasörü USB
belleğe kopyalayıp başka bir bilgisayarda kullanabilirsin.

## 10. Güncellemeler

Kurulu sürüm açılışta yeni sürüm olup olmadığına bakar (Ayarlar'dan kapatılabilir). Yeni
sürüm varsa üstte bir şerit çıkar; **Güncelle ve yeniden başlat** imzalı güncellemeyi indirir,
doğrular ve kurar. Portable sürüm kendini güncellemez.

Sorun mu yaşıyorsun? → [Sorun giderme](SORUN_GIDERME.md)
