// Standalone JVM harness: compile the actual Android production guard with small
// Android API test doubles, without requiring Dioxus, an APK, or an emulator.
plugins {
    kotlin("jvm") version "2.0.21"
}

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.4")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

kotlin {
    jvmToolchain(17)
}

// Compile production source directly rather than copying or reimplementing it.
tasks.named<org.jetbrains.kotlin.gradle.tasks.KotlinJvmCompile>("compileTestKotlin") {
    source(file("../src/main/kotlin/app/kabar/WebViewGuard.kt"))
    source(file("../src/main/kotlin/app/kabar/BackKeys.kt"))
}

tasks.test {
    useJUnitPlatform()
}
