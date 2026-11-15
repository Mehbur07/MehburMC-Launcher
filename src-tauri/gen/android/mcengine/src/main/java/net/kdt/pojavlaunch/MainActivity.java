/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): yalnız native kodun çağırdığı
 * kısımlar tutuldu, Amethyst arayüzüne bağımlılıklar çıkarıldı.
 */
package net.kdt.pojavlaunch;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;

import androidx.annotation.Keep;

/**
 * Amethyst'te oyun ekranıdır; native kod (AWT köprüsü) buradaki statik
 * yöntemleri adıyla çağırır. Burada yalnız o yöntemleri taşıyan bir sınıftır;
 * oyun ekranı com.mehbur.mcengine.GameActivity.
 */
@Keep
public final class MainActivity {
    @SuppressLint("StaticFieldLeak") // Uygulama bağlamı tutulur, ekran değil.
    private static volatile Context sContext;
    public static volatile ClipboardManager GLOBAL_CLIPBOARD;
    private static final Handler UI = new Handler(Looper.getMainLooper());

    private MainActivity() {
    }

    public static void init(Activity activity) {
        sContext = activity.getApplicationContext();
        GLOBAL_CLIPBOARD = (ClipboardManager) activity.getSystemService(Context.CLIPBOARD_SERVICE);
    }

    @Keep
    public static void openLink(String link) {
        Context ctx = sContext;
        if (ctx == null || link == null) return;
        UI.post(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(link));
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                ctx.startActivity(intent);
            } catch (Throwable ignored) {
                // Bağlantıyı açacak uygulama yoksa sessizce geç.
            }
        });
    }

    @Keep
    public static void querySystemClipboard() {
        UI.post(() -> {
            ClipboardManager cm = GLOBAL_CLIPBOARD;
            ClipData clip = cm == null ? null : cm.getPrimaryClip();
            CharSequence text = clip == null || clip.getItemCount() == 0 ? null : clip.getItemAt(0).getText();
            AWTInputBridge.nativeClipboardReceived(text == null ? null : text.toString(), text == null ? null : "plain");
        });
    }

    @Keep
    public static void putClipboardData(String data, String mimeType) {
        UI.post(() -> {
            ClipboardManager cm = GLOBAL_CLIPBOARD;
            if (cm == null) return;
            if ("text/plain".equals(mimeType)) cm.setPrimaryClip(ClipData.newPlainText("AWT Paste", data));
            else if ("text/html".equals(mimeType)) cm.setPrimaryClip(ClipData.newHtmlText("AWT Paste", data, data));
        });
    }
}
