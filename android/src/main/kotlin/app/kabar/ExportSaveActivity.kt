package app.kabar

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import java.io.File
import kotlin.concurrent.thread

/**
 * Settings › Export everything on Android. The export is first written to the
 * app's own folder (`Android/data/…`), which the Files app can't open; this
 * invisible activity then shows the system "Save to" dialog (Storage Access
 * Framework: Downloads, Drive…, no permission needed) and copies the file
 * there. Rust starts it through [ExportSave.open] and polls [ExportSave.state].
 */
class ExportSaveActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Recreated after a configuration change: the dialog is already showing.
        if (savedInstanceState != null) return
        val name = intent.getStringExtra(EXTRA_NAME) ?: "kabar-export.json"
        val create =
            Intent(Intent.ACTION_CREATE_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType("application/json")
                .putExtra(Intent.EXTRA_TITLE, name)
        try {
            @Suppress("DEPRECATION")
            startActivityForResult(create, REQUEST_SAVE)
        } catch (e: Exception) {
            Log.w(TAG, "No document UI", e)
            ExportSave.done(ExportSave.FAILED, null)
            finish()
        }
    }

    // The androidx result API needs a ComponentActivity; this module stays framework-only.
    @Suppress("OVERRIDE_DEPRECATION", "DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        val source = intent.getStringExtra(EXTRA_PATH)
        if (requestCode != REQUEST_SAVE || resultCode != RESULT_OK || uri == null || source == null) {
            ExportSave.done(ExportSave.CANCELLED, null)
            finish()
            return
        }
        val resolver = contentResolver
        thread(name = "kabar-export") {
            val saved =
                try {
                    resolver.openOutputStream(uri, "wt").use { out ->
                        requireNotNull(out) { "no output stream" }
                        File(source).inputStream().use { it.copyTo(out) }
                    }
                    displayName(uri) ?: ""
                } catch (e: Exception) {
                    Log.w(TAG, "Could not save the export", e)
                    null
                }
            ExportSave.done(if (saved != null) ExportSave.SAVED else ExportSave.FAILED, saved)
            runOnUiThread { finish() }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        // Closed without a result (e.g. the system killed the dialog).
        if (isFinishing && ExportSave.state() == ExportSave.OPEN) ExportSave.done(ExportSave.CANCELLED, null)
    }

    private fun displayName(uri: Uri): String? =
        try {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
                if (it.moveToFirst()) it.getString(0) else null
            }
        } catch (e: Exception) {
            null
        }

    companion object {
        private const val TAG = "KabarExport"
        private const val REQUEST_SAVE = 2
        const val EXTRA_PATH = "app.kabar.export.PATH"
        const val EXTRA_NAME = "app.kabar.export.NAME"
    }
}

/** Save-dialog state shared with Rust over JNI. */
object ExportSave {
    const val IDLE = 0
    const val OPEN = 1
    const val FAILED = 2
    const val SAVED = 3
    const val CANCELLED = 4

    @Volatile private var current = IDLE
    @Volatile private var savedName: String? = null

    /** Shows the "Save to" dialog for the file at [path], suggesting [name]. */
    @JvmStatic
    fun open(activity: Activity, path: String, name: String) {
        current = OPEN
        savedName = null
        activity.runOnUiThread {
            try {
                activity.startActivity(
                    Intent(activity, ExportSaveActivity::class.java)
                        .putExtra(ExportSaveActivity.EXTRA_PATH, path)
                        .putExtra(ExportSaveActivity.EXTRA_NAME, name),
                )
            } catch (e: Exception) {
                Log.w("KabarExport", "Could not start the save dialog", e)
                current = FAILED
            }
        }
    }

    /** IDLE, OPEN, FAILED, SAVED or CANCELLED. A finished state is reset once read. */
    @JvmStatic
    fun state(): Int = current

    /** The name the learner saved the file as (SAVED only). */
    @JvmStatic
    fun name(): String? = savedName

    @JvmStatic
    fun reset() {
        if (current != OPEN) current = IDLE
    }

    internal fun done(state: Int, name: String?) {
        savedName = name
        current = state
    }
}
