# İlerleme Kaydı

### [2026-11-15 - Mehbur07]
- **Yapılanlar & Kararlar:**
  - Android sürümü üç fazda eklendi (`feature/android-apk` branch'i, PR ile main'e): A0 araçlar
    ve APK iskeleti (K102), A1 telefon arayüzü (K103), A2 oyunu telefonda başlatma (K104).
  - Oyun motoru olarak Amethyst-Android 1.1.7 seçildi; hazır dosyalar resmi APK'dan
    `scripts/android-engine.mjs` ile alınıyor (SHA-256 sabit, git'e girmiyor).
  - Android JRE'leri (8/17/21/25) ilk oyunda indiriliyor; eski JNA sürümleri motorun yerel
    kütüphanesiyle uyumlu olsun diye 5.14.0 ile değiştiriliyor.
  - Yayın imza anahtarı oluşturuldu (`%USERPROFILE%\.android-keys`, şifre yalnız `.env`'de).
- **Son Commit:** dfe4290 docs: record the Android game engine decision (K104)
- **Aktif Branch:** main
- **Sıradaki İlk İş (Next Action):** APK'yı gerçek bir telefonda denemek; sorun çıkarsa
  profilin `logs/android-latest.log` dosyasıyla düzeltmek. Sonra Faz A3 (oyun içi dokunmatik
  kontroller) — yalnız onay gelince.
- **Bilinen Sorunlar / Dikkat Edilmesi Gerekenler:**
  - Oyun motoru hiç gerçek cihazda çalıştırılmadı.
  - Motor kütüphanelerinin çoğu 4 KB sayfa hizalı; 16 KB sayfalı yeni cihazlarda yüklenmeyebilir.
  - 26.2+ Adreno dışı GPU'larda Kopper Zink gerekir (eklenmedi).
  - Yeni APK farklı anahtarla imzalı: eski test sürümü önce kaldırılmalı.
  - İmza anahtarı ve `.env` yedeklenmeli; kaybolursa uygulama güncellenemez.
