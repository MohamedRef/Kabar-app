package app.batna

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build
import android.os.IBinder
import android.os.PowerManager

/**
 * Oral mode with the screen off, for walking: a foreground media service that
 * keeps the process (and Rust's think timer) running, holds a partial wake
 * lock, and shows the session on the lock screen with Again / Good buttons.
 *
 * Its MediaSession also takes headset buttons: play/pause is the main action
 * (show the answer, or Good once it is shown), next is Good, previous is Again.
 * The service never calls into Rust: buttons leave an action in [OralBridge],
 * which Rust polls.
 *
 * Adapted from kabar-app main: android/src/main/kotlin/app/kabar/PlaybackService.kt
 * (no MediaPlayer: the speech comes from TextToSpeech, driven from Rust).
 */
class OralService : Service() {
    private var session: MediaSession? = null
    private var wake: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val s = MediaSession(this, "Batna oral")
        s.setCallback(object : MediaSession.Callback() {
            override fun onPlay() = OralBridge.post(OralBridge.MAIN)
            override fun onPause() = OralBridge.post(OralBridge.MAIN)
            override fun onSkipToNext() = OralBridge.post(OralBridge.GOOD)
            override fun onSkipToPrevious() = OralBridge.post(OralBridge.AGAIN)
            override fun onStop() = OralBridge.post(OralBridge.END)
        })
        s.isActive = true
        session = s
        val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
        wake = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "batna:oral").apply {
            // A safety net: a forgotten session never holds the CPU for more than an hour.
            acquire(60 * 60 * 1000L)
        }
        OralBridge.service = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_AGAIN -> OralBridge.post(OralBridge.AGAIN)
            ACTION_GOOD -> OralBridge.post(OralBridge.GOOD)
            ACTION_MAIN -> OralBridge.post(OralBridge.MAIN)
            ACTION_END -> OralBridge.post(OralBridge.END)
        }
        refresh()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        OralBridge.service = null
        wake?.takeIf { it.isHeld }?.release()
        wake = null
        session?.release()
        session = null
        super.onDestroy()
    }

    /** Mirrors [OralBridge]'s question and phase in the session and notification. */
    fun refresh() {
        val s = session ?: return
        s.setMetadata(
            MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, OralBridge.title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, OralBridge.text)
                .build(),
        )
        // "Playing" makes this the session that gets the headset's buttons.
        s.setPlaybackState(
            PlaybackState.Builder()
                .setActions(
                    PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
                        PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_SKIP_TO_NEXT or
                        PlaybackState.ACTION_SKIP_TO_PREVIOUS or PlaybackState.ACTION_STOP,
                )
                .setState(PlaybackState.STATE_PLAYING, PlaybackState.PLAYBACK_POSITION_UNKNOWN, 1f)
                .build(),
        )
        val notification = notification(s)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun notification(s: MediaSession): Notification {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (nm.getNotificationChannel(CHANNEL) == null) {
            // Low importance: no sound, no pop-up; just the controls.
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Batna oral", NotificationManager.IMPORTANCE_LOW))
        }
        val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
        val b = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle(OralBridge.title)
            .setContentText(OralBridge.text)
            .setContentIntent(open)
            .setOngoing(true)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
        if (OralBridge.revealed) {
            b.addAction(button(android.R.drawable.ic_media_previous, OralBridge.againLabel, ACTION_AGAIN))
            b.addAction(button(android.R.drawable.ic_media_next, OralBridge.goodLabel, ACTION_GOOD))
        } else {
            b.addAction(button(android.R.drawable.ic_media_play, OralBridge.showLabel, ACTION_MAIN))
        }
        b.addAction(button(android.R.drawable.ic_menu_close_clear_cancel, OralBridge.endLabel, ACTION_END))
        val compact = if (OralBridge.revealed) intArrayOf(0, 1) else intArrayOf(0)
        return b.setStyle(Notification.MediaStyle().setMediaSession(s.sessionToken).setShowActionsInCompactView(*compact))
            .build()
    }

    private fun button(icon: Int, label: String, name: String): Notification.Action =
        Notification.Action.Builder(
            icon,
            label,
            PendingIntent.getService(
                this,
                name.hashCode(),
                Intent(this, OralService::class.java).setAction(name),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            ),
        ).build()

    companion object {
        const val CHANNEL = "batna.oral"
        const val NOTIFICATION_ID = 7207
        const val ACTION_AGAIN = "app.batna.oral.AGAIN"
        const val ACTION_GOOD = "app.batna.oral.GOOD"
        const val ACTION_MAIN = "app.batna.oral.MAIN"
        const val ACTION_END = "app.batna.oral.END"
    }
}

/** What Rust calls (static methods, any thread). */
object OralBridge {
    const val MAIN = 1
    const val GOOD = 2
    const val AGAIN = 3
    const val END = 4

    @Volatile var service: OralService? = null
    @Volatile var title: String = ""
    @Volatile var text: String = ""
    @Volatile var revealed: Boolean = false
    @Volatile var showLabel: String = "Show"
    @Volatile var againLabel: String = "Again"
    @Volatile var goodLabel: String = "Good"
    @Volatile var endLabel: String = "End"
    @Volatile private var pending: Int = 0

    /** Starts the service (from the foreground, when a session starts). */
    @JvmStatic
    fun start(context: Context, show: String, again: String, good: String, end: String) {
        showLabel = show
        againLabel = again
        goodLabel = good
        endLabel = end
        pending = 0
        val app = context.applicationContext
        if (service == null) {
            app.startForegroundService(Intent(app, OralService::class.java))
        } else {
            service?.refresh()
        }
    }

    /** The current question for the lock screen; `revealed` swaps the buttons. */
    @JvmStatic
    fun update(title: String, text: String, revealed: Boolean) {
        this.title = title
        this.text = text
        this.revealed = revealed
        service?.refresh()
    }

    @JvmStatic
    fun stop(context: Context) {
        pending = 0
        val app = context.applicationContext
        app.stopService(Intent(app, OralService::class.java))
    }

    /** The last button pressed since the previous call (0 = none). */
    @JvmStatic
    fun take(): Int {
        val p = pending
        pending = 0
        return p
    }

    fun post(action: Int) {
        pending = action
    }
}
