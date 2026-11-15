/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): yalnız native kodun çağırdığı
 * kısımlar tutuldu, Amethyst arayüzüne bağımlılıklar çıkarıldı.
 */
package net.kdt.pojavlaunch;

import androidx.annotation.Keep;

/** AWT (Caciocavallo) panosu için native köprü. */
@Keep
public final class AWTInputBridge {
    private AWTInputBridge() {
    }

    public static native void nativeSendData(int type, int i1, int i2, int i3, int i4);

    public static native void nativeClipboardReceived(String data, String mimeTypeSub);

    static {
        System.loadLibrary("pojavexec_awt");
    }
}
