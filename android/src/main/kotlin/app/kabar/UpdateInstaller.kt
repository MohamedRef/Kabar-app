package app.kabar

import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.app.ActivityManager
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import java.io.File
import kotlin.concurrent.thread

/**
 * Settings › About › Updates on Android: installs a downloaded, already
 * hash-checked APK over the running Kabar with PackageInstaller.
 *
 * Before anything reaches the system it checks the APK the way Android will:
 * same package, signed by the same certificate as the installed Kabar, and a
 * versionCode no lower than this one. Android then asks the learner to
 * confirm, except on Android 12+ once Kabar installed itself before (it is
 * then the installer of record and may update itself quietly).
 *
 * Rust starts it through [install] and polls [state], like [ExportSave].
 */
object UpdateInstaller {
    const val IDLE = 0
    const val WORKING = 1
    const val WAITING_USER = 2
    const val FAILED = 3
    const val CANCELLED = 4

    private const val TAG = "KabarUpdate"
    internal const val ACTION_STATUS = "app.kabar.update.STATUS"
    private const val PREFS = "kabar_update"

    @Volatile private var current = IDLE
    @Volatile private var message: String? = null

    /** Whether the learner allowed Kabar to install apps ("Install unknown apps"). */
    @JvmStatic
    fun canRequestInstalls(activity: Activity): Boolean = activity.packageManager.canRequestPackageInstalls()

    /** On mobile data or with Data Saver on: automatic downloads wait for Wi-Fi. */
    @JvmStatic
    fun isMetered(activity: Activity): Boolean {
        val cm = activity.getSystemService(ConnectivityManager::class.java) ?: return true
        return cm.isActiveNetworkMetered ||
            cm.restrictBackgroundStatus == ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED
    }

    /**
     * Android's confirmation screen, kept when it arrived while Kabar was in
     * the background (Android 10+ blocks starting it from there). Shown on
     * the next resume, and offered as a notification meanwhile.
     */
    @Volatile internal var pendingConfirm: Intent? = null

    @JvmStatic
    fun showPendingConfirm(activity: Activity) {
        val confirm = pendingConfirm ?: return
        pendingConfirm = null
        activity.getSystemService(NotificationManager::class.java)?.cancel(CONFIRM_NOTIFICATION_ID)
        try {
            activity.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (e: Exception) {
            done(FAILED, e.message)
        }
    }

    internal fun inForeground(): Boolean {
        val info = ActivityManager.RunningAppProcessInfo()
        ActivityManager.getMyMemoryState(info)
        return info.importance <= ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND
    }

    /** "Tap to install the update": the confirmation screen, from the shade. */
    internal fun notifyConfirm(context: Context, confirm: Intent) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val title = prefs.getString("ready_title", null) ?: return
        val nm = context.getSystemService(NotificationManager::class.java) ?: return
        if (!nm.areNotificationsEnabled()) return
        ensureChannel(nm)
        val open = PendingIntent.getActivity(context, 1, confirm, PendingIntent.FLAG_IMMUTABLE)
        nm.notify(
            CONFIRM_NOTIFICATION_ID,
            Notification.Builder(context, CHANNEL)
                .setSmallIcon(app.kabar.bridge.R.drawable.ic_stat_kabar)
                .setContentTitle(title)
                .setContentIntent(open)
                .setAutoCancel(true)
                .build(),
        )
    }

    private fun ensureChannel(nm: NotificationManager) {
        if (nm.getNotificationChannel(CHANNEL) == null) {
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Kabar updates", NotificationManager.IMPORTANCE_LOW))
        }
    }

    /** Opens Kabar's "Install unknown apps" switch in Android Settings. */
    @JvmStatic
    fun openInstallPermission(activity: Activity) {
        activity.startActivity(
            Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + activity.packageName))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }

    /**
     * Whether an update can go in without a confirmation dialog: Android 12+
     * and Kabar installed the running version itself.
     */
    @JvmStatic
    fun canInstallQuietly(activity: Activity): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return false
        return try {
            activity.packageManager.getInstallSourceInfo(activity.packageName).installingPackageName ==
                activity.packageName
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Installs the APK at [path]. [versionCode] is the signed manifest's (0 to
     * skip that check). [doneTitle]/[doneBody] are the "Kabar was updated"
     * notification in the learner's language, shown once the new version is in;
     * [readyTitle] the "tap to install" one, if Android must ask while Kabar
     * is in the background.
     */
    @JvmStatic
    fun install(
        activity: Activity,
        path: String,
        versionCode: Long,
        doneTitle: String,
        doneBody: String,
        readyTitle: String,
    ) {
        current = WORKING
        message = null
        val context = activity.applicationContext
        thread(name = "kabar-update") {
            try {
                val file = File(path)
                check(context, file, versionCode)
                context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                    .putString("title", doneTitle).putString("body", doneBody).putString("ready_title", readyTitle).apply()
                commit(context, file)
            } catch (e: Exception) {
                Log.w(TAG, "Update not installed", e)
                done(FAILED, e.message ?: e.toString())
            }
        }
    }

    private fun check(context: Context, file: File, versionCode: Long) {
        require(file.isFile) { "the download is gone" }
        val pm = context.packageManager
        val flags = PackageManager.GET_SIGNING_CERTIFICATES
        val apk = pm.getPackageArchiveInfo(file.path, flags) ?: error("not an APK Android can read")
        require(apk.packageName == context.packageName) { "the APK is ${apk.packageName}, not Kabar" }
        val installed = pm.getPackageInfo(context.packageName, flags)
        require(signers(apk) == signers(installed) && signers(apk).isNotEmpty()) {
            "the APK isn't signed with this Kabar's key"
        }
        require(apk.longVersionCode >= installed.longVersionCode) {
            "the APK (${apk.longVersionCode}) is older than this Kabar (${installed.longVersionCode})"
        }
        require(versionCode <= 0 || apk.longVersionCode == versionCode) {
            "the APK's versionCode ${apk.longVersionCode} isn't the signed ${versionCode}"
        }
    }

    private fun signers(info: PackageInfo): Set<String> {
        val signing = info.signingInfo ?: return emptySet()
        val certs = if (signing.hasMultipleSigners()) signing.apkContentsSigners else signing.signingCertificateHistory
        return certs.orEmpty().map { it.toCharsString() }.toSet()
    }

    private fun commit(context: Context, file: File) {
        val installer = context.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(context.packageName)
            setSize(file.length())
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
            }
        }
        val id = installer.createSession(params)
        installer.openSession(id).use { session ->
            session.openWrite("kabar.apk", 0, file.length()).use { out ->
                file.inputStream().use { it.copyTo(out) }
                session.fsync(out)
            }
            val status = Intent(context, UpdateInstallReceiver::class.java).setAction(ACTION_STATUS)
            // Mutable: the installer adds the status extras to it.
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
            session.commit(PendingIntent.getBroadcast(context, id, status, flags).intentSender)
        }
    }

    /** IDLE, WORKING, WAITING_USER, FAILED or CANCELLED. */
    @JvmStatic
    fun state(): Int = current

    /** Why the last install failed (FAILED only). */
    @JvmStatic
    fun message(): String? = message

    @JvmStatic
    fun reset() {
        if (current != WORKING && current != WAITING_USER) {
            current = IDLE
            message = null
        }
    }

    internal fun done(state: Int, why: String?) {
        message = why
        current = state
    }

    /** "Kabar was updated", once the new version replaced the old one. */
    internal fun notifyUpdated(context: Context) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val title = prefs.getString("title", null) ?: return
        val body = prefs.getString("body", null) ?: ""
        prefs.edit().clear().apply()
        val nm = context.getSystemService(NotificationManager::class.java) ?: return
        nm.cancel(CONFIRM_NOTIFICATION_ID)
        if (!nm.areNotificationsEnabled()) return
        ensureChannel(nm)
        val launch = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return
        val open = PendingIntent.getActivity(context, 0, launch, PendingIntent.FLAG_IMMUTABLE)
        val notification =
            Notification.Builder(context, CHANNEL)
                .setSmallIcon(app.kabar.bridge.R.drawable.ic_stat_kabar)
                .setContentTitle(title)
                .setContentText(body)
                .setContentIntent(open)
                .setAutoCancel(true)
                .build()
        nm.notify(NOTIFICATION_ID, notification)
    }

    private const val CHANNEL = "kabar_updates"
    private const val NOTIFICATION_ID = 7301
    private const val CONFIRM_NOTIFICATION_ID = 7302
}

/** The installer's verdict, and the "Kabar was updated" moment after it. */
class UpdateInstallReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_MY_PACKAGE_REPLACED -> UpdateInstaller.notifyUpdated(context)
            UpdateInstaller.ACTION_STATUS -> status(context, intent)
        }
    }

    private fun status(context: Context, intent: Intent) {
        when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
                if (confirm == null) {
                    UpdateInstaller.done(UpdateInstaller.FAILED, "no confirmation screen")
                    return
                }
                UpdateInstaller.done(UpdateInstaller.WAITING_USER, null)
                if (UpdateInstaller.inForeground()) {
                    try {
                        context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                    } catch (e: Exception) {
                        UpdateInstaller.done(UpdateInstaller.FAILED, e.message)
                    }
                } else {
                    // Android 10+ won't open it from the background: keep it
                    // for the next resume and offer it in the shade.
                    UpdateInstaller.pendingConfirm = confirm
                    UpdateInstaller.notifyConfirm(context, confirm)
                }
            }
            // The new Kabar is in; this process ends now.
            PackageInstaller.STATUS_SUCCESS -> UpdateInstaller.done(UpdateInstaller.IDLE, null)
            PackageInstaller.STATUS_FAILURE_ABORTED -> UpdateInstaller.done(UpdateInstaller.CANCELLED, null)
            else -> {
                val why = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "status $status"
                UpdateInstaller.done(UpdateInstaller.FAILED, why)
            }
        }
    }
}
