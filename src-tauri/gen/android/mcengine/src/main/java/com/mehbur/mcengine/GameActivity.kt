package com.mehbur.mcengine

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import android.widget.TextView
import android.widget.Toast
import net.kdt.pojavlaunch.ExitActivity
import net.kdt.pojavlaunch.Logger
import net.kdt.pojavlaunch.MainActivity
import net.kdt.pojavlaunch.utils.JREUtils
import org.lwjgl.glfw.CallbackBridge
import java.io.File
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.min
import kotlin.system.exitProcess

/**
 * Oyun ekranı (K104). Ayrı süreçte (`:game`) açılır, planı okur, çizim
 * yüzeyini (SurfaceView) oyuna bağlar ve Java'yı başlatır.
 *
 * Dokunma (A2):
 * - Menülerde parmak fare imlecidir: dokun = sol tık, sürükle = sürükle,
 *   iki parmakla yukarı/aşağı = kaydırma.
 * - Dünyada sürükle = etrafa bak, kısa dokun = sağ tık (koy/kullan),
 *   basılı tut = sol tık basılı (kır/vur).
 * - Geri tuşu = ESC; sağ üstteki düğme ekran klavyesini açar.
 */
class GameActivity : Activity(), CallbackBridge.GrabListener {
    private val ui = Handler(Looper.getMainLooper())
    private lateinit var plan: GamePlan
    private lateinit var surface: SurfaceView
    private lateinit var status: TextView
    private lateinit var keyboard: KeyboardSink

    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        hideSystemBars()

        val planPath = intent.getStringExtra(EXTRA_PLAN)
        plan = try {
            GamePlan.read(File(requireNotNull(planPath) { "no plan" }))
        } catch (e: Throwable) {
            fail(e)
            return
        }

        // CallbackBridge ilk kez arayüz iş parçacığında yüklenmeli (pojavexec).
        CallbackBridge.nativeSetUseInputStackQueue(plan.modernInput)
        MainActivity.init(this)
        val logFile = File(plan.gameDir, "logs/android-latest.log")
        try {
            // Native günlükçü dosyayı oluşturmaz, yalnız açar.
            logFile.parentFile?.mkdirs()
            logFile.createNewFile()
            Logger.begin(logFile.absolutePath)
        } catch (e: Throwable) {
            fail(e)
            return
        }
        Logger.appendToLog("MehburMC engine: ${plan.versionId} on ${Build.MANUFACTURER} ${Build.MODEL} (Android ${Build.VERSION.RELEASE})")

        val root = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        surface = SurfaceView(this)
        root.addView(surface, FrameLayout.LayoutParams(-1, -1))
        keyboard = KeyboardSink(this)
        root.addView(keyboard, FrameLayout.LayoutParams(1, 1))
        status = TextView(this).apply {
            setText(R.string.mcengine_preparing)
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
        }
        root.addView(status, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
        root.addView(keyboardButton(), FrameLayout.LayoutParams(dp(44), dp(44), Gravity.TOP or Gravity.END).apply {
            setMargins(0, dp(8), dp(8), 0)
        })
        setContentView(root)

        surface.holder.addCallback(SurfaceCallbacks())
        surface.setOnTouchListener(Touch())
        CallbackBridge.addGrabListener(this)
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun keyboardButton() = TextView(this).apply {
        text = "⌨"
        contentDescription = getString(R.string.mcengine_keyboard)
        gravity = Gravity.CENTER
        setTextColor(Color.WHITE)
        setTextSize(TypedValue.COMPLEX_UNIT_SP, 20f)
        alpha = 0.6f
        background = GradientDrawable().apply {
            cornerRadius = dp(10).toFloat()
            setColor(Color.argb(140, 0, 0, 0))
        }
        setOnClickListener { toggleKeyboard() }
    }

    private fun toggleKeyboard() {
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        if (keyboard.hasFocus() && imm.isActive(keyboard)) {
            imm.hideSoftInputFromWindow(keyboard.windowToken, 0)
            keyboard.clearFocus()
        } else {
            keyboard.requestFocus()
            imm.showSoftInput(keyboard, InputMethodManager.SHOW_IMPLICIT)
        }
    }

    @Suppress("DEPRECATION")
    private fun hideSystemBars() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            window.setDecorFitsSystemWindows(false)
            window.insetsController?.let {
                it.hide(WindowInsets.Type.systemBars())
                it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            window.decorView.systemUiVisibility = (View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                or View.SYSTEM_UI_FLAG_FULLSCREEN
                or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN)
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideSystemBars()
        if (started) CallbackBridge.nativeSetWindowAttrib(GLFW_FOCUSED, if (hasFocus) 1 else 0)
    }

    override fun onResume() {
        super.onResume()
        if (started) {
            CallbackBridge.nativeSetWindowAttrib(GLFW_FOCUSED, 1)
            CallbackBridge.nativeSetWindowAttrib(GLFW_HOVERED, 1)
        }
    }

    override fun onPause() {
        if (started) {
            // Dünyadaysa oyunu duraklat (menü açılır).
            if (CallbackBridge.isGrabbing()) CallbackBridge.sendKeyPress(Keys.GLFW_KEY_ESCAPE)
            CallbackBridge.nativeSetWindowAttrib(GLFW_FOCUSED, 0)
            CallbackBridge.nativeSetWindowAttrib(GLFW_HOVERED, 0)
        }
        super.onPause()
    }

    override fun onStart() {
        super.onStart()
        if (started) CallbackBridge.nativeSetWindowAttrib(GLFW_VISIBLE, 1)
    }

    override fun onStop() {
        if (started) CallbackBridge.nativeSetWindowAttrib(GLFW_VISIBLE, 0)
        super.onStop()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // Oyun zaten açık: launcher'dan yeniden OYNA'ya basıldı, aynı oyuna dönülür.
    }

    override fun onDestroy() {
        CallbackBridge.removeGrabListener(this)
        super.onDestroy()
    }

    /** Oyun ekranının çizim çözünürlüğü: büyük ekranlarda kısa kenar sınırlanır. */
    private fun renderSize(viewW: Int, viewH: Int): Pair<Int, Int> {
        val powerful = GameLauncher.memoryCap(this) >= 1536 && Runtime.getRuntime().availableProcessors() > 4
        val target = if (powerful) 1080 else 720
        val shortSide = min(viewW, viewH)
        val scale = if (shortSide <= target) 1f else target.toFloat() / shortSide
        fun even(v: Float) = ceil(v).toInt().let { if (it % 2 != 0) it - 1 else it }.coerceAtLeast(2)
        return even(viewW * scale) to even(viewH * scale)
    }

    private inner class SurfaceCallbacks : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (started) {
                // Uygulamaya geri dönüldü: yeni yüzeyi oyuna bağla.
                JREUtils.setupBridgeWindow(holder.surface)
                refreshSize(holder)
                return
            }
            started = true
            refreshSize(holder)
            JREUtils.setupBridgeWindow(holder.surface)
            startGame()
        }

        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            refreshSize(holder)
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {}
    }

    private fun refreshSize(holder: SurfaceHolder) {
        // Görünüm henüz yerleşmediyse yüzeyin kendi boyutu kullanılır.
        val vw = surface.width.takeIf { it > 1 } ?: holder.surfaceFrame.width()
        val vh = surface.height.takeIf { it > 1 } ?: holder.surfaceFrame.height()
        if (vw < 2 || vh < 2) return
        val (w, h) = renderSize(vw, vh)
        if (w == CallbackBridge.windowWidth && h == CallbackBridge.windowHeight) return
        CallbackBridge.physicalWidth = vw
        CallbackBridge.physicalHeight = vh
        CallbackBridge.windowWidth = w
        CallbackBridge.windowHeight = h
        CallbackBridge.dpiScale = w.toFloat() / vw
        holder.setFixedSize(w, h)
        CallbackBridge.sendUpdateWindowSize(w, h)
    }

    private fun startGame() {
        val listener = object : Logger.eventLogListener {
            override fun onEventLogged(text: String) {
                if (text.contains("LWJGL") || text.contains("Backend library")) {
                    ui.post { status.visibility = View.GONE }
                    Logger.removeLogListener(this)
                }
            }
        }
        Logger.addLogListener(listener)
        ui.postDelayed({ status.visibility = View.GONE }, 30_000)

        Thread({
            val code = try {
                EngineFiles(this).also { it.ensure() }.let { files ->
                    GameLauncher.launch(this, plan, files, CallbackBridge.windowWidth, CallbackBridge.windowHeight)
                }
            } catch (e: Throwable) {
                Log.e(TAG, "engine failed", e)
                Logger.appendToLog("Engine failed: " + Log.getStackTraceString(e))
                ui.post { fail(e) }
                return@Thread
            }
            Logger.appendToLog("Java exit code: $code")
            ExitActivity.showExitMessage(this, code, false)
            exitProcess(code)
        }, "JVM Main thread").start()
    }

    private fun fail(e: Throwable) {
        Log.e(TAG, "cannot start the game", e)
        Toast.makeText(this, getString(R.string.mcengine_failed, e.message ?: e.javaClass.simpleName), Toast.LENGTH_LONG).show()
        finish()
        // Yarım başlamış bir motor süreçte kalmasın.
        ui.postDelayed({ android.os.Process.killProcess(android.os.Process.myPid()) }, 3_000)
    }

    // --- Girdi ---

    override fun onGrabState(isGrabbing: Boolean) {
        touch?.reset()
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        when (event.keyCode) {
            KeyEvent.KEYCODE_VOLUME_UP, KeyEvent.KEYCODE_VOLUME_DOWN, KeyEvent.KEYCODE_VOLUME_MUTE ->
                return super.dispatchKeyEvent(event)
        }
        if (!started) return super.dispatchKeyEvent(event)
        if (event.action == KeyEvent.ACTION_MULTIPLE && event.keyCode == KeyEvent.KEYCODE_UNKNOWN) {
            event.characters?.forEach { CallbackBridge.sendChar(it, CallbackBridge.getCurrentMods()) }
            return true
        }
        val glfw = Keys.glfw(event.keyCode)
        val char = event.unicodeChar
        if (glfw == 0 && char == 0) return super.dispatchKeyEvent(event)
        val down = event.action == KeyEvent.ACTION_DOWN
        when (event.keyCode) {
            KeyEvent.KEYCODE_SHIFT_LEFT, KeyEvent.KEYCODE_SHIFT_RIGHT -> CallbackBridge.holdingShift = down
            KeyEvent.KEYCODE_CTRL_LEFT, KeyEvent.KEYCODE_CTRL_RIGHT -> CallbackBridge.holdingCtrl = down
            KeyEvent.KEYCODE_ALT_LEFT, KeyEvent.KEYCODE_ALT_RIGHT -> CallbackBridge.holdingAlt = down
        }
        if (down && event.repeatCount > 0 && glfw == Keys.GLFW_KEY_ESCAPE) return true
        CallbackBridge.sendKeycode(glfw, if (down) char.toChar() else '\u0000', event.scanCode, CallbackBridge.getCurrentMods(), down)
        return true
    }

    private var touch: Touch? = null

    /** Dokunmaları fare olaylarına çevirir (ayrıntı sınıf açıklamasında). */
    private inner class Touch : View.OnTouchListener {
        private val slop = dp(10)
        private var pointer = -1
        private var downX = 0f
        private var downY = 0f
        private var lastX = 0f
        private var lastY = 0f
        private var downAt = 0L
        private var moved = false
        private var holdingLeft = false
        private var guiPressed = false
        private var scrolling = false
        private var scrollY = 0f
        private val longPress = Runnable {
            if (!moved && CallbackBridge.isGrabbing()) {
                holdingLeft = true
                CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_LEFT, true)
            }
        }

        init {
            touch = this
        }

        fun reset() {
            ui.removeCallbacks(longPress)
            if (holdingLeft || guiPressed) CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_LEFT, false)
            holdingLeft = false
            guiPressed = false
            scrolling = false
            pointer = -1
        }

        override fun onTouch(v: View, e: MotionEvent): Boolean {
            if (!started || v.width == 0 || v.height == 0) return true
            val sx = CallbackBridge.windowWidth / v.width.toFloat()
            val sy = CallbackBridge.windowHeight / v.height.toFloat()
            val grabbing = CallbackBridge.isGrabbing()
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    pointer = e.getPointerId(0)
                    downX = e.x; downY = e.y; lastX = e.x; lastY = e.y
                    downAt = e.eventTime
                    moved = false
                    if (grabbing) {
                        ui.postDelayed(longPress, LONG_PRESS_MS)
                    } else {
                        CallbackBridge.sendCursorPos(e.x * sx, e.y * sy)
                        CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_LEFT, true)
                        guiPressed = true
                    }
                }
                MotionEvent.ACTION_POINTER_DOWN -> if (!grabbing && e.pointerCount == 2) {
                    // İki parmak: tıklamayı bırak, kaydırmaya geç.
                    if (guiPressed) {
                        CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_LEFT, false)
                        guiPressed = false
                    }
                    scrolling = true
                    scrollY = (e.getY(0) + e.getY(1)) / 2
                }
                MotionEvent.ACTION_MOVE -> {
                    if (scrolling && e.pointerCount >= 2) {
                        val y = (e.getY(0) + e.getY(1)) / 2
                        val d = y - scrollY
                        if (abs(d) >= dp(SCROLL_STEP_DP)) {
                            CallbackBridge.sendScroll(0.0, if (d > 0) 1.0 else -1.0)
                            scrollY = y
                        }
                        return true
                    }
                    val i = e.findPointerIndex(pointer)
                    if (i < 0) return true
                    val x = e.getX(i)
                    val y = e.getY(i)
                    if (abs(x - downX) + abs(y - downY) > slop) moved = true
                    if (grabbing) {
                        if (moved && !holdingLeft) ui.removeCallbacks(longPress)
                        CallbackBridge.sendCursorPos(
                            CallbackBridge.mouseX + (x - lastX) * sx,
                            CallbackBridge.mouseY + (y - lastY) * sy,
                        )
                    } else if (!scrolling) {
                        CallbackBridge.sendCursorPos(x * sx, y * sy)
                    }
                    lastX = x
                    lastY = y
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    ui.removeCallbacks(longPress)
                    val tap = e.actionMasked == MotionEvent.ACTION_UP && !moved &&
                        e.eventTime - downAt < TAP_MS
                    when {
                        holdingLeft -> {
                            CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_LEFT, false)
                            holdingLeft = false
                        }
                        guiPressed -> {
                            if (!scrolling) CallbackBridge.sendCursorPos(e.x * sx, e.y * sy)
                            CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_LEFT, false)
                            guiPressed = false
                        }
                        grabbing && tap -> {
                            CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_RIGHT, true)
                            ui.postDelayed({
                                CallbackBridge.sendMouseButton(Keys.GLFW_MOUSE_BUTTON_RIGHT, false)
                            }, CLICK_MS)
                        }
                    }
                    scrolling = false
                    pointer = -1
                }
            }
            return true
        }
    }

    companion object {
        const val EXTRA_PLAN = "com.mehbur.mcengine.PLAN"
        private const val TAG = "MehburEngine"
        private const val GLFW_FOCUSED = 0x20001
        private const val GLFW_VISIBLE = 0x20004
        private const val GLFW_HOVERED = 0x2000B
        private const val LONG_PRESS_MS = 350L
        private const val TAP_MS = 250L
        private const val CLICK_MS = 50L
        private const val SCROLL_STEP_DP = 24

        /** Bu süreçte oyun bir kez başlar (Java yeniden başlatılamaz). */
        @Volatile
        private var started = false
    }
}
