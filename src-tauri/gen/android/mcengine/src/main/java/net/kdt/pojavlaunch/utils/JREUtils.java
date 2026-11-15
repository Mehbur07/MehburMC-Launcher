/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): yalnız native kodun çağırdığı
 * kısımlar tutuldu, Amethyst arayüzüne bağımlılıklar çıkarıldı.
 */
package net.kdt.pojavlaunch.utils;

import android.content.Context;

/** libpojavexec'in Java tarafı: yalnız native yöntemler. Mantık GameLauncher'da. */
public final class JREUtils {
    private JREUtils() {
    }

    public static native int chdir(String path);

    public static native boolean dlopen(String libPath);

    public static native void setLdLibraryPath(String ldLibraryPath);

    public static native void setupBridgeWindow(Object surface);

    public static native void releaseBridgeWindow();

    public static native void initializeHooks();

    public static native void setupExitMethod(Context context);

    static {
        System.loadLibrary("exithook");
        System.loadLibrary("pojavexec");
        System.loadLibrary("pojavexec_awt");
    }
}
