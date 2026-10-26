package app.kabar

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaMetadata
import android.media.MediaPlayer
import android.media.PlaybackParams
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build
import android.os.IBinder

/**
 * Background playback for downloaded Sinetron and Kabar Radio episodes (PLAN §16):
 * one framework [MediaPlayer] in a foreground service with a [MediaSession], so an
 * episode keeps playing with the screen off and shows lock-screen / notification
 * controls. Framework classes only (no AndroidX), so the module needs no new
 * dependencies.
 *
 * Rust drives it through [PlaybackBridge] over JNI and polls the position to keep
 * the transcript in step; the service never calls back into Rust.
 */
class PlaybackService : Service() {
    private var session: MediaSession? = null
    private var focus: AudioFocusRequest? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val s = MediaSession(this, "Kabar")
        s.setCallback(object : MediaSession.Callback() {
            override fun onPlay() = PlaybackBridge.resume()
            override fun onPause() = PlaybackBridge.pause()
            override fun onStop() = PlaybackBridge.stop(this@PlaybackService)
            override fun onSeekTo(pos: Long) = PlaybackBridge.seek(pos / 1000.0)
        })
        s.isActive = true
        session = s
        PlaybackBridge.service = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_PAUSE -> PlaybackBridge.pause()
            ACTION_PLAY -> PlaybackBridge.resume()
            ACTION_STOP -> PlaybackBridge.stop(this)
            else -> requestFocus()
        }
        refresh()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        PlaybackBridge.service = null
        PlaybackBridge.release()
        abandonFocus()
        session?.release()
        session = null
        super.onDestroy()
    }

    /** Asks for audio focus; another app's call or music pauses the episode. */
    fun requestFocus(): Boolean {
        val am = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        val req = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(ATTRIBUTES)
            .setOnAudioFocusChangeListener { change ->
                if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
                    PlaybackBridge.pause()
                }
            }
            .build()
        focus = req
        return am.requestAudioFocus(req) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
    }

    private fun abandonFocus() {
        val am = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        focus?.let { am.abandonAudioFocusRequest(it) }
        focus = null
    }

    /** Updates the session state and the notification from the player. */
    fun refresh() {
        val s = session ?: return
        val playing = PlaybackBridge.isPlaying()
        val positionMs = (PlaybackBridge.position().coerceAtLeast(0.0) * 1000).toLong()
        s.setMetadata(
            MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, PlaybackBridge.title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, PlaybackBridge.show)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, PlaybackBridge.durationMs())
                .build(),
        )
        s.setPlaybackState(
            PlaybackState.Builder()
                .setActions(
                    PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
                        PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_STOP or
                        PlaybackState.ACTION_SEEK_TO,
                )
                .setState(
                    if (playing) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED,
                    positionMs,
                    if (playing) PlaybackBridge.rate else 0f,
                )
                .build(),
        )
        val notification = notification(s, playing)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun notification(s: MediaSession, playing: Boolean): Notification {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (nm.getNotificationChannel(CHANNEL) == null) {
            // Low importance: no sound, no pop-up; just the controls.
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Kabar audio", NotificationManager.IMPORTANCE_LOW))
        }
        val toggle = if (playing) {
            Notification.Action.Builder(android.R.drawable.ic_media_pause, "Pause", action(ACTION_PAUSE)).build()
        } else {
            Notification.Action.Builder(android.R.drawable.ic_media_play, "Play", action(ACTION_PLAY)).build()
        }
        val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(PlaybackBridge.title)
            .setContentText(PlaybackBridge.show)
            .setContentIntent(open)
            .setDeleteIntent(action(ACTION_STOP))
            .setOngoing(playing)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .addAction(toggle)
            .setStyle(Notification.MediaStyle().setMediaSession(s.sessionToken).setShowActionsInCompactView(0))
            .build()
    }

    private fun action(name: String): PendingIntent =
        PendingIntent.getService(
            this,
            name.hashCode(),
            Intent(this, PlaybackService::class.java).setAction(name),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    companion object {
        const val CHANNEL = "kabar.playback"
        const val NOTIFICATION_ID = 4512
        const val ACTION_PLAY = "app.kabar.PLAY"
        const val ACTION_PAUSE = "app.kabar.PAUSE"
        const val ACTION_STOP = "app.kabar.STOP"
        val ATTRIBUTES: AudioAttributes = AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_MEDIA)
            .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
            .build()
    }
}

/**
 * What Rust calls (static methods, any thread). Positions are seconds. All state
 * changes run on the player under a lock; the service mirrors them in the
 * notification.
 */
object PlaybackBridge {
    @Volatile var service: PlaybackService? = null
    @Volatile var title: String = ""
    @Volatile var show: String = "Kabar"
    @Volatile var rate: Float = 1f
    private var player: MediaPlayer? = null
    private var path: String? = null
    @Volatile private var ended = false
    private val lock = Any()

    /** Loads `path` (an MP3 on the device) and plays it from `at` seconds. */
    @JvmStatic
    fun play(context: Context, path: String, title: String, show: String, at: Double, rate: Float) {
        synchronized(lock) {
            this.title = title
            this.show = show
            this.rate = rate
            ended = false
            if (this.path != path || player == null) {
                release()
                val p = MediaPlayer()
                p.setAudioAttributes(PlaybackService.ATTRIBUTES)
                p.setDataSource(path)
                p.prepare()
                p.setOnCompletionListener {
                    ended = true
                    service?.refresh()
                }
                player = p
                this.path = path
            }
            val p = player ?: return
            p.seekTo((at * 1000).toInt())
            applyRate(p)
            p.start()
        }
        val app = context.applicationContext
        val intent = Intent(app, PlaybackService::class.java)
        if (service == null) {
            app.startForegroundService(intent)
        } else {
            service?.requestFocus()
            service?.refresh()
        }
    }

    @JvmStatic
    fun pause() {
        synchronized(lock) { player?.takeIf { it.isPlaying }?.pause() }
        service?.refresh()
    }

    @JvmStatic
    fun resume() {
        synchronized(lock) {
            val p = player ?: return
            ended = false
            applyRate(p)
            p.start()
        }
        service?.requestFocus()
        service?.refresh()
    }

    @JvmStatic
    fun seek(at: Double) {
        synchronized(lock) {
            val p = player ?: return
            ended = false
            p.seekTo((at * 1000).toInt())
            if (!p.isPlaying) {
                applyRate(p)
                p.start()
            }
        }
        service?.refresh()
    }

    @JvmStatic
    fun setRate(r: Float) {
        synchronized(lock) {
            rate = r
            player?.takeIf { it.isPlaying }?.let { applyRate(it) }
        }
        service?.refresh()
    }

    /** Stops playback and the service (leaving the player, or swiping it away). */
    @JvmStatic
    fun stop(context: Context) {
        release()
        val app = context.applicationContext
        app.stopService(Intent(app, PlaybackService::class.java))
    }

    /** Seconds into the track; -1 when nothing is loaded. */
    @JvmStatic
    fun position(): Double = synchronized(lock) { player?.let { it.currentPosition / 1000.0 } ?: -1.0 }

    @JvmStatic
    fun isPlaying(): Boolean = synchronized(lock) { player?.isPlaying == true }

    /** True once the track has played to its end (until the next play or seek). */
    @JvmStatic
    fun hasEnded(): Boolean = ended

    fun durationMs(): Long = synchronized(lock) { player?.duration?.toLong() ?: 0L }

    fun release() {
        synchronized(lock) {
            player?.release()
            player = null
            path = null
        }
    }

    private fun applyRate(p: MediaPlayer) {
        // Setting params on a paused player starts it, so only call this before start().
        p.playbackParams = PlaybackParams().setSpeed(rate)
    }
}
