package app.kabar

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import org.json.JSONArray

/**
 * Kak Nur's local reminders (issue #127, phase 1): Rust works out the next few
 * reminders with `coach::reminders::upcoming` and their wording in the UI
 * language, then hands them over as JSON. Each becomes an inexact
 * [AlarmManager] alarm (no exact-alarm permission), and [ReminderReceiver]
 * posts it on the "Kak Nur" channel. Works offline and without Kabar Cloud.
 *
 * The schedule is replaced, never added to, every time the app re-plans
 * (studying, a settings change, a snooze), so a met goal silences the rest of
 * the day once the app has seen it. It is kept in SharedPreferences to
 * survive reboots and app updates.
 */
object ReminderBridge {
    const val CHANNEL = "kak_nur"
    private const val PREFS = "kabar_reminders"
    private const val KEY = "schedule"
    /** Alarms are request codes 0 until this; Rust never sends more. */
    const val MAX = 8
    private const val PERMISSION_REQUEST = 4127

    /** `[{"at": unix ms, "title": "...", "text": "..."}]`, soonest first. */
    @JvmStatic
    fun schedule(context: Context, json: String) {
        val app = context.applicationContext
        app.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY, json).apply()
        arm(app, json)
    }

    /** Re-arms the stored schedule (after a reboot or an update). */
    fun rearm(context: Context) {
        val json = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null) ?: return
        arm(context, json)
    }

    private fun arm(context: Context, json: String) {
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        for (i in 0 until MAX) {
            alarms.cancel(pending(context, i, null, null))
        }
        val items = try { JSONArray(json) } catch (e: org.json.JSONException) { JSONArray() }
        val now = System.currentTimeMillis()
        for (i in 0 until minOf(items.length(), MAX)) {
            val item = items.getJSONObject(i)
            val at = item.getLong("at")
            if (at <= now) continue
            val intent = pending(context, i, item.getString("title"), item.getString("text"))
            // Inexact while idle: Doze may hold it a few minutes, which suits a nudge.
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, intent)
        }
    }

    private fun pending(context: Context, code: Int, title: String?, text: String?): PendingIntent {
        val intent = Intent(context, ReminderReceiver::class.java).setAction("app.kabar.REMIND")
        title?.let { intent.putExtra("title", it) }
        text?.let { intent.putExtra("text", it) }
        return PendingIntent.getBroadcast(
            context, code, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    /** Shows the system prompt for notifications on Android 13+, if not granted yet. */
    @JvmStatic
    fun requestPermission(activity: Activity) {
        if (Build.VERSION.SDK_INT >= 33 &&
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), PERMISSION_REQUEST)
        }
    }

    fun show(context: Context, title: String, text: String) {
        val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, "Kak Nur", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "Gentle study reminders"
            },
        )
        val open = context.packageManager.getLaunchIntentForPackage(context.packageName)
            ?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED)
        val tap = open?.let {
            PendingIntent.getActivity(context, 0, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        }
        val note = Notification.Builder(context, CHANNEL)
            .setSmallIcon(app.kabar.bridge.R.drawable.ic_stat_kabar)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(Notification.BigTextStyle().bigText(text))
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        // One reminder at a time: a newer one replaces an unread older one.
        nm.notify(CHANNEL.hashCode(), note)
    }
}

/** Posts a scheduled reminder, and re-arms the schedule after a reboot or update. */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED -> ReminderBridge.rearm(context)
            else -> {
                val title = intent.getStringExtra("title") ?: return
                val text = intent.getStringExtra("text") ?: return
                val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                if (nm.areNotificationsEnabled()) ReminderBridge.show(context, title, text)
            }
        }
    }
}
