# Oyun motoru (K104): native kod bu sınıflara adlarıyla ulaşır (JNI), R8
# adlarını değiştirmemeli ya da silmemeli.
-keep class net.kdt.pojavlaunch.** { *; }
-keep class org.lwjgl.glfw.CallbackBridge { *; }
-keep class com.oracle.dalvik.VMLauncher { *; }
-keep class com.mehbur.mcengine.GameActivity { *; }
-keepclasseswithmembernames class * {
    native <methods>;
}
