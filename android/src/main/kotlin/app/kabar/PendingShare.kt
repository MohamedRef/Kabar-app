package app.kabar

import android.content.Context
import java.io.File

/**
 * One pending shared item, kept as a small JSON file so it survives the app
 * process being restarted between the share and Kabar opening. The newest share
 * wins. Rust calls [take] (through JNI) on start-up and whenever Kabar resumes.
 */
object PendingShare {
    private const val FILE_NAME = "pending-share.json"

    private fun file(context: Context) = File(context.noBackupFilesDir, FILE_NAME)

    @JvmStatic
    @Synchronized
    fun put(context: Context, json: String) {
        val target = file(context)
        val tmp = File(target.path + ".tmp")
        tmp.writeText(json)
        if (!tmp.renameTo(target)) {
            target.delete()
            tmp.renameTo(target)
        }
    }

    /** Returns the pending item's JSON and forgets it, or null if nothing is waiting. */
    @JvmStatic
    @Synchronized
    fun take(context: Context): String? {
        val target = file(context)
        if (!target.exists()) return null
        val json = target.readText()
        target.delete()
        return json.ifBlank { null }
    }
}
