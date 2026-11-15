package com.mehbur.mclauncher

import android.app.Activity
import android.content.Intent
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import com.mehbur.mcengine.GameActivity

@InvokeArg
class LaunchArgs {
    lateinit var planPath: String
}

/**
 * Rust'tan (src-tauri/src/game_engine.rs) gelen "oyunu başlat" isteği (K104):
 * oyun ekranını ayrı süreçte açar. Oyun açıksa aynı ekrana dönülür.
 */
@TauriPlugin
class GamePlugin(private val activity: Activity) : Plugin(activity) {
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
}
