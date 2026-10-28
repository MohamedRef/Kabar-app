package app.kabar

import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.os.Bundle
import android.util.Log
import org.json.JSONObject
import java.time.LocalTime
import kotlin.concurrent.thread

/**
 * "A new Kabar is out": the system notification for a new version, and the
 * periodic background check that sends it while Kabar is closed.
 *
 * Rust words the notification (title, what's new, button) in the learner's
 * language and decides when one is due (`update::notice`: once per version
 * and step, never in quiet hours, never after the banner was closed). Kotlin
 * only shows it: from the running app through [show], and from
 * [UpdateCheckJob], which calls the same Rust code over JNI.
 *
 * A tap opens Kabar through [UpdateTapActivity], which leaves the action
 * (`open`, `download` or `install`) for Rust to take ([takeTap]) once the
 * window is in front: Settings › About opens on the Updates card, and the
 * button's action starts.
 */
object UpdateNotice {
    private const val TAG = "KabarUpdate"
    /** Its own channel, so the learner can tune it apart from install progress. */
    private const val CHANNEL = "kabar_new_version"
    private const val NOTIFICATION_ID = 7303
    private const val JOB_ID = 7304
    private const val PREFS = "kabar_update_notice"
    internal const val EXTRA_TAP = "app.kabar.update.TAP"

    /** About every four hours; Android picks the moment within the last hour. */
    private const val PERIOD_MS = 4L * 60 * 60 * 1000
    private const val FLEX_MS = 60L * 60 * 1000

    /** Whether notifications are allowed for Kabar and for this channel. */
    internal fun allowed(context: Context): Boolean {
        val nm = context.getSystemService(NotificationManager::class.java) ?: return false
        if (!nm.areNotificationsEnabled()) return false
        val channel = nm.getNotificationChannel(CHANNEL) ?: return true
        return channel.importance != NotificationManager.IMPORTANCE_NONE
    }

    /**
     * Shows the notice (`{"title", "body", "action", "button", "channel"}`)
     * unless notifications are off, or, with [onlyInBackground], Kabar is in
     * front (its banner says it there). True once shown.
     */
    @JvmStatic
    fun show(context: Context, json: String, onlyInBackground: Boolean): Boolean {
        if (!allowed(context)) return false
        if (onlyInBackground && UpdateInstaller.inForeground()) return false
        val app = context.applicationContext
        val nm = app.getSystemService(NotificationManager::class.java) ?: return false
        val n = try { JSONObject(json) } catch (e: org.json.JSONException) { return false }
        val title = n.optString("title").ifBlank { return false }
        val body = n.optString("body")
        nm.createNotificationChannel(
            NotificationChannel(
                CHANNEL,
                n.optString("channel").ifBlank { "New Kabar versions" },
                NotificationManager.IMPORTANCE_DEFAULT,
            ),
        )
        val builder = Notification.Builder(app, CHANNEL)
            .setSmallIcon(app.kabar.bridge.R.drawable.ic_stat_kabar)
            .setContentTitle(title)
            .setContentText(body.lineSequence().firstOrNull().orEmpty())
            .setStyle(Notification.BigTextStyle().bigText(body))
            .setContentIntent(tap(app, "open", 0))
            .setOnlyAlertOnce(true)
            .setAutoCancel(true)
        val action = n.optString("action")
        val button = n.optString("button")
        if (action.isNotBlank() && button.isNotBlank()) {
            builder.addAction(Notification.Action.Builder(null, button, tap(app, action, 1)).build())
        }
        nm.notify(NOTIFICATION_ID, builder.build())
        return true
    }

    private fun tap(context: Context, action: String, code: Int): PendingIntent {
        val intent = Intent(context, UpdateTapActivity::class.java)
            .putExtra(EXTRA_TAP, action)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        return PendingIntent.getActivity(
            context, code, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    /** Takes the notice out of the shade (closed in the app, or installed). */
    @JvmStatic
    fun clear(context: Context) {
        context.getSystemService(NotificationManager::class.java)?.cancel(NOTIFICATION_ID)
    }

    /**
     * Starts the background check (kept across reboots and updates), or with
     * [on] false stops it. Re-scheduling an existing job keeps its timing.
     */
    @JvmStatic
    fun scheduleChecks(context: Context, on: Boolean) {
        val app = context.applicationContext
        val jobs = app.getSystemService(JobScheduler::class.java) ?: return
        app.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putBoolean("checks", on).apply()
        if (!on) {
            jobs.cancel(JOB_ID)
            return
        }
        val pending = jobs.getPendingJob(JOB_ID)
        if (pending != null && pending.intervalMillis == PERIOD_MS) return
        val job = JobInfo.Builder(JOB_ID, ComponentName(app, UpdateCheckJob::class.java))
            .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
            .setPeriodic(PERIOD_MS, FLEX_MS)
            .setPersisted(true)
            .build()
        if (jobs.schedule(job) != JobScheduler.RESULT_SUCCESS) {
            Log.w(TAG, "Background update check not scheduled")
        }
    }

    /** After an update or a cleared job list: schedules again if it was on. */
    internal fun rescheduleIfOn(context: Context) {
        if (context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean("checks", false)) {
            scheduleChecks(context, true)
        }
    }

    internal fun putTap(context: Context, action: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString("tap", action).apply()
    }

    /** The action a tap left (`open`, `download`, `install`), then forgets it. */
    @JvmStatic
    fun takeTap(context: Context): String? {
        val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val tap = prefs.getString("tap", null) ?: return null
        prefs.edit().remove("tap").apply()
        return tap
    }
}

/**
 * The periodic background check: loads Kabar's native library (already
 * loaded when the app runs in this process) and asks Rust to look at the
 * update feed, download in automatic mode on an unmetered network, and say
 * whether a notification is due. Runs on its own thread; never more than
 * one at a time (JobScheduler).
 */
class UpdateCheckJob : JobService() {
    @Volatile private var worker: Thread? = null

    override fun onStartJob(params: JobParameters): Boolean {
        val app = applicationContext
        worker = thread(name = "kabar-update-check") {
            try {
                if (loadNative(app)) {
                    val metered = metered(app)
                    val canNotify = UpdateNotice.allowed(app) && !UpdateInstaller.inForeground()
                    val now = LocalTime.now()
                    val json = check(app.filesDir.absolutePath, metered, canNotify, now.hour * 60 + now.minute)
                    if (!json.isNullOrBlank()) UpdateNotice.show(app, json, false)
                }
            } catch (t: Throwable) {
                Log.w(TAG, "Background update check failed", t)
            } finally {
                jobFinished(params, false)
            }
        }
        return true
    }

    /** Stopped early (the network went, or time ran out): the next period tries again. */
    override fun onStopJob(params: JobParameters): Boolean = false

    private fun metered(context: Context): Boolean {
        val cm = context.getSystemService(ConnectivityManager::class.java) ?: return true
        return cm.isActiveNetworkMetered ||
            cm.restrictBackgroundStatus == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED
    }

    /** `update::background_check` in Rust (platform/android.rs). */
    private external fun check(dataDir: String, metered: Boolean, canNotify: Boolean, localMinute: Int): String?

    companion object {
        private const val TAG = "KabarUpdate"
        @Volatile private var loaded = false

        /**
         * The library dx builds, named in MainActivity's `android.app.lib_name`
         * (the same one the app loads; loading it again is a no-op).
         */
        @Synchronized
        private fun loadNative(context: Context): Boolean {
            if (loaded) return true
            val name = try {
                val activity = ComponentName(context.packageName, "dev.dioxus.main.MainActivity")
                context.packageManager.getActivityInfo(activity, PackageManager.GET_META_DATA)
                    .metaData?.getString("android.app.lib_name")
            } catch (e: Exception) {
                null
            } ?: "main"
            return try {
                System.loadLibrary(name)
                loaded = true
                true
            } catch (e: UnsatisfiedLinkError) {
                Log.w(TAG, "No native library '$name' for the update check", e)
                false
            }
        }
    }
}

/** A tap on the notice: leaves its action for the app, opens Kabar, and goes. Invisible. */
class UpdateTapActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        UpdateNotice.putTap(this, intent.getStringExtra(UpdateNotice.EXTRA_TAP) ?: "open")
        UpdateNotice.clear(this)
        packageManager.getLaunchIntentForPackage(packageName)
            ?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED)
            ?.let { startActivity(it) }
        finish()
    }
}
