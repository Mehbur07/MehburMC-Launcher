package com.mehbur.mcengine

import android.content.Context
import java.io.File

/**
 * APK içindeki motor varlıklarını (`assets/engine/components`: Android LWJGL,
 * Caciocavallo, yöntem ajanı, log4j yamaları) uygulama klasörüne açar. Sürüm
 * dosyası değişmediyse tekrar kopyalamaz.
 */
class EngineFiles(context: Context) {
    private val assets = context.assets
    val root = File(context.filesDir, "engine")
    val components = File(root, "components")
    val nativeLibDir: String = context.applicationInfo.nativeLibraryDir
    val cacheDir: File = File(context.cacheDir, "engine").apply { mkdirs() }

    fun lwjglJars(version: String) = File(components, "lwjgl3/$version")

    fun lwjglNatives(version: String) = File(components, "lwjgl-$version-natives/arm64-v8a")

    /** Gerekirse varlıkları açar; uzun sürebilir, arka planda çağrılmalı. */
    fun ensure() {
        val wanted = assets.open("$ASSET_ROOT/version").bufferedReader().use { it.readText().trim() }
        val stamp = File(components, ".version")
        if (stamp.isFile && stamp.readText().trim() == wanted) return
        components.deleteRecursively()
        copyDir("$ASSET_ROOT/components", components)
        stamp.writeText(wanted)
    }

    private fun copyDir(assetPath: String, dest: File) {
        val children = assets.list(assetPath).orEmpty()
        if (children.isEmpty()) {
            dest.parentFile?.mkdirs()
            assets.open(assetPath).use { input -> dest.outputStream().use { input.copyTo(it) } }
            return
        }
        dest.mkdirs()
        for (child in children) copyDir("$assetPath/$child", File(dest, child))
    }

    companion object {
        const val ASSET_ROOT = "engine"
    }
}
