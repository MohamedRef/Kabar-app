package app.kabar

import android.app.NotificationManager
import android.content.Context
import android.content.SharedPreferences
import com.google.firebase.FirebaseApp
import com.google.firebase.messaging.FirebaseMessaging
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import org.json.JSONException
import org.json.JSONObject

/**
 * Kak Nur's reminders pushed by Kabar Cloud (issue #187). The Cloud's
 * quarter-hour alarm decides from every device's synced activity, so a goal
 * met on the tablet silences the phone too, and sends a data-only FCM message
 * `{kind, line, seed}` with nothing personal in it. [PushService] words it
 * from the phrasebook Rust keeps here (`coach::notify::phrasebook`: every
 * wording of the pushable lines in the UI language, name filled in) and shows
 * it like a local reminder. No Rust runs for a push, so it works with Kabar
 * closed.
 *
 * Firebase only starts in an APK built with its settings
 * (`scripts/android-firebase.py` turns google-services.json into string
 * resources, read by Firebase's own init provider). Without them [available]
 * is false and the local reminders in Reminders.kt stay in charge.
 *
 * While Kabar Cloud pushes to this device's current token ([pushed]), Rust
 * schedules no local reminders, so none arrives twice. A rotated token reads
 * as not pushed until the app has registered it again.
 */
object PushBridge {
    private const val PREFS = "kabar_push"
    private const val TOKEN = "token"
    private const val PUSHED_FOR = "pushed_for"
    private const val BOOK = "phrasebook"

    private fun prefs(context: Context): SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** Firebase started: this build carries its settings. */
    @JvmStatic
    fun available(context: Context): Boolean = try {
        FirebaseApp.getApps(context.applicationContext).isNotEmpty()
    } catch (e: Exception) {
        false
    }

    /**
     * This device's FCM token as last seen, or null before Firebase has
     * handed one over. Always asks Firebase again in the background, so a
     * rotated token shows on the next call.
     */
    @JvmStatic
    fun token(context: Context): String? {
        if (!available(context)) return null
        val prefs = prefs(context)
        try {
            FirebaseMessaging.getInstance().token.addOnSuccessListener { t ->
                if (!t.isNullOrEmpty()) storeToken(context, t)
            }
        } catch (e: Exception) {
            return null
        }
        return prefs.getString(TOKEN, null)
    }

    fun storeToken(context: Context, token: String) {
        prefs(context).edit().putString(TOKEN, token).apply()
    }

    /** Kabar Cloud pushes to the token Firebase holds now. */
    @JvmStatic
    fun pushed(context: Context): Boolean {
        val prefs = prefs(context)
        val pushedFor = prefs.getString(PUSHED_FOR, null)
        return !pushedFor.isNullOrEmpty() && pushedFor == prefs.getString(TOKEN, null)
    }

    /** Kabar Cloud pushes to `token` (empty: to no token on this device). */
    @JvmStatic
    fun setPushed(context: Context, token: String) {
        prefs(context).edit().putString(PUSHED_FOR, token).apply()
    }

    /** `{"title": "Kak Nur", "lines": {"remind_study": ["…", …], …}}` */
    @JvmStatic
    fun setPhrasebook(context: Context, json: String) {
        prefs(context).edit().putString(BOOK, json).apply()
    }

    /**
     * The title and text for a push: `lines[line][seed mod n]`, the wording
     * `voice::say(line, seed)` picks in the app (`coach::notify::push_text`).
     * A line this build doesn't know falls back to the plain study nudge.
     */
    fun words(context: Context, line: String, seed: Long): Pair<String, String>? = try {
        val book = JSONObject(prefs(context).getString(BOOK, null) ?: return null)
        val lines = book.getJSONObject("lines")
        val variants = lines.optJSONArray(line) ?: lines.optJSONArray("remind_study") ?: return null
        if (variants.length() == 0) return null
        val text = variants.getString(Math.floorMod(seed, variants.length().toLong()).toInt())
        Pair(book.optString("title", "Kak Nur"), text)
    } catch (e: JSONException) {
        null
    }
}

/** Receives Kabar Cloud's reminder pushes and Firebase's new tokens. */
class PushService : FirebaseMessagingService() {
    override fun onNewToken(token: String) {
        PushBridge.storeToken(this, token)
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val data = message.data
        val line = data["line"] ?: return
        val seed = data["seed"]?.toLongOrNull() ?: return
        // Not registered for this token (signed out, or rotated and not yet
        // re-sent): the local schedule is ringing instead.
        if (!PushBridge.pushed(this)) return
        val (title, text) = PushBridge.words(this, line, seed) ?: return
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (nm.areNotificationsEnabled()) {
            ReminderBridge.show(this, title, text)
        }
    }
}
