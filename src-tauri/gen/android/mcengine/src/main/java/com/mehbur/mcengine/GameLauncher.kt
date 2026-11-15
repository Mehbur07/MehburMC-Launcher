package com.mehbur.mcengine

import android.app.Activity
import android.app.ActivityManager
import android.content.Context
import android.os.Build
import android.system.Os
import android.util.Log
import com.oracle.dalvik.VMLauncher
import net.kdt.pojavlaunch.Logger
import net.kdt.pojavlaunch.utils.JREUtils
import java.io.File
import java.util.Locale
import java.util.TimeZone

/**
 * Java'yı bu süreçte başlatır (K104). Amethyst'in `JREUtils.launchJavaVM` ve
 * `Tools.launchMinecraft` adımlarının karşılığıdır (LGPL-3.0 kaynaktan
 * uyarlanmış mantık): ortam değişkenleri, kütüphane yolu, grafik çevirmeni,
 * JVM argümanları ve son olarak JLI_Launch.
 */
object GameLauncher {
    private const val TAG = "MehburEngine"
    const val RENDERER_MOBILEGLUES = "opengles_mobileglues"
    const val RENDERER_GL4ES = "opengles2"

    /** Cihazın belleğine göre üst sınır (Amethyst'in varsayılan tablosu, MB). */
    fun memoryCap(context: Context): Int {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val info = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
        val ram = (info.totalMem / (1024 * 1024)).toInt()
        return when {
            ram < 1024 -> 296
            ram < 1536 -> 448
            ram < 2048 -> 656
            ram < 3064 -> 936
            ram < 4096 -> 1144
            ram < 6144 -> 1536
            else -> 2048
        }
    }

    /** MobileGlues GLES 3 ister; yoksa gl4es kullanılır. */
    fun pickRenderer(context: Context, wanted: String): String {
        if (wanted != RENDERER_MOBILEGLUES) return RENDERER_GL4ES
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val gles = am.deviceConfigurationInfo.reqGlEsVersion
        return if (gles >= 0x30000) RENDERER_MOBILEGLUES else RENDERER_GL4ES
    }

    /** Java bitene kadar döner; çıkış kodunu verir. */
    fun launch(
        activity: Activity,
        plan: GamePlan,
        files: EngineFiles,
        windowWidth: Int,
        windowHeight: Int,
    ): Int {
        val home = plan.javaHome
        val jreLib = if (File(home, "lib/aarch64").isDirectory) "lib/aarch64" else "lib"
        val nativeDir = files.nativeLibDir
        val lwjglNatives = files.lwjglNatives(plan.lwjgl).absolutePath
        val cache = files.cacheDir.absolutePath
        val ldPath = listOf(
            "$home/$jreLib/jli",
            "$home/$jreLib",
            "/system/lib64",
            "/vendor/lib64",
            "/vendor/lib64/hw",
            nativeDir,
            lwjglNatives,
        ).joinToString(":")

        var renderer = pickRenderer(activity, plan.renderer)
        val env = linkedMapOf(
            "POJAV_NATIVEDIR" to nativeDir,
            "JAVA_HOME" to home,
            "HOME" to plan.gameDir,
            "TMPDIR" to cache,
            "LIBGL_MIPMAP" to "3",
            "LIBGL_NOERROR" to "1",
            "LIBGL_NOINTOVLHACK" to "1",
            "LIBGL_NORMALIZE" to "1",
            "LIBGL_ES" to "2",
            "FORCE_VSYNC" to "false",
            "MESA_GLSL_CACHE_DIR" to cache,
            "force_glsl_extensions_warn" to "true",
            "allow_higher_compat_version" to "true",
            "allow_glsl_extension_directive_midshader" to "true",
            "LD_LIBRARY_PATH" to ldPath,
            "PATH" to "$home/bin:" + (Os.getenv("PATH") ?: ""),
            "AMETHYST_RENDERER" to renderer,
            "AWTSTUB_WIDTH" to windowWidth.toString(),
            "AWTSTUB_HEIGHT" to windowHeight.toString(),
        )
        if (renderer == RENDERER_MOBILEGLUES) {
            val mg = File(files.root, "MobileGlues")
            writeMobileGluesConfig(mg)
            env["MG_DIR_PATH"] = mg.absolutePath
            env["POJAVEXEC_EGL"] = "libmobileglues.so"
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O &&
            Build.MANUFACTURER.lowercase(Locale.ROOT).contains("huawei")
        ) {
            env["POJAV_EMUI_ITERATOR_MITIGATE"] = "1"
        }
        for ((k, v) in env) {
            Logger.appendToLog("env $k=$v")
            Os.setenv(k, v, true)
        }

        val serverDir = File("$home/$jreLib/server/libjvm.so")
        val jvmLibDir = "$home/$jreLib/" + if (serverDir.exists()) "server" else "client"
        JREUtils.setLdLibraryPath("$jvmLibDir:$ldPath")

        val graphicsLib = loadGraphicsLibrary(renderer, nativeDir).also { (lib, used) ->
            if (used != renderer) {
                renderer = used
                Os.setenv("AMETHYST_RENDERER", used, true)
                Os.unsetenv("POJAVEXEC_EGL")
            }
            Logger.appendToLog("renderer $used ($lib)")
        }.first

        val memory = minOf(plan.memoryMb, memoryCap(activity)).coerceAtLeast(296)
        val args = buildArgs(plan, files, renderer, graphicsLib, memory, windowWidth, windowHeight)
        Logger.appendToLog("Java ${plan.javaMajor}, ${memory} MB, LWJGL ${plan.lwjgl}")

        prepareOptions(plan.gameDir, windowWidth, windowHeight)
        initJavaRuntime(home, jreLib, jvmLibDir, nativeDir)
        JREUtils.setupExitMethod(activity.application)
        JREUtils.initializeHooks()
        JREUtils.chdir(plan.gameDir)
        Log.i(TAG, "launching ${plan.versionId}")
        return VMLauncher.launchJVM(args.toTypedArray())
    }

    private fun buildArgs(
        plan: GamePlan,
        files: EngineFiles,
        renderer: String,
        graphicsLib: String,
        memory: Int,
        width: Int,
        height: Int,
    ): List<String> {
        val nativeDir = files.nativeLibDir
        val comps = files.components
        val lwjglNatives = files.lwjglNatives(plan.lwjgl).absolutePath
        val user = purge(plan.jvmArgs)

        // Profil aynısını vermediyse eklenir (Amethyst: getJavaArgs).
        val defaults = listOf(
            "-Djava.home=${plan.javaHome}",
            "-Djava.io.tmpdir=${files.cacheDir.absolutePath}",
            "-Djna.boot.library.path=$nativeDir",
            "-Duser.home=${plan.gameDir}",
            "-Duser.language=${Locale.getDefault().language}",
            "-Dos.name=Linux",
            "-Dos.version=Android-${Build.VERSION.RELEASE}",
            "-Dpojav.path.minecraft=${plan.gameDir}",
            "-Dpojav.path.private.account=${File(files.root, "accounts").absolutePath}",
            "-Duser.timezone=${TimeZone.getDefault().id}",
            "-Dorg.lwjgl.vulkan.libname=libvulkan.so",
            "-Dglfwstub.windowWidth=$width",
            "-Dglfwstub.windowHeight=$height",
            "-Dglfwstub.initEgl=false",
            "-Dext.net.resolvPath=${File(comps, "resolv.conf").absolutePath}",
            "-Dlog4j2.formatMsgNoLookups=true",
            "-Dnet.minecraft.clientmodname=MehburMC",
            "-Dfml.earlyprogresswindow=false",
            "-Dloader.disable_forked_guis=true",
            "-Djdk.lang.Process.launchMechanism=FORK",
        ).filter { d ->
            val key = d.substringBefore('=')
            user.none { it.startsWith(key) }
        }

        val args = mutableListOf("java")
        args += user
        args += defaults
        args += "-Xms${memory}M"
        args += "-Xmx${memory}M"
        args += "-Dorg.lwjgl.opengl.libname=$graphicsLib"
        args += "-Dorg.lwjgl.freetype.libname=$lwjglNatives/libfreetype.so"
        args += "-Dorg.lwjgl.spvc.libname=spirv-cross-c-shared"
        args += "-Dorg.lwjgl.system.allocator=system"
        args += "-XX:ActiveProcessorCount=${Runtime.getRuntime().availableProcessors()}"
        args += "-javaagent:${File(comps, "methods_injector_agent/methods_injector_agent.jar").absolutePath}"

        args += cacioArgs(plan.javaMajor == 8, comps)
        logConfig(plan, comps)?.let { args += "-Dlog4j.configurationFile=$it" }
        args += "-Djava.library.path=$lwjglNatives:$nativeDir"
        args += "-cp"
        args += classpath(plan, files)
        args += "-Dorg.lwjgl.librarypath=$lwjglNatives"
        args += "-Dfml.ignoreInvalidMinecraftCertificates=true"
        args += "-Dimgui.library.name=imgui-java"
        args += plan.mainClass
        args += plan.gameArgs
        Log.i(TAG, "renderer=$renderer args=${args.size}")
        return args
    }

    /** Telefonda zarar veren ya da motorun kendi verdiği argümanlar (Amethyst'teki liste). */
    private fun purge(args: List<String>): List<String> {
        val banned = listOf(
            "-Xms", "-Xmx", "-d32", "-d64", "-Xint",
            "-XX:+UseTransparentHugePages", "-XX:+UseLargePagesInMetaspace", "-XX:+UseLargePages",
            "-Dorg.lwjgl.opengl.libname", "-Dorg.lwjgl.freetype.libname", "-XX:ActiveProcessorCount",
        )
        return args.filter { a -> banned.none { a.startsWith(it) } }
    }

    /** Önce motorun LWJGL'i (lwjgl.jar, birleşik modüller, diğerleri), sonra oyun. */
    private fun classpath(plan: GamePlan, files: EngineFiles): String {
        val dir = files.lwjglJars(plan.lwjgl)
        val core = File(dir, "lwjgl.jar")
        val merged = File(dir, "lwjgl-${plan.lwjgl}-merged-modules.jar")
        val lwjglx = File(dir, "lwjgl-lwjglx.jar")
        check(core.isFile && merged.isFile) { "LWJGL ${plan.lwjgl} is missing from the engine files" }
        val modules = dir.listFiles { f ->
            f.name.endsWith(".jar") && f.name != core.name && f.name != merged.name && f.name != lwjglx.name
        }.orEmpty().sortedBy { it.name }
        val parts = mutableListOf(core.absolutePath, merged.absolutePath)
        parts += modules.map { it.absolutePath }
        parts += plan.classpath
        if (plan.lwjglx) parts += lwjglx.absolutePath
        return parts.joinToString(":")
    }

    /** AWT için Caciocavallo (Amethyst: getCacioJavaArgs). */
    private fun cacioArgs(java8: Boolean, comps: File): List<String> {
        val out = mutableListOf(
            "-Djava.awt.headless=false",
            "-Dcacio.managed.screensize=1280x720",
            "-Dcacio.font.fontmanager=sun.awt.X11FontManager",
            "-Dcacio.font.fontscaler=sun.font.FreetypeFontScaler",
            "-Dswing.defaultlaf=javax.swing.plaf.metal.MetalLookAndFeel",
        )
        val dir = File(comps, if (java8) "caciocavallo" else "caciocavallo17")
        val jars = dir.listFiles { f -> f.name.endsWith(".jar") }.orEmpty().sortedBy { it.name }
        if (java8) {
            out += "-Dawt.toolkit=net.java.openjdk.cacio.ctc.CTCToolkit"
            out += "-Djava.awt.graphicsenv=net.java.openjdk.cacio.ctc.CTCGraphicsEnvironment"
        } else {
            jars.firstOrNull { it.name.contains("cacio-tta") }?.let { out += "-javaagent:${it.absolutePath}" }
            out += "-Dawt.toolkit=com.github.caciocavallosilano.cacio.ctc.CTCToolkit"
            out += "-Djava.awt.graphicsenv=com.github.caciocavallosilano.cacio.ctc.CTCGraphicsEnvironment"
            for (p in listOf(
                "java.desktop/java.awt", "java.desktop/java.awt.peer", "java.desktop/sun.awt.image",
                "java.desktop/sun.java2d", "java.desktop/java.awt.dnd.peer", "java.desktop/sun.awt",
                "java.desktop/sun.awt.event", "java.desktop/sun.awt.datatransfer", "java.desktop/sun.font",
                "java.base/sun.security.action",
            )) out += "--add-exports=$p=ALL-UNNAMED"
            for (p in listOf(
                "java.base/java.util", "java.desktop/java.awt", "java.desktop/sun.font",
                "java.desktop/sun.java2d", "java.base/java.lang.reflect", "java.base/java.net",
            )) out += "--add-opens=$p=ALL-UNNAMED"
        }
        val boot = jars.joinToString("") { ":" + it.absolutePath }
        out += "-Xbootclasspath/" + (if (java8) "p" else "a") + boot
        return out
    }

    /** Mojang'ın log4j ayarının güvenlik yamalı ve düz metin çıktılı karşılığı. */
    private fun logConfig(plan: GamePlan, comps: File): String? {
        val id = plan.logConfigId ?: return null
        val patched = File(comps, "security/" + id.replace("client", "log4j-rce-patch"))
        if (patched.isFile) return patched.absolutePath
        return plan.logConfigPath?.takeIf { File(it).isFile }
    }

    /** Grafik çevirmenini yükler; olmazsa gl4es'e düşer. (kütüphane, çevirmen) */
    private fun loadGraphicsLibrary(renderer: String, nativeDir: String): Pair<String, String> {
        val lib = if (renderer == RENDERER_MOBILEGLUES) "libmobileglues.so" else "libng_gl4es.so"
        if (JREUtils.dlopen(lib) || JREUtils.dlopen("$nativeDir/$lib")) return lib to renderer
        Log.e(TAG, "could not load $lib, falling back to gl4es")
        JREUtils.dlopen("$nativeDir/libng_gl4es.so")
        return "libng_gl4es.so" to RENDERER_GL4ES
    }

    private fun findInLdLibPath(name: String): String {
        val path = Os.getenv("LD_LIBRARY_PATH") ?: return name
        for (dir in path.split(':')) {
            val f = File(dir, name)
            if (f.isFile) return f.absolutePath
        }
        return name
    }

    /** JRE kütüphanelerini doğru sırayla yükler (Amethyst: initJavaRuntime). */
    private fun initJavaRuntime(home: String, jreLib: String, jvmLibDir: String, nativeDir: String) {
        JREUtils.dlopen(findInLdLibPath("libjli.so"))
        if (!JREUtils.dlopen("libjvm.so")) JREUtils.dlopen("$jvmLibDir/libjvm.so")
        for (name in listOf(
            "libverify.so", "libjava.so", "libnet.so", "libnio.so", "libawt.so",
            "libawt_headless.so", "libfreetype.so", "libfontmanager.so",
        )) JREUtils.dlopen(findInLdLibPath(name))
        File(home, jreLib).walkTopDown()
            .filter { it.isFile && it.name.endsWith(".so") }
            .forEach { JREUtils.dlopen(it.absolutePath) }
        JREUtils.dlopen("$nativeDir/libopenal.so")
    }

    /** Pencere boyutunu oyuna bildirir (Amethyst: realStart → MCOptionUtils). */
    private fun prepareOptions(gameDir: String, width: Int, height: Int) {
        val file = File(gameDir, "options.txt")
        val lines = if (file.isFile) file.readLines().toMutableList() else mutableListOf()
        fun set(key: String, value: String) {
            val i = lines.indexOfFirst { it.startsWith("$key:") }
            if (i >= 0) lines[i] = "$key:$value" else lines += "$key:$value"
        }
        set("fullscreen", "false")
        set("overrideWidth", width.toString())
        set("overrideHeight", height.toString())
        file.parentFile?.mkdirs()
        file.writeText(lines.joinToString("\n", postfix = "\n"))
    }

    /** MobileGlues varsayılan ayarları (Amethyst: writeMGRendererSettings). */
    private fun writeMobileGluesConfig(dir: File) {
        dir.mkdirs()
        File(dir, "config.json").writeText(
            """{"enableANGLE":0,"enableNoError":0,"fsr1Setting":0,"enableExtComputeShader":0,""" +
                """"angleDepthClearFixMode":0,"enableExtTimerQuery":0,"enableExtDirectStateAccess":0,""" +
                """"multidrawMode":0,"maxGlslCacheSize":128}""",
        )
    }
}
