/*
 * Amethyst-Android (AngelAuraMC, 1.1.7, commit 56194e3) kaynağından uyarlanmıştır.
 * Lisans: GNU LGPL-3.0 — https://github.com/AngelAuraMC/Amethyst-Android
 * Değişiklikler MehburMC Launcher içindir (K104): SDL, oyun kolu ve Amethyst
 * arayüzü bağları çıkarıldı; @CriticalNative kullanılmaz (native taraf bunu
 * kendisi sınar ve normal JNI yöntemlerini kaydeder).
 */
package org.lwjgl.glfw;

import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.os.Handler;
import android.os.Looper;
import android.util.DisplayMetrics;
import android.util.Log;

import androidx.annotation.Keep;
import androidx.annotation.Nullable;

import net.kdt.pojavlaunch.MainActivity;

import java.nio.ByteBuffer;
import java.util.ArrayList;

/** Android → oyun (GLFW) girdi köprüsü; oyun tarafı da buradaki yöntemleri çağırır. */
public class CallbackBridge {
    /** Oyunun fareyi yakalayıp yakalamadığı (dünyada: evet, menüde: hayır) değişince. */
    public interface GrabListener {
        void onGrabState(boolean isGrabbing);
    }

    private static final Handler UI = new Handler(Looper.getMainLooper());
    private static volatile boolean isGrabbing = false;
    private static final ArrayList<GrabListener> grabListeners = new ArrayList<>();

    public static final int CLIPBOARD_COPY = 2000;
    public static final int CLIPBOARD_PASTE = 2001;
    public static final int CLIPBOARD_OPEN = 2002;

    public static volatile int windowWidth, windowHeight;
    public static volatile int physicalWidth, physicalHeight;
    public static volatile float dpiScale = 1f;
    public static float mouseX, mouseY;
    public static volatile boolean holdingAlt, holdingCapslock, holdingCtrl, holdingNumlock, holdingShift;

    public static final int GLFW_MOD_SHIFT = 0x1;
    public static final int GLFW_MOD_CONTROL = 0x2;
    public static final int GLFW_MOD_ALT = 0x4;
    public static final int GLFW_MOD_CAPS_LOCK = 0x10;
    public static final int GLFW_MOD_NUM_LOCK = 0x20;

    public static final ByteBuffer sGamepadButtonBuffer;
    public static final ByteBuffer sGamepadAxisBuffer;

    public static void sendCursorPos(float x, float y) {
        mouseX = x;
        mouseY = y;
        nativeSendCursorPos(mouseX, mouseY);
    }

    /** Tuş kodu (GLFW) ve/veya karakter gönderir. */
    public static void sendKeycode(int keycode, char keychar, int scancode, int modifiers, boolean isDown) {
        if (keycode != 0) nativeSendKey(keycode, scancode, isDown ? 1 : 0, modifiers);
        if (isDown && keychar != 0 && !Character.isISOControl(keychar)) {
            nativeSendCharMods(keychar, modifiers);
            nativeSendChar(keychar);
        }
    }

    /** Yazılan bir karakter (klavye uygulamasından). */
    public static void sendChar(char keychar, int modifiers) {
        nativeSendCharMods(keychar, modifiers);
        nativeSendChar(keychar);
    }

    public static void sendKeyPress(int keyCode) {
        sendKeycode(keyCode, '\u0000', 0, getCurrentMods(), true);
        sendKeycode(keyCode, '\u0000', 0, getCurrentMods(), false);
    }

    public static void sendMouseButton(int button, boolean isDown) {
        nativeSendMouseButton(button, isDown ? 1 : 0, getCurrentMods());
    }

    public static void sendScroll(double xoffset, double yoffset) {
        nativeSendScroll(xoffset, yoffset);
    }

    public static void sendUpdateWindowSize(int w, int h) {
        nativeSendScreenSize(w, h);
    }

    public static boolean isGrabbing() {
        return isGrabbing;
    }

    // Oyun (JRE) tarafından çağrılır.
    @Keep
    public static @Nullable String accessAndroidClipboard(int type, String copy) {
        ClipboardManager cm = MainActivity.GLOBAL_CLIPBOARD;
        switch (type) {
            case CLIPBOARD_COPY:
                if (cm != null) cm.setPrimaryClip(ClipData.newPlainText("Copy", copy));
                return null;
            case CLIPBOARD_PASTE:
                if (cm != null && cm.hasPrimaryClip() && cm.getPrimaryClipDescription() != null
                        && cm.getPrimaryClipDescription().hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                        && cm.getPrimaryClip() != null) {
                    CharSequence text = cm.getPrimaryClip().getItemAt(0).getText();
                    return text == null ? "" : text.toString();
                }
                return "";
            case CLIPBOARD_OPEN:
                MainActivity.openLink(copy);
                return null;
            default:
                return null;
        }
    }

    // Oyun tarafından çağrılır (SDL bildirimleri); SDL desteği yok.
    @Keep
    public static boolean notifyLauncher(int type, int... action) {
        Log.i("CallbackBridge", "notifyLauncher(" + type + ") ignored");
        return false;
    }

    public static int getCurrentMods() {
        int mods = 0;
        if (holdingAlt) mods |= GLFW_MOD_ALT;
        if (holdingCapslock) mods |= GLFW_MOD_CAPS_LOCK;
        if (holdingCtrl) mods |= GLFW_MOD_CONTROL;
        if (holdingNumlock) mods |= GLFW_MOD_NUM_LOCK;
        if (holdingShift) mods |= GLFW_MOD_SHIFT;
        return mods;
    }

    // Oyun tarafından çağrılır.
    @Keep
    private static void onDirectInputEnable() {
        Log.i("CallbackBridge", "onDirectInputEnable()");
    }

    // Oyun tarafından çağrılır.
    @Keep
    private static void onGrabStateChanged(final boolean grabbing) {
        isGrabbing = grabbing;
        UI.postDelayed(() -> {
            if (isGrabbing != grabbing) return; // arada yine değişti
            synchronized (grabListeners) {
                for (GrabListener g : grabListeners) g.onGrabState(grabbing);
            }
        }, 16);
    }

    public static void addGrabListener(GrabListener listener) {
        synchronized (grabListeners) {
            listener.onGrabState(isGrabbing);
            grabListeners.add(listener);
        }
    }

    public static void removeGrabListener(GrabListener listener) {
        synchronized (grabListeners) {
            grabListeners.remove(listener);
        }
    }

    // imgui-java'nın glfwGetWindowContentScale'i için.
    @Keep
    private static float getAndroidDPI() {
        DisplayMetrics metrics = new DisplayMetrics();
        metrics.setToDefaults();
        return metrics.density * dpiScale;
    }

    // RegisterNatives ile bağlanır (input_bridge_v3.c).
    @Keep public static native void nativeSetUseInputStackQueue(boolean useInputStackQueue);
    @Keep private static native boolean nativeSendChar(char codepoint);
    @Keep private static native boolean nativeSendCharMods(char codepoint, int mods);
    @Keep private static native void nativeSendKey(int key, int scancode, int action, int mods);
    @Keep private static native void nativeSendCursorPos(float x, float y);
    @Keep private static native void nativeSendMouseButton(int button, int action, int mods);
    @Keep private static native void nativeSendScroll(double xoffset, double yoffset);
    @Keep private static native void nativeSendScreenSize(int width, int height);
    public static native void nativeSetWindowAttrib(int attrib, int value);
    private static native ByteBuffer nativeCreateGamepadButtonBuffer();
    private static native ByteBuffer nativeCreateGamepadAxisBuffer();

    static {
        System.loadLibrary("pojavexec");
        sGamepadButtonBuffer = nativeCreateGamepadButtonBuffer();
        sGamepadAxisBuffer = nativeCreateGamepadAxisBuffer();
    }
}
