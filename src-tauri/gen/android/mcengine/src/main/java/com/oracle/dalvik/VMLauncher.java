/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): yalnız native kodun çağırdığı
 * kısımlar tutuldu, Amethyst arayüzüne bağımlılıklar çıkarıldı.
 */
package com.oracle.dalvik;

/** JVM'i bu süreçte başlatır (libpojavexec: JLI_Launch). */
public final class VMLauncher {
    private VMLauncher() {
    }

    public static native int launchJVM(String[] args);
}
