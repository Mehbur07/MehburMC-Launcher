/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): yalnız native kodun çağırdığı
 * kısımlar tutuldu, Amethyst arayüzüne bağımlılıklar çıkarıldı.
 */
package net.kdt.pojavlaunch;

import androidx.annotation.Keep;

import java.util.concurrent.CopyOnWriteArrayList;

/** Oyun çıktısını (stdout/stderr) bir dosyaya yazan native günlükçü. */
@Keep
public class Logger {
    private static final CopyOnWriteArrayList<eventLogListener> logListeners = new CopyOnWriteArrayList<>();

    /** Metni günlüğe ekler. */
    public static native void appendToLog(String text);

    /** Günlük dosyasını sıfırlar ve süreç çıktısını ona yönlendirir. */
    public static native void begin(String logFilePath);

    public static void addLogListener(eventLogListener listener) {
        boolean wasEmpty = logListeners.isEmpty();
        logListeners.add(listener);
        if (wasEmpty) setLogListener(Logger::onEventLogged);
    }

    public static void removeLogListener(eventLogListener listener) {
        logListeners.remove(listener);
        if (logListeners.isEmpty()) setLogListener(null);
    }

    private static void onEventLogged(String text) {
        for (eventLogListener l : logListeners) l.onEventLogged(text);
    }

    @Keep
    public interface eventLogListener {
        void onEventLogged(String text);
    }

    private static native void setLogListener(eventLogListener logListener);
}
