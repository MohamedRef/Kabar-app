# WebView renderer-loss JVM regression tests

Run from the repository root with Java 17 and Gradle 8.10.2:

```sh
gradle --no-daemon -p android/renderer-tests test
```

This **standalone Kotlin/JVM Gradle build** compiles `android/src/main/kotlin/app/kabar/WebViewGuard.kt`
directly, plus small test-scope Android API doubles. It does not copy the production
logic, build an APK, start an emulator, or call `Process.killProcess`. The
`RendererRecovery` injection seam lets tests observe the existing foreground
restart/background stop policy without touching the real process.

The tests cover delayed WebView/client discovery and retry bounds, wrapper
idempotence, wry-client callback forwarding, renderer cleanup and `true`
acknowledgment, all foreground/background × crash/system-kill combinations,
committed diagnostics and startup consumption, Android 10 API gating, and
Android 11+ process-exit history sorting, descriptions, suppression of seen
records, and unavailable-history behavior.

**What they cannot verify:** actual WebView renderer behavior, Dioxus/wry
callback installation, OEM process management, Android's real shared-preference
and `ApplicationExitInfo` implementations, or the October 30 Samsung Device
Care report. Build the generated Android app and test a renderer loss on an
emulator and a physical device separately before concluding anything about
real-device recovery. A handled renderer exit is not evidence that the original
warning was caused by a renderer exit.
