import org.gradle.api.tasks.Delete
import org.gradle.api.tasks.testing.Test

buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.10.1")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
        maven {
            url = uri("https://jitpack.io")
            content {
                includeGroup("cz.adaptech.tesseract4android")
            }
        }
    }
    configurations.configureEach {
        resolutionStrategy.force(
            "org.jetbrains.kotlin:kotlin-stdlib:1.9.25",
            "org.jetbrains.kotlin:kotlin-stdlib-common:1.9.25",
            "org.jetbrains.kotlin:kotlin-stdlib-jdk7:1.9.25",
            "org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.9.25",
            "org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1",
            "org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.8.1",
        )
    }
}

subprojects {
    tasks.withType<Test>().configureEach {
        // Gradle's global worker pool remains the upper bound, so parallel test
        // forks cannot exceed the CPU budget selected for this invocation.
        maxParallelForks = gradle.startParameter.maxWorkerCount.coerceAtLeast(1)
    }
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
