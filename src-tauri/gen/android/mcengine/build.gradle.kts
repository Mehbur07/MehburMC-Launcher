// Oyun motoru modülü (K104): Minecraft'ı telefonda çalıştıran Amethyst
// parçaları (LGPL-3.0) ve bizim oyun ekranımız. Hazır native kütüphaneler ve
// varlıklar `scripts/android-engine.mjs` ile src/main/jniLibs ve
// src/main/assets/engine altına konur (git'e girmez).
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.mehbur.mcengine"
    compileSdk = 37
    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

dependencies {
    implementation("androidx.annotation:annotation:1.9.1")
}
