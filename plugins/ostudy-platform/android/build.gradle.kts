import java.io.File
import java.net.URI
import java.nio.file.AtomicMoveNotSupportedException
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.MessageDigest

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

group = "io.github.jferments.ostudy"
version = "0.1.0"

val tesseractDataCommit = "87416418657359cb625c412a48b6e1d6d41c29bd"
val tesseractEnglishBlobSha1 = "bbef4675053b5b468cdb477053e28b1c698ba08e"
val tesseractEnglishBytes = 4_113_088L
val generatedTesseractAssets = layout.buildDirectory.dir("generated/tesseract-assets")

fun gitBlobSha1(file: File): String {
    val digest = MessageDigest.getInstance("SHA-1")
    digest.update("blob ${file.length()}\u0000".toByteArray(Charsets.UTF_8))
    file.inputStream().buffered().use { input ->
        val buffer = ByteArray(64 * 1024)
        while (true) {
            val count = input.read(buffer)
            if (count < 0) break
            digest.update(buffer, 0, count)
        }
    }
    return digest.digest().joinToString("") { "%02x".format(it) }
}

val prepareTesseractData by tasks.registering {
    val output = generatedTesseractAssets.map { it.file("tessdata/eng.traineddata") }
    outputs.file(output)
    doLast {
        val target = output.get().asFile
        if (target.isFile && target.length() == tesseractEnglishBytes &&
            gitBlobSha1(target) == tesseractEnglishBlobSha1
        ) {
            return@doLast
        }
        target.parentFile.mkdirs()
        val temporary = File(target.parentFile, "${target.name}.download")
        temporary.delete()
        val source = URI.create(
            "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/" +
                "$tesseractDataCommit/eng.traineddata",
        ).toURL()
        source.openConnection().apply {
            connectTimeout = 30_000
            readTimeout = 120_000
            setRequestProperty("User-Agent", "ostudy-android-build")
        }.getInputStream().use { input ->
            temporary.outputStream().buffered().use { outputStream -> input.copyTo(outputStream) }
        }
        require(temporary.length() == tesseractEnglishBytes) {
            "Pinned English Tesseract data has an unexpected byte length."
        }
        require(gitBlobSha1(temporary) == tesseractEnglishBlobSha1) {
            "Pinned English Tesseract data failed its immutable Git-blob identity check."
        }
        try {
            Files.move(
                temporary.toPath(),
                target.toPath(),
                StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING,
            )
        } catch (_: AtomicMoveNotSupportedException) {
            Files.move(
                temporary.toPath(),
                target.toPath(),
                StandardCopyOption.REPLACE_EXISTING,
            )
        }
    }
}

repositories {
    // Tesseract4Android is Apache-2.0 and published from its tagged source by JitPack.
    maven { url = uri("https://jitpack.io") }
}

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

    sourceSets {
        getByName("main").assets.srcDir(generatedTesseractAssets)
    }
}

tasks.named("preBuild").configure {
    dependsOn(prepareTesseractData)
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

    // Apache-2.0 OCR wrapper around Tesseract 5.5.1 / Leptonica 1.85.0.
    implementation("cz.adaptech.tesseract4android:tesseract4android:4.9.0")

    // Apache-2.0 local QR scanner. Pin ZXing core explicitly rather than accepting
    // the older transitive version declared by the embedded scanner artifact.
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
    implementation("com.google.zxing:core:3.5.4")
}
