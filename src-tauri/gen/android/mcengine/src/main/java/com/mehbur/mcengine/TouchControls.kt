package com.mehbur.mcengine

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.widget.FrameLayout
import android.widget.TextView
import org.lwjgl.glfw.CallbackBridge
import java.io.File
import kotlin.math.hypot
import kotlin.math.max

/**
 * Oyun içi dokunmatik kontroller (A3, K105). Yalnız dünyadayken (fare oyuna
 * kilitliyken) görünür; menülerde gizlenir ve basılı tuşlar bırakılır.
 *
 * - Sol alt: yürüme çubuğu (W/A/S/D). İleri sonuna kadar itilince koşulur (Ctrl).
 * - Sağ alt: zıpla (basılı tut), eğil (dokun aç/kapa), envanter.
 * - Sol üst: duraklat, sohbet, kamera açısı (F5), eşya at (Q).
 * - Alt orta: oyunun eşya çubuğu; yuvaya dokununca o yuva seçilir (1-9).
 *
 * Boş alanlar dokunmayı tutmaz: alttaki oyun yüzeyine (etrafa bakma, koy/kır)
 * geçer. Her düğme kendi parmağını alır, böylece yürürken bakılabilir.
 */
@SuppressLint("ViewConstructor")
class TouchControls(
    context: Context,
    private val controls: GamePlan.Controls,
    private val gameDir: String,
) : FrameLayout(context) {
    private val scale = controls.sizePercent / 100f
    private val held = mutableSetOf<Int>()
    private val toggles = mutableListOf<KeyButton>()
    private val stick = Joystick()
    private val hotbar = HotbarZone()

    init {
        isMotionEventSplittingEnabled = true
        val alpha = controls.opacityPercent / 100f

        addView(stick, LayoutParams(size(140), size(140), Gravity.BOTTOM or Gravity.START).apply {
            setMargins(dp(28), 0, 0, dp(28))
        })
        stick.alpha = alpha

        val jump = KeyButton("⤒", R.string.mcengine_jump, KEY_SPACE, Mode.HOLD)
        val sneak = KeyButton("⇩", R.string.mcengine_sneak, KEY_LEFT_SHIFT, Mode.TOGGLE)
        val inventory = KeyButton("▦", R.string.mcengine_inventory, KEY_E, Mode.TAP)
        add(jump, 76, Gravity.BOTTOM or Gravity.END, end = 28, bottom = 36)
        add(sneak, 60, Gravity.BOTTOM or Gravity.END, end = 28 + 76 + 16, bottom = 28)
        add(inventory, 52, Gravity.BOTTOM or Gravity.END, end = 40, bottom = 36 + 76 + 16)

        val top = listOf(
            KeyButton("❚❚", R.string.mcengine_pause, KEY_ESCAPE, Mode.TAP),
            KeyButton("💬", R.string.mcengine_chat, KEY_T, Mode.TAP),
            KeyButton("👁", R.string.mcengine_perspective, KEY_F5, Mode.TAP),
            KeyButton("⇣", R.string.mcengine_drop, KEY_Q, Mode.TAP),
        )
        top.forEachIndexed { i, b -> add(b, 44, Gravity.TOP or Gravity.START, start = 8 + i * (44 + 8), top = 8) }
        for (b in listOf(jump, sneak, inventory) + top) b.alpha = alpha

        addView(hotbar, LayoutParams(0, 0))
        visibility = GONE
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()
    private fun size(v: Int) = dp((v * scale).toInt())

    private fun add(
        b: KeyButton, sizeDp: Int, gravity: Int,
        start: Int = 0, top: Int = 0, end: Int = 0, bottom: Int = 0,
    ) {
        val s = size(sizeDp)
        addView(b, LayoutParams(s, s, gravity).apply {
            setMargins(size(start), size(top), size(end), size(bottom))
        })
    }

    /** Dünyaya girildi/çıkıldı. Çıkışta tüm basılı tuşlar bırakılır. */
    fun setInWorld(inWorld: Boolean) {
        if (!inWorld) releaseAll()
        visibility = if (inWorld) VISIBLE else GONE
        if (inWorld) hotbar.place()
    }

    fun releaseAll() {
        stick.reset()
        for (t in toggles) t.reset()
        for (k in held.toList()) key(k, false)
    }

    private fun key(code: Int, down: Boolean) {
        if (down && !held.add(code)) return
        if (!down && !held.remove(code)) return
        if (code == KEY_LEFT_SHIFT) CallbackBridge.holdingShift = down
        if (code == KEY_LEFT_CONTROL) CallbackBridge.holdingCtrl = down
        CallbackBridge.sendKeycode(code, '\u0000', 0, CallbackBridge.getCurrentMods(), down)
    }

    private fun tap(code: Int) {
        key(code, true)
        key(code, false)
    }

    private enum class Mode { HOLD, TOGGLE, TAP }

    /** Yuvarlak köşeli, simgeli düğme. */
    private inner class KeyButton(label: String, description: Int, private val code: Int, private val mode: Mode) :
        TextView(context) {
        private var on = false
        private val bg = GradientDrawable().apply {
            cornerRadius = dp(14).toFloat()
            setStroke(dp(1), Color.argb(120, 255, 255, 255))
        }

        init {
            text = label
            contentDescription = context.getString(description)
            gravity = Gravity.CENTER
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f * scale)
            background = bg
            paint()
            if (mode == Mode.TOGGLE) toggles += this
        }

        private fun paint(pressed: Boolean = false) {
            bg.setColor(
                when {
                    on -> Color.argb(170, 0, 200, 220)
                    pressed -> Color.argb(170, 255, 255, 255)
                    else -> Color.argb(110, 0, 0, 0)
                },
            )
        }

        fun reset() {
            on = false
            paint()
        }

        @SuppressLint("ClickableViewAccessibility")
        override fun onTouchEvent(e: MotionEvent): Boolean {
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    when (mode) {
                        Mode.HOLD -> key(code, true)
                        Mode.TOGGLE -> {
                            on = !on
                            key(code, on)
                        }
                        Mode.TAP -> tap(code)
                    }
                    paint(pressed = true)
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    if (mode == Mode.HOLD) key(code, false)
                    paint()
                }
            }
            return true
        }
    }

    /** Yürüme çubuğu: merkeze göre yön; kenara itince koşma. */
    private inner class Joystick : View(context) {
        private val base = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.argb(90, 0, 0, 0) }
        private val ring = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = dp(2).toFloat()
            color = Color.argb(150, 255, 255, 255)
        }
        private val knob = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.argb(190, 255, 255, 255) }
        private var nx = 0f
        private var ny = 0f
        private val keys = mutableSetOf<Int>()

        fun reset() {
            nx = 0f
            ny = 0f
            apply(emptySet())
            invalidate()
        }

        private fun apply(next: Set<Int>) {
            for (k in keys - next) key(k, false)
            for (k in next - keys) key(k, true)
            keys.clear()
            keys += next
        }

        @SuppressLint("ClickableViewAccessibility")
        override fun onTouchEvent(e: MotionEvent): Boolean {
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE -> {
                    val r = width / 2f
                    var dx = (e.x - r) / r
                    var dy = (e.y - r) / r
                    val mag = hypot(dx, dy)
                    if (mag > 1f) {
                        dx /= mag
                        dy /= mag
                    }
                    nx = dx
                    ny = dy
                    val next = mutableSetOf<Int>()
                    if (mag >= DEAD_ZONE) {
                        val ux = dx / max(mag, 1e-3f)
                        val uy = dy / max(mag, 1e-3f)
                        if (uy < -AXIS) next += KEY_W
                        if (uy > AXIS) next += KEY_S
                        if (ux < -AXIS) next += KEY_A
                        if (ux > AXIS) next += KEY_D
                        if (mag >= SPRINT_ZONE && uy < -SPRINT_AXIS) next += KEY_LEFT_CONTROL
                    }
                    apply(next)
                    invalidate()
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> reset()
            }
            return true
        }

        override fun onDraw(c: Canvas) {
            val r = width / 2f
            c.drawCircle(r, r, r - ring.strokeWidth, base)
            c.drawCircle(r, r, r - ring.strokeWidth, ring)
            val k = r * 0.4f
            c.drawCircle(r + nx * (r - k), r + ny * (r - k), k, knob)
        }
    }

    /**
     * Oyunun eşya çubuğunun üstüne konan görünmez alan. Yeri oyunun arayüz
     * ölçeğinden hesaplanır (options.txt `guiScale`), dünyaya her girişte
     * yeniden okunur (oyun içinde değiştirilmiş olabilir).
     */
    private inner class HotbarZone : View(context) {
        private var slot = -1

        fun place() {
            val w = CallbackBridge.windowWidth
            val h = CallbackBridge.windowHeight
            val dpi = CallbackBridge.dpiScale
            if (w < 2 || h < 2 || dpi <= 0f) return
            val (guiScale, unicode) = readOptions()
            val s = Hotbar.scale(w, h, guiScale, unicode)
            val lp = layoutParams as LayoutParams
            lp.width = (Hotbar.WIDTH * s / dpi).toInt()
            lp.height = (Hotbar.HEIGHT * s / dpi).toInt()
            lp.gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
            layoutParams = lp
        }

        private fun readOptions(): Pair<Int, Boolean> = try {
            val lines = File(gameDir, "options.txt").readLines()
            fun value(k: String) = lines.firstOrNull { it.startsWith("$k:") }?.substringAfter(':')?.trim()
            (value("guiScale")?.toIntOrNull() ?: 0) to (value("forceUnicodeFont") == "true")
        } catch (_: Throwable) {
            0 to false
        }

        @SuppressLint("ClickableViewAccessibility")
        override fun onTouchEvent(e: MotionEvent): Boolean {
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE -> {
                    val i = Hotbar.slotAt(e.x, width.toFloat())
                    if (i != slot) {
                        slot = i
                        tap(KEY_1 + i)
                    }
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> slot = -1
            }
            return true
        }
    }

    private companion object {
        const val DEAD_ZONE = 0.25f
        const val AXIS = 0.38f
        const val SPRINT_ZONE = 0.92f
        const val SPRINT_AXIS = 0.8f

        // GLFW tuş kodları.
        const val KEY_SPACE = 32
        const val KEY_1 = 49
        const val KEY_A = 65
        const val KEY_D = 68
        const val KEY_E = 69
        const val KEY_Q = 81
        const val KEY_S = 83
        const val KEY_T = 84
        const val KEY_W = 87
        const val KEY_ESCAPE = 256
        const val KEY_F5 = 294
        const val KEY_LEFT_SHIFT = 340
        const val KEY_LEFT_CONTROL = 341
    }
}

/** Oyunun eşya çubuğu ölçüleri (arayüz pikseli) ve ölçek hesabı. */
object Hotbar {
    const val WIDTH = 182
    const val HEIGHT = 22

    /**
     * Minecraft'ın arayüz ölçeği hesabı (`Window.calculateScale`): 0 = otomatik,
     * ekran 320x240 arayüz pikselinin altına düşmeyecek en büyük ölçek.
     */
    fun scale(w: Int, h: Int, guiScale: Int, forceUnicode: Boolean): Int {
        var i = 1
        while (i != guiScale && i < w && i < h && w / (i + 1) >= 320 && h / (i + 1) >= 240) i++
        if (forceUnicode && i % 2 != 0) i++
        return i
    }

    /** Alan içindeki x konumuna denk gelen yuva (0-8). */
    fun slotAt(x: Float, width: Float): Int {
        if (width <= 0f) return 0
        // Çubuğun iki yanında 1'er piksellik kenar var, her yuva 20 piksel.
        val gx = x / width * WIDTH
        return ((gx - 1f) / 20f).toInt().coerceIn(0, 8)
    }
}
