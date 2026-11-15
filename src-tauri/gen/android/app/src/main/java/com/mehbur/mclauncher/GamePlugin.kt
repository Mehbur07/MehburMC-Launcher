package com.mehbur.mclauncher

import android.app.Activity
import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Process
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.mehbur.mcengine.GameActivity

@InvokeArg
class LaunchArgs {
    lateinit var planPath: String
}

@InvokeArg
class StatusArgs {
    /** Oyunun başlatıldığı an (Unix ms); daha eski çıkış kayıtları sayılmaz. */
    var since: Long = 0
}

/**
 * Rust'tan (src-tauri/src/game_engine.rs) gelen istekler (K104, A3):
 * - `launch`: oyun ekranını ayrı süreçte (`:game`) açar; oyun açıksa aynı ekrana dönülür.
 * - `status`: oyun süreci açık mı, kapandıysa sistem bunu çökme olarak mı kaydetti.
 * - `stop`: oyun sürecini kapatır (launcher'daki Durdur düğmesi).
 */
@TauriPlugin
class GamePlugin(private val activity: Activity) : Plugin(activity) {
    private val am get() = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager

    @Command
    fun launch(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(LaunchArgs::class.java)
            val intent = Intent(activity, GameActivity::class.java)
                .putExtra(GameActivity.EXTRA_PLAN, args.planPath)
                // Oyun kendi görevinde açılır (son uygulamalarda ayrı görünür).
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }

    @Command
    fun status(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StatusArgs::class.java)
            val running = gamePid() != null
            val result = JSObject()
            result.put("running", running)
            result.put("crashed", !running && crashedSince(args.since))
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        // Aynı uygulamanın süreci olduğu için öldürme izni vardır.
        gamePid()?.let { Process.killProcess(it) }
        invoke.resolve()
    }

    private fun gamePid(): Int? {
        val name = activity.packageName + GAME_PROCESS
        return am.runningAppProcesses?.firstOrNull { it.processName == name }?.pid
    }

    /** Android 11+ son kapanışın nedenini saklar; eski sürümlerde bilinmez. */
    private fun crashedSince(since: Long): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return false
        val name = activity.packageName + GAME_PROCESS
        return am.getHistoricalProcessExitReasons(activity.packageName, 0, 10).any {
            it.processName == name && it.timestamp >= since && it.reason in CRASH_REASONS
        }
    }

    private companion object {
        const val GAME_PROCESS = ":game"
        val CRASH_REASONS = setOf(
            ApplicationExitInfo.REASON_CRASH,
            ApplicationExitInfo.REASON_CRASH_NATIVE,
            ApplicationExitInfo.REASON_ANR,
            ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
            ApplicationExitInfo.REASON_LOW_MEMORY,
        )
    }
}
