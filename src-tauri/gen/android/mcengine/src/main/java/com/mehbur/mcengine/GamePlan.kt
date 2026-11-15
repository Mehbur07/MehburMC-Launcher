package com.mehbur.mcengine

import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Launcher'ın (Rust, `launch/android.rs`) yazdığı başlatma planı (K104).
 * Alan adları Rust'taki `AndroidLaunch` ile birebir aynıdır.
 */
data class GamePlan(
    val versionId: String,
    val javaHome: String,
    val javaMajor: Int,
    val gameDir: String,
    val nativesDir: String,
    val classpath: List<String>,
    val jvmArgs: List<String>,
    val mainClass: String,
    val gameArgs: List<String>,
    val memoryMb: Int,
    val lwjgl: String,
    val lwjglx: Boolean,
    val renderer: String,
    val modernInput: Boolean,
    val logConfigId: String?,
    val logConfigPath: String?,
) {
    companion object {
        fun read(file: File): GamePlan = parse(JSONObject(file.readText()))

        fun parse(o: JSONObject): GamePlan {
            val log = o.optJSONObject("logConfig")
            return GamePlan(
                versionId = o.getString("versionId"),
                javaHome = o.getString("javaHome"),
                javaMajor = o.getInt("javaMajor"),
                gameDir = o.getString("gameDir"),
                nativesDir = o.getString("nativesDir"),
                classpath = o.getJSONArray("classpath").strings(),
                jvmArgs = o.getJSONArray("jvmArgs").strings(),
                mainClass = o.getString("mainClass"),
                gameArgs = o.getJSONArray("gameArgs").strings(),
                memoryMb = o.getInt("memoryMb"),
                lwjgl = o.getString("lwjgl"),
                lwjglx = o.getBoolean("lwjglx"),
                renderer = o.getString("renderer"),
                modernInput = o.getBoolean("modernInput"),
                logConfigId = log?.getString("id"),
                logConfigPath = log?.getString("path"),
            )
        }

        private fun JSONArray.strings(): List<String> = List(length()) { getString(it) }
    }
}
