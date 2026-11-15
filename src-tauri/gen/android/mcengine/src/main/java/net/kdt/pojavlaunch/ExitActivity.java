/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): yalnız native kodun çağırdığı
 * kısımlar tutuldu, Amethyst arayüzüne bağımlılıklar çıkarıldı.
 */
package net.kdt.pojavlaunch;

import android.content.Context;
import android.content.Intent;

import androidx.annotation.Keep;

import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;

/**
 * Java kapanınca native çıkış kancası bunu çağırır, ardından süreç biter.
 * Amethyst'te bir ekran açar; burada launcher öne getirilir ve çıkış kodu
 * iletilir (launcher, sıfır değilse kısa bir uyarı gösterir).
 */
@Keep
public final class ExitActivity {
    public static final String EXTRA_EXIT_CODE = "com.mehbur.mcengine.EXIT_CODE";
    public static final String EXTRA_SIGNAL = "com.mehbur.mcengine.EXIT_SIGNAL";

    /** Çıkış kodunun yazılacağı dosya (A3); launcher oyunun nasıl bittiğini buradan okur. */
    private static volatile String sExitFile;

    private ExitActivity() {
    }

    public static void setExitFile(String path) {
        sExitFile = path;
    }

    public static void showExitMessage(Context ctx, int code, boolean isSignal) {
        // Sinyalle ölüm (çökme) dosyaya yazılmaz: launcher onu sistemin
        // kayıtlarından (ApplicationExitInfo) öğrenir.
        String exitFile = sExitFile;
        if (exitFile != null && !isSignal) {
            try {
                File f = new File(exitFile);
                File dir = f.getParentFile();
                if (dir != null) dir.mkdirs();
                try (FileOutputStream out = new FileOutputStream(f)) {
                    out.write(Integer.toString(code).getBytes(StandardCharsets.UTF_8));
                }
            } catch (Throwable ignored) {
                // Yazılamazsa launcher oyunu "dışarıdan kapatıldı" sayar.
            }
        }
        try {
            Intent i = ctx.getPackageManager().getLaunchIntentForPackage(ctx.getPackageName());
            if (i == null) return;
            i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_REORDER_TO_FRONT);
            i.putExtra(EXTRA_EXIT_CODE, code);
            i.putExtra(EXTRA_SIGNAL, isSignal);
            ctx.startActivity(i);
        } catch (Throwable ignored) {
            // Süreç zaten kapanıyor; launcher'ı öne getirememek oyunu etkilemez.
        }
    }
}
