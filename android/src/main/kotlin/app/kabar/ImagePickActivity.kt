package app.kabar

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
import kotlin.concurrent.thread

/**
 * The Reader's "Open a screenshot" on Android. The WebView's file input can't be
 * used: Dioxus intercepts it and opens a desktop-only native dialog, so nothing
 * comes back. Rust starts this invisible activity through [ImagePick.open]; it shows
 * the system photo picker (no storage permission needed), copies the chosen image
 * like a share and stores it as a [PendingShare], then closes. Rust polls
 * [ImagePick.state] and [PendingShare.take] until the picker is done.
 */
class ImagePickActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Recreated after a configuration change: the picker is already showing.
        if (savedInstanceState != null) return
        try {
            @Suppress("DEPRECATION")
            startActivityForResult(pickIntent(), REQUEST_PICK)
        } catch (e: Exception) {
            Log.w(TAG, "No image picker", e)
            ImagePick.done(ImagePick.FAILED)
            finish()
        }
    }

    // The androidx result API needs a ComponentActivity; this module stays framework-only.
    @Suppress("OVERRIDE_DEPRECATION", "DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (requestCode != REQUEST_PICK || resultCode != RESULT_OK || uri == null) {
            ImagePick.done(ImagePick.IDLE)
            finish()
            return
        }
        val app = applicationContext
        // Read the type while this activity still holds the URI grant.
        val type = contentResolver.getType(uri) ?: "image/*"
        thread(name = "kabar-pick") {
            val json =
                try {
                    SharedInputs.image(app, uri, type, null)
                } catch (e: Exception) {
                    Log.w(TAG, "Could not read the picked image", e)
                    null
                }
            if (json != null) PendingShare.put(app, json)
            ImagePick.done(if (json != null) ImagePick.IDLE else ImagePick.FAILED)
            runOnUiThread { finish() }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        // Closed without a result (e.g. the system killed the picker).
        if (isFinishing && ImagePick.state() == ImagePick.OPEN) ImagePick.done(ImagePick.IDLE)
    }

    private fun pickIntent(): Intent =
        if (Build.VERSION.SDK_INT >= 33) {
            // The system photo picker: recent screenshots first, no permission.
            Intent(MediaStore.ACTION_PICK_IMAGES).setType("image/*")
        } else {
            Intent(Intent.ACTION_GET_CONTENT).setType("image/*").addCategory(Intent.CATEGORY_OPENABLE)
        }

    private companion object {
        const val TAG = "KabarPick"
        const val REQUEST_PICK = 1
    }
}

/** Picker state shared with Rust over JNI. */
object ImagePick {
    const val IDLE = 0
    const val OPEN = 1
    const val FAILED = 2

    @Volatile private var current = IDLE

    /** Shows the photo picker on top of Kabar. */
    @JvmStatic
    fun open(activity: Activity) {
        current = OPEN
        activity.runOnUiThread {
            try {
                activity.startActivity(Intent(activity, ImagePickActivity::class.java))
            } catch (e: Exception) {
                Log.w("KabarPick", "Could not start the picker", e)
                current = FAILED
            }
        }
    }

    /** IDLE (done or cancelled; a picked image is waiting in [PendingShare]), OPEN or FAILED. */
    @JvmStatic
    fun state(): Int = current

    /** Rust saw FAILED and showed a message. */
    @JvmStatic
    fun reset() {
        if (current == FAILED) current = IDLE
    }

    internal fun done(state: Int) {
        current = state
    }
}
