import java.io.File
import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.MessageDigest

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

group = "io.github.jferments.ostudy"
version = "0.1.0"

val tesseract4AndroidVersion = "4.9.0"
val tessdataCommit = "87416418657359cb625c412a48b6e1d6d41c29bd"
val tessdataEnglishBlobSha1 = "bbef4675053b5b468cdb477053e28b1c698ba08e"
val generatedTessdataAssets = layout.buildDirectory.dir("generated/tessdata/assets")
val generatedEnglishData = generatedTessdataAssets.map {
    it.file("tessdata/eng.traineddata").asFile
}

fun File.gitBlobSha1(): String {
    val digest = MessageDigest.getInstance("SHA-1")
    digest.update("blob ${length()}\u0000".toByteArray(StandardCharsets.UTF_8))
    inputStream().buffered().use { input ->
        val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            digest.update(buffer, 0, read)
        }
    }
    return digest.digest().joinToString("") { byte -> "%02x".format(byte) }
}

val prepareBundledTessdata by tasks.registering {
    description = "Downloads and verifies the pinned English Tesseract model."
    group = "build setup"
    outputs.file(generatedEnglishData)

    doLast {
        val target = generatedEnglishData.get()
        if (target.isFile && target.gitBlobSha1() == tessdataEnglishBlobSha1) return@doLast

        target.parentFile.mkdirs()
        val temporary = File(target.parentFile, "${target.name}.partial")
        temporary.delete()

        val localOverride = System.getenv("OSTUDY_TESSDATA_ENG")
            ?.takeIf(String::isNotBlank)
            ?.let(::File)
        if (localOverride != null) {
            require(localOverride.isFile) {
                "OSTUDY_TESSDATA_ENG does not point to a readable traineddata file."
            }
            localOverride.copyTo(temporary, overwrite = true)
        } else {
            val source = URI(
                "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/" +
                    "$tessdataCommit/eng.traineddata",
            ).toURL()
            source.openStream().buffered().use { input ->
                temporary.outputStream().buffered().use(input::copyTo)
            }
        }

        require(temporary.gitBlobSha1() == tessdataEnglishBlobSha1) {
            "The downloaded English Tesseract model does not match the pinned Git blob."
        }
        if (target.exists()) target.delete()
        if (!temporary.renameTo(target)) {
            temporary.copyTo(target, overwrite = true)
            temporary.delete()
        }
    }
}

android {
    namespace = "io.github.jferments.ostudy.platform"
    compileSdk = 36

    defaultConfig {
        minSdk = 28
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets.getByName("main").assets.srcDir(generatedTessdataAssets)

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

tasks.named("preBuild").configure {
    dependsOn(prepareBundledTessdata)
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

    // Apache-2.0 Tesseract is the primary Android OCR engine. The standard
    // single-threaded build is used because Rust already owns bounded page
    // concurrency and separate native instances can run in parallel safely.
    implementation(
        "cz.adaptech.tesseract4android:tesseract4android:$tesseract4AndroidVersion",
    )

    // ML Kit remains an offline fallback for pages on which Tesseract fails to
    // initialize, times out, throws, or returns unusably weak text.
    implementation("com.google.mlkit:text-recognition:16.0.1")

    // QR scanning is bundled in-app so pairing never depends on an external
    // barcode application or a network-delivered model.
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
}
