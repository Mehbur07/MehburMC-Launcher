# Gizlilik Politikası — MehburMC Launcher

Son güncelleme: 20 Ekim 2026

MehburMC Launcher, Minecraft Java Edition'ı başlatmak için ücretsiz bir masaüstü uygulamasıdır.
Bu sayfa, launcher'ın hangi verileri nerede tuttuğunu anlatır. Reklam yoktur, veriler satılmaz
veya paylaşılmaz.

*English summary at the bottom.*

## 1. Yalnızca bilgisayarında kalan veriler

Oyun dosyaları, profiller (instance'lar), modlar, dünyalar, ayarlar, oyun hesaplarının adları,
skin kütüphanen ve günlük (log) dosyaları `%APPDATA%\MehburMC` altında, **yalnızca senin
bilgisayarında** durur ve bize gönderilmez.

**Çevrim dışı hesap** kullanıyorsan launcher sunucumuza hiçbir şey göndermez.

## 2. Sunucuda tutulan veriler

E-posta ya da Google hesabıyla giriş yaparsan aşağıdakiler [Supabase](https://supabase.com)
üzerindeki sunucumuzda saklanır:

| Veri | Neden |
|---|---|
| E-posta adresin (Google ile girişte Google hesabının e-postası, adı ve profil resmi bağlantısı) | Hesabını tanımak, giriş ve şifre sıfırlama |
| Şifren | Yalnızca Supabase'de, şifrelenmiş (hash) olarak; launcher saklamaz. Google ile girişte şifren bize hiç gelmez |
| Oyun hesabı adların | Aynı adın iki kişide olmaması için |
| Arkadaş kodun, görünen adın, profil fotoğrafın, son görülme zamanın | Arkadaşlar özelliği (açarsan) |
| Arkadaşlıkların ve mesajların | Arkadaşlar özelliği |
| Paylaştığın skin/pelerinler, yüklediğin modlar ve bunların güvenlik tarama sonuçları, bildirimler | Topluluk ve MehburMC Kütüphanesi |
| Yönetici rütbeleri, yasaklar ve yönetici işlem kayıtları | Kuralların uygulanması |

Yöneticiler, kuralları uygulamak için hesap adlarını, arkadaş kodlarını, katılma ve son görülme
tarihlerini görebilir; **e-posta adreslerini göremez**.

## 3. Üçüncü taraflar

- **Supabase** — hesap ve sunucu verilerinin barındırıldığı hizmet.
- **Google** — yalnızca "Google ile giriş yap"ı seçersen; giriş Google'ın kendi sayfasında yapılır.
- **Mojang/Microsoft, Modrinth, CurseForge, SkinMC, Adoptium** — oyun dosyaları, modlar, skinler
  ve Java bu sitelerden doğrudan indirilir. Bu istekler o sitelerin kendi gizlilik politikalarına tabidir.

## 4. Verilerini silmek

- **Arkadaşlar → Arkadaşları kapat ve verilerimi sil**: profilin, fotoğrafın, arkadaşlıkların,
  mesajların ve paylaşımların sunucudan silinir.
- Paylaştığın skinleri ve yüklediğin modları istediğin an geri çekebilirsin.
- Hesabının tamamen silinmesi için [GitHub Issues](https://github.com/Mehbur07/MehburMC-Launcher/issues)
  üzerinden istekte bulun (e-posta adresini oraya yazma; seninle iletişime geçilir).

## 5. Değişiklikler

Bu politika değişirse bu sayfa güncellenir ve tarih yenilenir.

---

## English summary

MehburMC Launcher is a free desktop launcher for Minecraft Java Edition. Game files, settings, logs
and skins stay on your computer. If you sign in with email or Google, your email (and, with Google,
your name and profile picture link), game account names, and — only for features you use — your
friend code, profile photo, last-seen time, friendships, messages, shared skins and uploaded mods are
stored on our [Supabase](https://supabase.com) server. Passwords are handled only by Supabase (hashed);
with Google sign-in the launcher never sees your password. Admins can see names and friend codes but
not emails. Offline accounts send nothing to our server. We do not sell or share data and show no ads.
To delete your data use "Friends → Turn off friends and delete my data", withdraw your shares, or
request full account deletion via [GitHub Issues](https://github.com/Mehbur07/MehburMC-Launcher/issues).
