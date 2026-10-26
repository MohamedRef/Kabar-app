package app.kabar

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.util.Log
import kotlin.concurrent.thread

/**
 * Receives text and images shared from other apps (and "Kabar" from the
 * text-selection menu), stores them as a pending [PendingShare] JSON for Rust,
 * then brings Kabar's main activity to the front and closes itself.
 *
 * It has no UI. Copying an image can take a moment, so that runs off the main thread.
 */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val received = intent
        // "android-app://com.whatsapp" -> "com.whatsapp"; null when Android doesn't say.
        val source = referrer?.takeIf { it.scheme == "android-app" }?.host
        if (received?.action == Intent.ACTION_PROCESS_TEXT) {
            // We only read the selection; the other app keeps its text unchanged.
            setResult(RESULT_CANCELED)
        }
        val app = applicationContext
        thread(name = "kabar-share") {
            val json =
                try {
                    received?.let { SharedInputs.toJson(app, it, source) }
                } catch (e: Exception) {
                    Log.w(TAG, "Could not read the shared item", e)
                    null
                }
            if (json != null) PendingShare.put(app, json)
            runOnUiThread {
                openKabar()
                finish()
            }
        }
    }

    /** Same intent the launcher uses: resumes the existing task instead of restarting the app. */
    private fun openKabar() {
        val launch = packageManager.getLaunchIntentForPackage(packageName) ?: return
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED)
        startActivity(launch)
    }

    private companion object {
        const val TAG = "KabarShare"
    }
}
