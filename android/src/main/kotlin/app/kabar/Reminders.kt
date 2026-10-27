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
import android.os.PowerManager
import org.json.JSONArray
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId

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
 * survive reboots and app updates. Alarms are armed from each reminder's
 * local date and time in the device's zone, and re-armed when the zone or
 * the clock changes, so daylight saving never moves 19:00. Shown reminders
 * are remembered until the app takes them ([takeDelivered]) and counts them
 * toward the synced daily limit.
 */
object ReminderBridge {
    const val CHANNEL = "kak_nur"
    private const val PREFS = "kabar_reminders"
    private const val KEY = "schedule"
    private const val DELIVERED = "delivered"
    /** Alarms are request codes 0 until this; Rust never sends more. */
    const val MAX = 8
    private const val PERMISSION_REQUEST = 4127

    /**
     * `[{"at": unix ms, "date": "YYYY-MM-DD", "time": "HH:MM", "title", "text"}]`,
     * soonest first.
     */
    @JvmStatic
    fun schedule(context: Context, json: String) {
        val app = context.applicationContext
        app.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY, json).apply()
        arm(app, json)
    }

    /** Re-arms the stored schedule (after a reboot, an update or a clock change). */
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
            val at = wallClock(item.optString("date"), item.optString("time")) ?: item.getLong("at")
            if (at <= now) continue
            val intent = pending(context, i, item.getString("title"), item.getString("text"), item.getLong("at"))
            // Inexact while idle: Doze may hold it a few minutes, which suits a nudge.
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, intent)
        }
    }

    /** The local date and time in the device's zone now, as unix ms. */
    private fun wallClock(date: String, time: String): Long? = try {
        LocalDate.parse(date).atTime(LocalTime.parse(time))
            .atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
    } catch (e: Exception) {
        null
    }

    private fun pending(context: Context, code: Int, title: String?, text: String?, at: Long = 0): PendingIntent {
        val intent = Intent(context, ReminderReceiver::class.java).setAction("app.kabar.REMIND")
        title?.let { intent.putExtra("title", it) }
        text?.let { intent.putExtra("text", it) }
        intent.putExtra("at", at)
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

    /** Remembers that the reminder planned for `at` was shown. */
    fun markDelivered(context: Context, at: Long) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val list = try { JSONArray(prefs.getString(DELIVERED, "[]")) } catch (e: org.json.JSONException) { JSONArray() }
        list.put(at)
        prefs.edit().putString(DELIVERED, list.toString()).apply()
    }

    /** The reminders shown since the last call (`[unix ms]`), then forgets them. */
    @JvmStatic
    fun takeDelivered(context: Context): String {
        val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val list = prefs.getString(DELIVERED, "[]") ?: "[]"
        prefs.edit().remove(DELIVERED).apply()
        return list
    }

    /**
     * Whether Android lets Kabar run its alarms freely. Samsung's One UI puts
     * apps unused for a few days to sleep, and their alarms stop.
     */
    @JvmStatic
    fun batteryUnrestricted(context: Context): Boolean {
        val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        return pm.isIgnoringBatteryOptimizations(context.packageName)
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

/** Posts a scheduled reminder, and re-arms the schedule after a reboot, an update or a clock change. */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED,
            Intent.ACTION_TIMEZONE_CHANGED, Intent.ACTION_TIME_CHANGED -> ReminderBridge.rearm(context)
            else -> {
                val title = intent.getStringExtra("title") ?: return
                val text = intent.getStringExtra("text") ?: return
                val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                if (nm.areNotificationsEnabled()) {
                    ReminderBridge.show(context, title, text)
                    ReminderBridge.markDelivered(context, intent.getLongExtra("at", 0))
                }
            }
        }
    }
}
