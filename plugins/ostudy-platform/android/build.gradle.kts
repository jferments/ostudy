plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

group = "io.github.jferments.ostudy"
version = "0.1.0"

android {
    namespace = "io.github.jferments.ostudy.platform"
    compileSdk = 36

    defaultConfig {
        minSdk = 28
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation(project(":tauri-android"))
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.lifecycle:lifecycle-common:2.10.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")
    implementation("com.fasterxml.jackson.core:jackson-core:2.19.2")
    implementation("com.fasterxml.jackson.core:jackson-databind:2.19.2")
    implementation("com.fasterxml.jackson.module:jackson-module-kotlin:2.19.2")

    // Bundled Text Recognition v2 keeps document OCR available immediately and
    // offline without a separate native OCR runtime or trained-data lifecycle.
    implementation("com.google.mlkit:text-recognition:16.0.1")

    // Google Code Scanner delegates the permissionless scanner UI to Google Play
    // services and returns only the decoded Barcode result to ostudy.
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
}
