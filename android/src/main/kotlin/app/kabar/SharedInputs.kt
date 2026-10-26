package app.kabar

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.webkit.MimeTypeMap
import org.json.JSONObject
import java.io.File
import java.io.IOException

/**
 * Turns a share intent into the JSON of `kabar_protocol::reader::SharedInput`:
 * `{"kind":"text","text":…,"source":…}` or `{"kind":"image","path":…,"mime":…,"source":…}`.
 */
internal object SharedInputs {
    /** Screenshots are a few MB; anything this big is not something we can read. */
    private const val MAX_IMAGE_BYTES = 40L * 1024 * 1024

    /** Copies older than this are removed from the cache on the next share. */
    private const val KEEP_COPIES_MS = 3L * 24 * 60 * 60 * 1000

    fun toJson(context: Context, intent: Intent, source: String?): String? =
        when (intent.action) {
            Intent.ACTION_PROCESS_TEXT -> text(intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT), source)
            Intent.ACTION_SEND -> {
                val type = intent.type.orEmpty()
                if (type.startsWith("image/")) {
                    image(context, stream(intent), type, source)
                } else {
                    text(intent.getCharSequenceExtra(Intent.EXTRA_TEXT), source)
                }
            }
            else -> null
        }

    private fun text(value: CharSequence?, source: String?): String? {
        val text = value?.toString()?.trim().orEmpty()
        if (text.isEmpty()) return null
        return JSONObject()
            .put("kind", "text")
            .put("text", text)
            .putOpt("source", source)
            .toString()
    }

    private fun image(context: Context, uri: Uri?, type: String, source: String?): String? {
        // Only content:// URIs: a file:// URI could point at our own private files.
        if (uri == null || uri.scheme != ContentResolver.SCHEME_CONTENT) return null
        val resolver = context.contentResolver
        val mime = resolver.getType(uri)?.takeIf { it.startsWith("image/") } ?: type
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime) ?: "img"

        val dir = File(context.cacheDir, "shared").apply { mkdirs() }
        pruneOldCopies(dir)
        val out = File(dir, "share-${System.currentTimeMillis()}.$ext")
        try {
            val input = resolver.openInputStream(uri) ?: return null
            input.use { src ->
                out.outputStream().use { dst ->
                    val buf = ByteArray(64 * 1024)
                    var total = 0L
                    while (true) {
                        val n = src.read(buf)
                        if (n < 0) break
                        total += n
                        if (total > MAX_IMAGE_BYTES) throw IOException("Shared image is larger than $MAX_IMAGE_BYTES bytes")
                        dst.write(buf, 0, n)
                    }
                }
            }
        } catch (e: Exception) {
            out.delete()
            throw e
        }
        return JSONObject()
            .put("kind", "image")
            .put("path", out.absolutePath)
            .put("mime", mime)
            .putOpt("source", source)
            .toString()
    }

    private fun stream(intent: Intent): Uri? =
        if (Build.VERSION.SDK_INT >= 33) {
            intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getParcelableExtra(Intent.EXTRA_STREAM)
        }

    private fun pruneOldCopies(dir: File) {
        val cutoff = System.currentTimeMillis() - KEEP_COPIES_MS
        dir.listFiles()?.filter { it.isFile && it.lastModified() < cutoff }?.forEach { it.delete() }
    }
}
