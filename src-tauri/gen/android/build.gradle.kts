import org.gradle.api.tasks.Delete

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
    }
    configurations.configureEach {
        resolutionStrategy.force(
            "org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1",
            "org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.8.1",
        )
    }
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
