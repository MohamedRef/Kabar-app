package app.kabar

import android.app.Activity
import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.content.MemoryPreferences
import android.graphics.Bitmap
import android.os.Build
import android.os.UiTasks
import android.util.Log
import android.view.FrameLayout
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNotSame
import kotlin.test.assertSame
import kotlin.test.assertTrue

class WebViewGuardTest {
    @BeforeEach
    fun reset() {
        UiTasks.clear()
        Log.warnings.clear()
        Build.VERSION.SDK_INT = Build.VERSION_CODES.R
    }

    private class RecordingClient : WebViewClient() {
        val callbacks = mutableListOf<String>()
        val response = WebResourceResponse()
        var lastRequest: WebResourceRequest? = null
        var lastError: WebResourceError? = null
        var lastIcon: Bitmap? = null

        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse {
            callbacks.add("intercept")
            lastRequest = request
            return response
        }

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            callbacks.add("navigate")
            lastRequest = request
            return true
        }

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            callbacks.add("started:$url")
            lastIcon = favicon
        }

        override fun onPageFinished(view: WebView, url: String) {
            callbacks.add("finished:$url")
        }

        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            callbacks.add("error")
            lastRequest = request
            lastError = error
        }
    }

    private class FakeRecovery(val activity: Activity, val web: WebView? = null) : RendererRecovery {
        var foreground = false
        val calls = mutableListOf<String>()
        var noteAtStop: String? = null

        override fun inForeground(): Boolean = foreground

        override fun restart(activity: Activity) {
            assertSame(this.activity, activity)
            assertTrue(web == null || web.parent == null && web.destroyed)
            calls.add("restart")
        }

        override fun stopProcess() {
            assertTrue(web == null || web.parent == null && web.destroyed)
            val prefs = activity.getSharedPreferences("kabar_exits", Context.MODE_PRIVATE)
            assertTrue((prefs as MemoryPreferences).commits > 0, "diagnostic must be committed before process stop")
            noteAtStop = prefs.getString("renderer_gone", null)
            calls.add("stop")
        }
    }

    private data class Fixture(
        val activity: Activity,
        val parent: FrameLayout,
        val web: WebView,
        val client: RecordingClient,
        val recovery: FakeRecovery,
    )

    private fun fixture(): Fixture {
        val activity = Activity()
        val parent = FrameLayout()
        (activity.window.decorView as FrameLayout).addView(parent)
        val web = WebView()
        parent.addView(web)
        val inner = RecordingClient()
        web.webViewClient = inner
        val recovery = FakeRecovery(activity, web)
        assertEquals("", WebViewGuard.install(activity, recovery))
        assertNotSame(inner, web.webViewClient)
        return Fixture(activity, parent, web, inner, recovery)
    }

    @Test
    fun delayedDiscoveryWaitsForWebViewAndRealClient() {
        val activity = Activity()
        val recovery = FakeRecovery(activity)
        assertEquals("", WebViewGuard.install(activity, recovery))
        assertEquals(1, UiTasks.pending)

        val nested = FrameLayout()
        (activity.window.decorView as FrameLayout).addView(nested)
        val web = WebView()
        nested.addView(web)
        assertEquals(250L, UiTasks.step())
        assertEquals(WebViewClient::class.java, web.webViewClient.javaClass)
        assertEquals(1, UiTasks.pending)

        val client = RecordingClient()
        web.webViewClient = client
        assertEquals(250L, UiTasks.step())
        assertNotSame(client, web.webViewClient)
        assertEquals(0, UiTasks.pending)
        web.webViewClient.onPageFinished(web, "https://example.test/ready")
        assertEquals(listOf("finished:https://example.test/ready"), client.callbacks)
    }

    @Test
    fun missingWebViewStopsRetryingAfterBoundedWindow() {
        val activity = Activity()
        WebViewGuard.install(activity, FakeRecovery(activity))
        var attempts = 0
        while (UiTasks.pending > 0) {
            assertEquals(250L, UiTasks.step())
            attempts++
            assertTrue(attempts <= 40, "retry budget should be bounded")
        }
        assertEquals(40, attempts)
        assertTrue(Log.warnings.any { it.contains("No WebView to guard") })
    }

    @Test
    fun retriesStopForDestroyedOrFinishingActivity() {
        for (destroyed in listOf(true, false)) {
            UiTasks.clear()
            val activity = Activity()
            WebViewGuard.install(activity, FakeRecovery(activity))
            if (destroyed) activity.isDestroyed = true else activity.isFinishing = true
            UiTasks.step()
            assertEquals(0, UiTasks.pending)
        }
    }

    @Test
    fun overlappingDelayedInstallsStillAttachOnlyOneWrapper() {
        val activity = Activity()
        val recovery = FakeRecovery(activity)
        WebViewGuard.install(activity, recovery)
        WebViewGuard.install(activity, recovery)
        assertEquals(2, UiTasks.pending)

        val web = WebView()
        val client = RecordingClient()
        web.webViewClient = client
        (activity.window.decorView as FrameLayout).addView(web)
        UiTasks.step()
        val first = web.webViewClient
        UiTasks.step()
        assertSame(first, web.webViewClient)
        assertEquals(0, UiTasks.pending)
        web.webViewClient.onPageFinished(web, "https://example.test/one-delegate")
        assertEquals(listOf("finished:https://example.test/one-delegate"), client.callbacks)
    }

    @Test
    fun multipleInstallsNeverStackClientWrappers() {
        val f = fixture()
        val first = f.web.webViewClient
        WebViewGuard.install(f.activity, f.recovery)
        WebViewGuard.install(f.activity, f.recovery)
        assertSame(first, f.web.webViewClient)
        f.web.webViewClient.onPageFinished(f.web, "https://example.test/once")
        assertEquals(listOf("finished:https://example.test/once"), f.client.callbacks)
    }

    @Test
    fun allWryClientCallbacksAreForwardedWithoutChangingResults() {
        val f = fixture()
        val request = object : WebResourceRequest {}
        val error = WebResourceError()
        val icon = Bitmap()
        val guard = f.web.webViewClient

        assertSame(f.client.response, guard.shouldInterceptRequest(f.web, request))
        assertTrue(guard.shouldOverrideUrlLoading(f.web, request))
        guard.onPageStarted(f.web, "https://example.test/start", icon)
        guard.onPageFinished(f.web, "https://example.test/end")
        guard.onReceivedError(f.web, request, error)

        assertEquals(listOf("intercept", "navigate", "started:https://example.test/start",
            "finished:https://example.test/end", "error"), f.client.callbacks)
        assertSame(request, f.client.lastRequest)
        assertSame(error, f.client.lastError)
        assertSame(icon, f.client.lastIcon)
    }

    @Test
    fun rendererLossDetachesDestroysReportsHandledAndUsesCorrectPolicy() {
        for (crashed in listOf(false, true)) {
            for (foreground in listOf(false, true)) {
                val f = fixture()
                f.recovery.foreground = foreground
                var didCrashCalls = 0
                val detail = object : RenderProcessGoneDetail(crashed) {
                    override fun didCrash(): Boolean { didCrashCalls++; return super.didCrash() }
                }
                assertTrue(f.web.webViewClient.onRenderProcessGone(f.web, detail))
                assertEquals(1, didCrashCalls)
                assertEquals(0, f.parent.childCount)
                assertTrue(f.web.parent == null)
                assertTrue(f.web.destroyed)
                assertEquals(if (foreground) listOf("restart", "stop") else listOf("stop"), f.recovery.calls)

                val note = assertNotNull(f.recovery.noteAtStop)
                assertTrue(note.contains(if (crashed) "screen renderer crashed" else "screen renderer was stopped by Android"))
                assertTrue(note.contains(if (foreground) "in front; Kabar restarted" else "in the background; Kabar closed"))
            }
        }
    }

    @Test
    fun rendererDiagnosticIsCommittedSurvivesStartupAndIsConsumedOnce() {
        val f = fixture()
        f.recovery.foreground = false
        f.web.webViewClient.onRenderProcessGone(f.web, RenderProcessGoneDetail(true))
        val stored = assertNotNull(f.recovery.noteAtStop)
        // A fresh Activity/process sees the same disk-backed preference store.
        val nextActivity = Activity(f.activity.preferenceStore)
        val nextRecovery = FakeRecovery(nextActivity)
        assertEquals(stored, WebViewGuard.install(nextActivity, nextRecovery))
        assertEquals("", WebViewGuard.install(nextActivity, nextRecovery))
        assertEquals(null, nextActivity.getSharedPreferences("kabar_exits", Context.MODE_PRIVATE)
            .getString("renderer_gone", null))
    }

    @Test
    fun exitHistoryIsChronologicalDescribedAndNotRepeatedAcrossLaunches() {
        val f = fixture()
        val am = ActivityManager()
        f.activity.setService(ActivityManager::class.java, am)
        val prefs = f.activity.getSharedPreferences("kabar_exits", Context.MODE_PRIVATE)
        prefs.edit().putLong("seen", 1500L).apply()
        am.history = listOf(
            ApplicationExitInfo(3000L, ApplicationExitInfo.REASON_LOW_MEMORY, 300, "memory pressure"),
            ApplicationExitInfo(1000L, ApplicationExitInfo.REASON_ANR, 100, "old"),
            ApplicationExitInfo(2000L, ApplicationExitInfo.REASON_CRASH, 100, "Java exception"),
        )
        val first = WebViewGuard.install(f.activity, f.recovery).lines()
        assertEquals(2, first.size)
        assertTrue(first[0].contains("CRASH (Java) (in front): Java exception"))
        assertTrue(first[1].contains("low memory (in the background): memory pressure"))
        assertEquals(3000L, prefs.getLong("seen", 0L))
        assertEquals(listOf(Triple(f.activity.packageName, 0, 10)), am.queries)
        assertEquals("", WebViewGuard.install(f.activity, f.recovery))

        am.history = am.history + ApplicationExitInfo(4000L, ApplicationExitInfo.REASON_CRASH_NATIVE, 300)
        val next = WebViewGuard.install(f.activity, f.recovery)
        assertEquals(1, next.lines().size)
        assertTrue(next.contains("CRASH (native) (in the background)"))
        assertFalse(next.contains("Java exception"))
        assertEquals(4000L, prefs.getLong("seen", 0L))
    }

    @Test
    fun exitHistoryUnavailableDoesNotAdvanceSeenOrLoseLaterRecords() {
        val f = fixture()
        val prefs = f.activity.getSharedPreferences("kabar_exits", Context.MODE_PRIVATE)
        prefs.edit().putLong("seen", 900L).apply()
        assertEquals("", WebViewGuard.install(f.activity, f.recovery)) // no ActivityManager
        assertEquals(900L, prefs.getLong("seen", 0L))

        val am = ActivityManager()
        am.history = listOf(ApplicationExitInfo(1000L, ApplicationExitInfo.REASON_ANR, 100, "blocked"))
        am.failQuery = true
        f.activity.setService(ActivityManager::class.java, am)
        assertEquals("", WebViewGuard.install(f.activity, f.recovery)) // denied history
        assertEquals(900L, prefs.getLong("seen", 0L))

        am.failQuery = false
        val recovered = WebViewGuard.install(f.activity, f.recovery)
        assertTrue(recovered.contains("ANR (not responding) (in front): blocked"))
        assertEquals(1000L, prefs.getLong("seen", 0L))
    }

    @Test
    fun android10ConsumesRendererNoteButDoesNotQueryUnavailableExitApi() {
        val f = fixture()
        val prefs = f.activity.getSharedPreferences("kabar_exits", Context.MODE_PRIVATE)
        prefs.edit().putString("renderer_gone", "renderer saved before restart").apply()
        val am = ActivityManager()
        am.history = listOf(ApplicationExitInfo(1000L, ApplicationExitInfo.REASON_OTHER, 300))
        f.activity.setService(ActivityManager::class.java, am)

        Build.VERSION.SDK_INT = 29
        assertEquals("renderer saved before restart", WebViewGuard.install(f.activity, f.recovery))
        assertEquals(0, am.queries.size)
        assertEquals(0L, prefs.getLong("seen", 0L))
        assertEquals("", WebViewGuard.install(f.activity, f.recovery))

        Build.VERSION.SDK_INT = Build.VERSION_CODES.R
        assertTrue(WebViewGuard.install(f.activity, f.recovery).contains("other (system) (in the background)"))
        assertEquals(1000L, prefs.getLong("seen", 0L))
    }

    @Test
    fun startupReturnsSavedRendererNoteBeforeNewExitHistory() {
        val f = fixture()
        val prefs = f.activity.getSharedPreferences("kabar_exits", Context.MODE_PRIVATE)
        prefs.edit().putString("renderer_gone", "renderer recorded before process stop").apply()
        val am = ActivityManager()
        am.history = listOf(ApplicationExitInfo(1000L, ApplicationExitInfo.REASON_EXIT_SELF, 100))
        f.activity.setService(ActivityManager::class.java, am)
        val lines = WebViewGuard.install(f.activity, f.recovery).lines()
        assertEquals(2, lines.size)
        assertEquals("renderer recorded before process stop", lines[0])
        assertTrue(lines[1].contains("last run ended: exited (in front)"))
        assertEquals("", WebViewGuard.install(f.activity, f.recovery))
    }
}
