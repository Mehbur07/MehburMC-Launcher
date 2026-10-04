# Sorun Giderme

Launcher'daki her hata mesajının altında **Detayları kopyala** düğmesi vardır; teknik ayrıntı
ve hata kodu oradadır. Loglar: `%APPDATA%\MehburMC\game\mc\logs\launcher-*.log` (7 gün
saklanır; parolalar, token'lar ve API anahtarları maskelenir). Ayrıntılı log için
**Ayarlar → Gelişmiş → Hata ayıklama günlüğü** (yeniden başlatınca etkinleşir).

## Sık karşılaşılanlar

| Belirti / hata kodu | Neden | Çözüm |
|---|---|---|
| `net.unreachable`, `net.timeout` | İnternet yok, güvenlik duvarı ya da sunucu yanıt vermiyor | Bağlantıyı kontrol et. Daha önce kurulmuş sürümler çevrimdışı da başlar. |
| `version.notFound` | Sürüm hiç indirilmemiş ve internet yok | Bir kez internete bağlıyken başlat. |
| `download.hashMismatch` | İndirilen dosya bozuk veya değiştirilmiş | Tekrar dene; sürerse proxy/antivirüs dosyayı değiştiriyor olabilir. Launcher doğrulanmamış dosyayı **kullanmaz**. |
| `net.notAllowed` | İzin listesi dışında bir adres (güvenlik) | Normalde görülmez; modpack bilinmeyen bir sunucudan dosya istiyor olabilir. |
| `io.diskFull` | Disk dolu | Yer aç veya veri klasörünü başka diske taşı. |
| `io.permissionDenied`, `paths.notWritable` | Klasöre yazma izni yok (ör. kurumsal bilgisayar) | Portable kullan veya veri klasörünü yazılabilir bir yere taşı. |
| `java.unavailable` | Bu işletim sistemi/mimari için otomatik Java yok | Java'yı kendin kurup profil ayarlarından seç. |
| `instance.busy` | Profil zaten çalışıyor veya hazırlanıyor | Önce oyunu kapat / indirmenin bitmesini bekle. |
| `instance.invalid: javaPath` | Java yolu `java.exe`/`javaw.exe` değil | Profil ayarlarında **Gözat** ile `bin\java.exe` seç veya "Otomatik"e al. |
| `instance.invalid: jvmArgs …` | Komut çalıştıran JVM seçeneği | `-XX:OnError` / `-XX:OnOutOfMemoryError` güvenlik nedeniyle kabul edilmez. |
| `loader.addonUnavailable` | Mod/paket bu sürüm + mod yükleyici için yok | Farklı bir MC sürümü veya yükleyici seç. |
| `content.curseforgeKey` | CurseForge paketi için anahtar yok/geçersiz | console.curseforge.com'dan anahtar al, Ayarlar → Gelişmiş'e yapıştır. |
| `paths.moveFailed: not empty` | Taşıma hedefi boş değil | Boş bir klasör seç. |
| `skin.invalid` | Görsel PNG değil veya boyutu yanlış | Skin 64×64 / 64×32, pelerin 64×32 olmalı. |

## Oyun açılmıyor veya hemen kapanıyor

1. Çöküş penceresindeki teşhise bak (bellek, Java sürümü, eksik mod, sürücü…).
2. **Onar** ile dosyaları doğrula.
3. Son eklediğin modu kapatıp dene (Modlar sekmesi → anahtar).
4. Ekran kartı sürücünü güncelle; dizüstünde oyunun harici GPU'da çalıştığından emin ol.
5. NeoForge'da yükleme penceresi bazı sürücülerde çöker; launcher bunu algılayıp
   `earlyWindowControl=false` ile **bir kez otomatik** yeniden başlatır.

## Skin oyunda görünmüyor

- Profil Fabric/Quilt/Forge/NeoForge mi? Vanilla'da offline skin görünmez.
- Skin & Cape ekranında profil için "CustomSkinLoader kurulu" yazıyor mu?
- Skin hesaba **uygulandı** mı (kartta ✓)? Değişiklikten sonra oyunu yeniden başlat.
- CustomSkinLoader'ın kendi logu: `<profil>\CustomSkinLoader\CustomSkinLoader.log`.

## "Oyun açılınca kapat" seçiliyken konsol boş

Bu modda oyun çıktısı `<profil>\logs\launcher-output.log` dosyasına yazılır; launcher
kapandıktan sonra çıktıyı oradan okuyabilirsin.

## Güncelleme kanalına ulaşılamıyor

Proje deposu herkese açık olana kadar otomatik güncelleme kanalı erişilemez; bu bir hata
değildir. Yeni sürümü elle kurabilirsin.

## Hâlâ çözülmediyse

Detayları kopyala + ilgili launcher logunu (ve varsa crash raporunu) hata bildirimine ekle.
