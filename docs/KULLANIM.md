# MehburMC Launcher — Kullanım Kılavuzu

MehburMC Launcher, Minecraft: Java Edition için hafif, reklamsız ve telemetrisiz bir launcher'dır.
Minecraft'a **offline hesaplarla** girer; launcher'ın kendisi için ücretsiz bir
**MehburMC hesabı** (e-posta + şifre) gerekir.

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
11. [Arkadaşlar](#11-arkadaşlar)
12. [MehburMC hesabı](#12-mehburmc-hesabı)

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

## 2. İlk açılış: hesap oluştur

**Hesaplar → Yeni hesap oluştur** kısmına bir oyuncu adı yaz (3–16 karakter; harf, rakam, `_`)
ve **Oluştur**'a bas. Birden fazla hesap oluşturup **Kullan** ile aralarında geçiş yapabilirsin.

**Kullanıcı adları benzersizdir:** bir ad tüm MehburMC kullanıcıları arasında yalnızca bir kişiye
aittir (büyük/küçük harf fark etmez). Başkası almışsa "Bu kullanıcı adı alındı. Lütfen başka bir
ad seçiniz." yazar. Bu yüzden hesap oluştururken ve adı değiştirirken **internet gerekir**; ad
MehburMC sunucusuna kaydedilir. Hesabı silince adı serbest kalır. Bir bilgisayardan en fazla 10
ad alınabilir. Bu özellik gelmeden önce oluşturduğun bir hesabın adını başkası senden önce
aldıysa kartında **"Bu ad başka bir kullanıcıda — yeniden adlandır"** uyarısı çıkar; hesap yine
çalışır, ama yeni bir ad seçmen önerilir.

**Adı değiştirmek** için hesabın yanındaki kalem simgesine bas, yeni adı yaz ve **Kaydet**'e bas.
Yeni ad bir sonraki OYNA'da oyunda görünür. Oyuncu kimliği (UUID) yeni ada göre değişir; bu
yüzden tek oyunculu dünyalarda karakter yeni oyuncu gibi başlar. Skin ve pelerin ataması korunur.

**Profil fotoğrafı:** her hesabın bir fotoğrafı vardır. Başta bu, hesabın skininin kafasıdır
(skin atamadıysan oyunun o hesaba verdiği varsayılan skin: Steve, Alex, …). Fotoğrafın sol
altındaki **+** ile bilgisayarından bir `.png` seçebilirsin; ortadan kare kırpılıp küçültülür.
**Skin kafasına dön** simgesi fotoğrafı kaldırır. Arkadaşlar açıksa seçili hesabının fotoğrafını
arkadaşların da görür.

Hesaplar:
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

**MehburMC Kütüphanesi** (Mod Tarayıcı'nın son sekmesi): MehburMC kullanıcılarının yüklediği
modlar.
- **Mod yükle** → jar'ını seç. Launcher dosyayı hemen kontrol eder ve sonucu gösterir:
  yeşil = şüpheli bir şey yok, sarı = dikkat edilecek davranışlar var (ör. internete bağlanma;
  meşru modlarda da olur), kırmızı = yüklenemez (ör. Discord webhook'u, hesap dosyalarına
  erişim, gömülü .exe, mod tanımı yok, 25 MB'den büyük). Ad ve açıklama yazıp **Onaya gönder**.
- Yüklediğin modlar bir admin onaylayana kadar yalnızca sana, **Yüklediklerim** bölümünde
  "Onay bekliyor" olarak görünür. **Geri çek** modu sunucudan siler.
- Onaylı modu **Kur** ile hedef profile kurarsın. İndirilen dosya doğrulanır ve senin
  launcher'ında yeniden kontrol edilir; uyarı varsa önce sorulur. Hiçbir otomatik kontrol
  %100 güvenlik sağlamaz — şüpheli bir şey görürsen bayrak simgesiyle **Bildir**.

## 6. Skin ve pelerin

**Skin & Cape** ekranı:
- **Dosyadan ekle**: 64×64 veya 64×32 (ya da HD katları) PNG skin, 64×32 pelerin.
- **Özel MehburMC skin/pelerinleri** yalnızca kurucunun gönderdiği hesaplarda görünür; kartlarında
  kilit simgesi vardır ve paylaşılamaz. Geri alınırsa kütüphanenden kaldırılır.
- **Hazırlar**: oyunun varsayılan skinleri (Steve, Alex… — kurulu bir sürümün oyun dosyasından
  okunur) ve launcher'ın özgün skin/pelerin koleksiyonu. Tıkla önizle, **+** ile kütüphaneye ekle.
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

## 11. Arkadaşlar

Kenar çubuğundaki **Arkadaşlar** ile arkadaşlarınla mesajlaşabilir ve birbirinizin mod
listelerini görüp tek tıkla kurabilirsiniz. Özellik **varsayılan olarak kapalıdır**; ilk
açışta neyin saklandığını anlatan bir onay ekranı çıkar.

- **Arkadaş kodu:** açtığında sana `MEHBUR-XXXX` biçiminde bir kod verilir. Kodunu arkadaşına
  gönder; o da **Kodla arkadaş ekle** kutusuna yazar. İstek sende **Arkadaşlık istekleri** bölümünde
  görünür; **Kabul et** dersen arkadaş olursunuz.
- **Sohbet:** listeden arkadaşını seç. Enter gönderir, Shift+Enter yeni satır; mesajlar en
  fazla 2000 karakter. Okunmamış mesaj sayısı kenar çubuğunda görünür.
- **Mod paylaşma:** sayfanın altındaki **Paylaştığım profiller** bölümünde bir profili
  **Paylaş**. Modrinth'te bulunan modlar yalnızca bağlantı olarak, diğer jar'lar (≤50 MB)
  dosya olarak paylaşılır. Modları değiştirdikten sonra **Güncelle**, bırakmak için **Kaldır**.
- **Arkadaşın modlarını kurma:** arkadaşını seç → **Modları** sekmesi → paylaştığı profil →
  istediğin modları işaretle, hedef profilini seç → **Seçilenleri kur**. "Modrinth ✓" olanlar
  Modrinth'ten indirilir; "Doğrulanmamış" olanlar arkadaşının yüklediği dosyalardır, yalnızca
  güvendiğin kişilerden kur. Zaten kurulu olan dosyalar atlanır.
- **Çevrimiçi:** launcher açıkken arkadaşların seni çevrimiçi görür; çevrimiçi olanların
  fotoğrafının sağ altında **neon lime bir daire** yanar ve listenin en üstünde dururlar. Senin
  aktif hesabının fotoğrafında da daire, arkadaşlarına çevrimiçi göründüğünü gösterir.
  Launcher'ı kapatınca (ya da internet gidince en geç ~1,5 dakikada) çevrimdışı görünürsün.
- **Sil / engelle:** sohbetin üstündeki simgelerle. Engellenen kişi sana mesaj ve istek
  gönderemez; engeli listedeki **Engeli kaldır** ile açabilirsin.
- **Kapatma:** **Arkadaşları kapat ve verilerimi sil** hesabını, mesajlarını, arkadaşlıklarını
  ve yüklediğin dosyaları sunucudan siler.

Saklanan veriler: arkadaş kodun, görünen adın (seçili hesabının adı), mesajların ve paylaştığın
mod listeleri/dosyaları. Bunları yalnızca sen ve arkadaşların görebilir. Arkadaşlar internet
gerektirir; bağlantı yoksa sayfada "çevrimdışı" yazar, launcher'ın geri kalanı etkilenmez.

## 12. MehburMC hesabı

Launcher açılınca MehburMC hesabınla giriş yapman istenir.

- **Hesap oluştur:** e-posta ve şifre (en az 8 karakter) yaz. E-postana gelen **6 haneli kodu**
  launcher'a gir. Kod gelmezse spam klasörüne bak veya **Kodu tekrar gönder**.
- Launcher'ın eski bir sürümünde arkadaşları ya da hesap adlarını kullandıysan, hesap oluşturmak
  bu bilgisayardaki kimliğini hesaba **dönüştürür**: arkadaşların, ayrılmış adların ve
  paylaşımların korunur. (Bunun yerine başka bir hesaba giriş yaparsan o eski kimlik silinir.)
- **Şifremi unuttum:** e-postanı yaz, gelen kodu ve yeni şifreni gir.
- Bir kez giriş yaptıktan sonra internet yokken de oynayabilirsin.
- **Ayarlar → MehburMC hesabı → Çıkış yap** bu bilgisayardan çıkış yapar; profillerin, modların ve
  dünyaların silinmez.
- Kurallara uymayan hesaplar yöneticiler tarafından süreli ya da kalıcı **yasaklanabilir**;
  yasaklıyken launcher kullanılamaz, ekranda bitiş tarihi ve sebep yazar.

**Yöneticiler** kenar çubuğunda **Admin** sayfasını görür: Kütüphane'ye yüklenen modların onayı
(mod önce yöneticinin launcher'ında yeniden taranır), bildirilen içerik, banlar ve yönetici atama. Kurucu **Topluluk** sekmesinden paylaşılan skin/pelerinleri kaldırır
(ve geri getirir), ayrıca **Özel dokular** sekmesinden özel
MehburMC skin/pelerinlerini yükler ve seçtiği kişilere gönderir.

Sorun mu yaşıyorsun? → [Sorun giderme](SORUN_GIDERME.md)
