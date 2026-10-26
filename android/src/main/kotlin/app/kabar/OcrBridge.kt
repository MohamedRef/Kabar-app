package app.kabar

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.graphics.Rect
import android.media.ExifInterface
import android.os.Looper
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.Text
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.TextRecognizer
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import org.json.JSONArray
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlin.math.max

/**
 * On-device OCR with ML Kit Text Recognition v2 (Latin script, bundled model).
 *
 * Rust calls [recognize] over JNI from a background thread; it blocks until ML Kit
 * is done and returns the JSON of `kabar_protocol::reader::OcrResult`, with boxes in
 * pixels of the upright original image (origin top-left). Errors are thrown and
 * arrive in Rust as a caught Java exception.
 */
object OcrBridge {
    private const val TIMEOUT_SECONDS = 60L

    private val recognizer: TextRecognizer by lazy {
        TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
    }

    @JvmStatic
    fun recognize(path: String): String {
        check(Looper.myLooper() != Looper.getMainLooper()) { "OCR must not run on the main thread" }
        val image = UprightImage.load(path)
        try {
            val task = recognizer.process(InputImage.fromBitmap(image.bitmap, 0))
            val text = Tasks.await(task, TIMEOUT_SECONDS, TimeUnit.SECONDS)
            return toJson(text, image).toString()
        } finally {
            image.bitmap.recycle()
        }
    }

    private fun toJson(text: Text, image: UprightImage): JSONObject {
        val blocks = JSONArray()
        for (block in text.textBlocks) {
            val lines = JSONArray()
            for (line in block.lines) {
                lines.put(
                    JSONObject()
                        .put("text", line.text)
                        .put("bbox", rect(line.boundingBox, image.scale))
                        .put("confidence", line.confidence.toDouble()),
                )
            }
            if (lines.length() > 0) {
                blocks.put(JSONObject().put("bbox", rect(block.boundingBox, image.scale)).put("lines", lines))
            }
        }
        return JSONObject()
            .put("engine", "ml_kit")
            .put("width", image.width)
            .put("height", image.height)
            .put("blocks", blocks)
    }

    private fun rect(r: Rect?, scale: Float): JSONObject {
        val box = r ?: Rect()
        return JSONObject()
            .put("x", (box.left * scale).toDouble())
            .put("y", (box.top * scale).toDouble())
            .put("w", (box.width() * scale).toDouble())
            .put("h", (box.height() * scale).toDouble())
    }
}

/** A decoded bitmap turned upright per EXIF, plus the original upright size it maps to. */
internal class UprightImage(val bitmap: Bitmap, val width: Int, val height: Int, val scale: Float) {
    companion object {
        /** Longer sides are downsampled before recognition; boxes are scaled back. */
        private const val MAX_SIDE = 4096

        fun load(path: String): UprightImage {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(path, bounds)
            require(bounds.outWidth > 0 && bounds.outHeight > 0) { "Not an image Kabar can read" }

            var sample = 1
            while (max(bounds.outWidth, bounds.outHeight) / sample > MAX_SIDE) sample *= 2
            val decoded =
                BitmapFactory.decodeFile(path, BitmapFactory.Options().apply { inSampleSize = sample })
                    ?: throw IllegalArgumentException("Not an image Kabar can read")

            val degrees = exifRotation(path)
            val upright =
                if (degrees == 0) {
                    decoded
                } else {
                    val m = Matrix().apply { postRotate(degrees.toFloat()) }
                    Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, m, true).also {
                        if (it !== decoded) decoded.recycle()
                    }
                }
            val (w, h) =
                if (degrees % 180 == 0) bounds.outWidth to bounds.outHeight else bounds.outHeight to bounds.outWidth
            return UprightImage(upright, w, h, w.toFloat() / upright.width)
        }

        private fun exifRotation(path: String): Int =
            try {
                when (ExifInterface(path).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)) {
                    ExifInterface.ORIENTATION_ROTATE_90 -> 90
                    ExifInterface.ORIENTATION_ROTATE_180 -> 180
                    ExifInterface.ORIENTATION_ROTATE_270 -> 270
                    else -> 0
                }
            } catch (e: IOException) {
                0
            }
    }
}
