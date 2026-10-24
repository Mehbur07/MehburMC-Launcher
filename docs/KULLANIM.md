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
| Nereye kurulur | `%APPDATA%\MehburMC\app` (yönetici izni gerekmez) | Zip'i çıkardığın klasör |
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

Altındaki **Minecraft haberleri** ana sayfa açıkken birkaç dakikada bir kendiliğinden yenilenir;
launcher açıkken yayınlanan haberlerde **Yeni** rozeti çıkar.

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
  CurseForge `.zip` paketleri çoğu zaman kurulamaz; mümkünse paketin Modrinth (`.mrpack`) sürümünü kullan.

**Shader desteği**: Fabric/Quilt/NeoForge'da Iris + Sodium, Forge'da Oculus + Embeddium tek
tıkla kurulur (profil → Genel → **Tek tıkla kur**).

**MehburMC Kütüphanesi** (Mod Tarayıcı'nın son sekmesi): MehburMC kullanıcılarının yüklediği
modlar, modpackler, resource packler ve shaderlar. Üstteki **Tümü / Modlar / Modpackler /
Resource packler / Shaderlar** düğmeleriyle türe göre süz.
- **Yükle** → türü seç → dosyayı seç: mod = `.jar` (en fazla 25 MB), modpack = Modrinth
  `.mrpack`, resource pack ve shader = `.zip` (paketler en fazla 50 MB). Launcher dosyayı hemen
  kontrol eder ve sonucu gösterir: yeşil = şüpheli bir şey yok, sarı = dikkat edilecek şeyler var
  (ör. internete bağlanma — meşru modlarda da olur; modpack'te Modrinth dışından indirilen dosya),
  kırmızı = yüklenemez (ör. Discord webhook'u, hesap dosyalarına erişim, gömülü .exe, mod tanımı
  yok, resource pack'te `pack.mcmeta` yok, pakette kod dosyası, boyut sınırı aşıldı). Ad ve
  açıklama yazıp **Onaya gönder**. Kütüphanedeki yüklemelerinin toplamı en fazla 200 MB olabilir.
- Yüklediklerin bir admin onaylayana kadar yalnızca sana, **Yüklediklerim** bölümünde
  "Onay bekliyor" olarak görünür. **Geri çek** dosyayı sunucudan siler.
- Onaylı modu, resource pack'i veya shader'ı **Kur** ile hedef profile kurarsın (shader için
  profilde Iris veya OptiFine gerekir). Modpack'i **Profil olarak kur** yeni bir profil yapar;
  Profiller sayfasından başlatırsın. İndirilen dosya doğrulanır ve senin launcher'ında yeniden
  kontrol edilir; uyarı varsa önce sorulur. Hiçbir otomatik kontrol %100 güvenlik sağlamaz —
  şüpheli bir şey görürsen bayrak simgesiyle **Bildir**.

### Aynı profili iki hesapla açmak

Oyun açıkken **Hesaplar**'dan başka bir hesap seçip **OYNA**'ya basarsan aynı profil ikinci kez
açılır (ör. ana hesabınla oynarken bir sunucuda AFK hesabın açık kalabilir). Oyna sayfası
açık olan her oyunu hesap adıyla ve ayrı bir **Durdur** düğmesiyle gösterir. Aynı hesapla iki
kez girilemez. İki oyun aynı profil klasörünü ve aynı konsolu paylaşır; oyunlardan biri
açıkken mod ekleme/silme ve onarım kilitlidir. İki oyun için yeterli RAM'in olduğundan emin ol.

## 6. Skin ve pelerin

**Skin & Cape** ekranı:
- **Dosyadan ekle**: 64×64 veya 64×32 (ya da HD katları) PNG skin, 64×32 pelerin.
- **Özel MehburMC skin/pelerinleri** yalnızca kurucunun gönderdiği hesaplarda görünür; kartlarında
  kilit simgesi vardır ve paylaşılamaz. Geri alınırsa kütüphanenden kaldırılır.
- **Hazırlar**: oyunun varsayılan skinleri (Steve, Alex… — kurulu bir sürümün oyun dosyasından
  okunur) ve launcher'ın özgün koleksiyonu: **104 skin, 104 pelerin**. Üstteki kutuyla adına göre
  ara; liste ilk 24 tasarımla açılır, **Tümünü göster** ile hepsi görünür. Tıkla önizle, **+** ile kütüphaneye ekle.
- **Hazırlar → SkinMC**: skinmc.net'teki skinler. Sıralama seç (en yeni, bugün, bu hafta, bu ay,
  rastgele) ya da etiket ara (ör. `cat`, `ninja`); **Daha fazla yükle** ile sonraki sayfa gelir.
  Skinler yaptıkları kişilere aittir; launcher yalnızca baktığın sayfayı indirir.
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
    (o oturumun oynama süresi kaydedilmez). Sesli aramadaysan launcher kapanmaz, küçülür.
- **Sesli sohbet**: mikrofon seçimi, mikrofon testi (konuşunca çubuk hareket eder) ve
  **IP adresimi gizle** (bkz. Arkadaşlar → Sesli sohbet).
- **Veri klasörü**: konum, klasörü aç, taşı (aşağıda).
- **Gelişmiş**: hata ayıklama günlüğü.
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
- **Takma ad:** sohbetin üstündeki kalem simgesine bas ve arkadaşına kendi verdiğin adı yaz
  (en fazla 32 karakter). Bu ad listede, sohbette ve gruplarda görünür; **yalnızca sen
  görürsün**, arkadaşın bilmez. Gerçek adı sohbet başlığında parantez içinde durur.
  Kaldırmak için aynı pencerede **Takma adı kaldır**.
- **Grup sohbeti:** soldaki **Gruplar** bölümünde **+** → grup adı yaz, arkadaşlarını işaretle →
  **Oluştur**. Bir grupta seninle birlikte en fazla 16 kişi olur. Grubu kuran kişi **Kurucu**dur:
  adı değiştirir, **Üyeler** penceresinden arkadaşlarını ekler veya üyeleri çıkarır. Herkes
  **Gruptan ayrıl** diyebilir; kurucu ayrılırsa grup en eski üyeye geçer, son kişi ayrılınca grup
  silinir. Gruba eklediğin kişiler senin arkadaşın olmalı; gruptaki diğer üyelerle arkadaş
  olmanız gerekmez. Sonradan katılan biri eski mesajları görür ama okunmamış sayılmaz.
- **Sesli sohbet:** arkadaşının sohbet başlığındaki **telefon** simgesiyle onu ara; bir grupta
  **Sesli sohbet başlat** (sürüyorsa **Sesli sohbete katıl**). Aranan kişide sağ üstte zil sesiyle
  bir kart çıkar ve görev çubuğundaki simge yanıp söner; **Kabul et** / **Reddet**. 45 saniye
  cevaplanmayan arama cevapsız sayılır. Aramadayken sağ altta bir çubuk durur: konuşanın
  fotoğrafının etrafında yeşil halka, her kişi için ses düzeyi, **Mikrofonu kapat**, **Sesi kapat**
  (mikrofonu da kapatır) ve **Aramadan ayrıl**. Bir grup aramasına seninle birlikte 16 kişi
  katılabilir; grupta herkes ayrılınca arama biter, ikili aramada biri kapatınca. Aynı anda tek
  bir aramada olabilirsin. Launcher küçültülmüşken ya da oyun açıkken arama sürer. Mikrofon
  bulunamazsa ya da Windows izin vermezse yalnızca dinlersin (Windows Ayarları → Gizlilik ve
  güvenlik → Mikrofon → masaüstü uygulamaları). Ses kişiden kişiye gider, sunucuda kaydedilmez.
  **IP adresimi gizle** (Ayarlar → Sesli sohbet, varsayılan açık) sesi Cloudflare aktarım
  sunucusundan geçirir; böylece aramadaki kişiler (gruptaki tanımadıkların dahil) IP adresini
  göremez.
- **Ekran paylaşma:** aramadayken çubuktaki **Ekranı paylaş** simgesine bas, kaliteyi seç
  (720p 30 FPS / 1080p 30 FPS / 1080p 60 FPS — oyun için 60) ve **Ekran veya pencere seç**.
  Açılan pencerede tüm ekranı ya da tek bir pencereyi (ör. Minecraft) seç; bilgisayar sesinin de
  gitmesini istiyorsan **Sistem sesini paylaş** kutusunu işaretle. Paylaşırken çubukta küçük bir
  önizleme görünür; **Paylaşımı durdur** ile bitir. Grupta herkes görür; kişi sayısı arttıkça
  internet yükün bölüşüldüğü için kalite otomatik düşer. Karşı tarafta paylaşım, arama çubuğunun
  solunda bir panelde açılır: **Tam ekran** (Esc ile çık), **Küçült**, **Gizle**; birden çok kişi
  paylaşıyorsa üstteki adlarla seç. Oyun tam ekran (exclusive) modundaysa pencere siyah
  görünebilir; oyunu pencereli/kenarlıksız modda paylaş.
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

Saklanan veriler: arkadaş kodun, görünen adın (seçili hesabının adı), mesajların, grupların ve
grup mesajların, verdiğin takma adlar, kısa süreli arama kayıtları (kim kimi aradı; arama bittikten
bir gün sonra silinir) ve paylaştığın mod listeleri/dosyaları. Bunları yalnızca sen
ve arkadaşların (grup mesajlarını grubun üyeleri, takma adları yalnızca sen) görebilir. Arkadaşlar internet
gerektirir; bağlantı yoksa sayfada "çevrimdışı" yazar, launcher'ın geri kalanı etkilenmez.

## 12. MehburMC hesabı

Launcher açılınca MehburMC hesabınla giriş yapman istenir.

- **Hesap oluştur:** e-posta ve şifre (en az 8 karakter) yaz. E-postana gelen **6 haneli kodu**
  launcher'a gir. Kod gelmezse spam klasörüne bak veya **Kodu tekrar gönder**.
- Launcher'ın eski bir sürümünde arkadaşları ya da hesap adlarını kullandıysan, hesap oluşturmak
  bu bilgisayardaki kimliğini hesaba **dönüştürür**: arkadaşların, ayrılmış adların ve
  paylaşımların korunur. (Bunun yerine başka bir hesaba giriş yaparsan o eski kimlik silinir.)
- **Google ile giriş yap:** giriş ekranındaki düğme tarayıcıda Google'ı açar; hesabını seçince
  launcher'a dön, giriş kendiliğinden tamamlanır. Bu Google e-postasıyla daha önce hesap açtıysan
  aynı hesaba girersin. (5 dakika içinde tamamlanmazsa ya da **İptal**'e basarsan yeniden dene.)
- **Şifremi unuttum:** e-postanı yaz, gelen kodu ve yeni şifreni gir.
- **Çevrim dışı hesap** (yalnız **Hesap oluştur** ekranında): e-posta ve şifre gerekmez, hesap yalnızca
  bu bilgisayarda durur. Ama oyun **Demo modunda** açılır (Çok Oyunculu yok, sunuculara girilemez,
  dünya 5 oyun günüyle sınırlı); Mod Tarayıcısı, Sunucular, Arkadaşlar, SkinMC, topluluk skinleri ve
  skin paylaşma kapalıdır; MehburMC Kütüphanesine mod yüklenemez, admin olunamaz, özel skin alınamaz.
  Ayarlar'ın en altındaki neon kırmızı **Çıkış yap** ile çıkıp istediğin zaman e-postayla kayıt olabilirsin.
- Bir kez giriş yaptıktan sonra internet yokken de oynayabilirsin.
- Ayarlar'ın en altındaki neon kırmızı **Çıkış yap** bu bilgisayardan çıkış yapar; profillerin, modların ve
  dünyaların silinmez.
- Kurallara uymayan hesaplar yöneticiler tarafından süreli ya da kalıcı **yasaklanabilir**;
  yasaklıyken launcher kullanılamaz, ekranda bitiş tarihi ve sebep yazar.

**Yöneticiler** kenar çubuğunda **Admin** sayfasını görür: Kütüphane'ye yüklenen mod ve paketlerin onayı
(dosya önce yöneticinin launcher'ında yeniden taranır), bildirilen içerik, banlar ve yönetici atama. Kurucu **Topluluk** sekmesinden paylaşılan skin/pelerinleri kaldırır
(ve geri getirir), ayrıca **Özel dokular** sekmesinden özel
MehburMC skin/pelerinlerini yükler ve seçtiği kişilere gönderir.

**Oyuncular** sekmesi (her iki rütbe) e-postayla kayıtlı bütün hesapları listeler: çevrimiçi olanlar
üstte, oyun içi isimler, arkadaş kodu, katılma ve son görülme tarihi. İsim veya arkadaş koduyla
süzülür, 100'er 100'er **Daha fazla yükle** ile devam eder; satırdan doğrudan banlanır, ban kaldırılır.
Kurucu ayrıca yönetici atar/çıkarır ve **Özel skin gönder** ile açılan pencerede her özel skin/pelerin
için şalterle verir veya geri alır. E-posta adresleri gösterilmez.

Sorun mu yaşıyorsun? → [Sorun giderme](SORUN_GIDERME.md)
