import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "app.sahifa.reader"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.sahifa.reader"
        minSdk = 26
        targetSdk = 36
        versionCode = 3
        versionName = "0.3.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    // Release key lives outside the repo: ~/.android/oqu-keystore.properties
    // (or the file named by OQU_KEYSTORE_PROPS). Without it, release falls back to the debug key.
    val keyProps = Properties().apply {
        val f = file(System.getenv("OQU_KEYSTORE_PROPS") ?: "${System.getProperty("user.home")}/.android/oqu-keystore.properties")
        if (f.exists()) f.inputStream().use { load(it) }
    }
    signingConfigs {
        if (keyProps.isNotEmpty()) create("release") {
            storeFile = file(keyProps.getProperty("storeFile"))
            storePassword = keyProps.getProperty("storePassword")
            keyAlias = keyProps.getProperty("keyAlias")
            keyPassword = keyProps.getProperty("keyPassword")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }
    // One APK per CPU type (smaller downloads) plus a universal APK
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "armeabi-v7a", "x86_64")
            isUniversalApk = true
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlin { jvmToolchain(17) }
    buildFeatures { compose = true; buildConfig = true }
    packaging {
        // keep .so files uncompressed + 16 KB aligned inside the APK
        jniLibs.useLegacyPackaging = false
    }
    androidResources { noCompress += listOf("ttf") }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.09.00")
    implementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.core:core-ktx:1.17.0")
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.4")
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    // UniFFI's Kotlin bindings call into libreader_core.so through JNA
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
