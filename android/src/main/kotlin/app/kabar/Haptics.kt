package app.kabar

import android.content.Context
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager

/**
 * A light tap of vibration with lesson feedback (Settings › Learning ›
 * Vibration). Rust calls [play] from the UI thread; Vibrator is thread-safe
 * and returns at once. Devices without a motor (most Galaxy tablets) report
 * [available] false and Settings hides the switch there.
 */
object Haptics {
    const val CORRECT = 0
    const val MISSED = 1
    const val LESSON_END = 2

    private fun vibrator(context: Context): Vibrator? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
        }

    @JvmStatic
    fun available(context: Context): Boolean = vibrator(context)?.hasVibrator() == true

    @JvmStatic
    fun play(context: Context, kind: Int) {
        val v = vibrator(context) ?: return
        if (!v.hasVibrator()) return
        val effect = when (kind) {
            // A crisp click, like a key press.
            CORRECT -> predefined(VibrationEffect.EFFECT_CLICK, 18, 160)
            // Barely there: a nudge, never a buzz of disapproval.
            MISSED -> predefined(VibrationEffect.EFFECT_TICK, 12, 90)
            // Two soft taps and a fuller one, in time with the gong.
            LESSON_END -> VibrationEffect.createWaveform(
                longArrayOf(0, 25, 70, 25, 90, 60),
                intArrayOf(0, 110, 0, 140, 0, 220),
                -1,
            )
            else -> return
        }
        v.vibrate(effect)
    }

    /** Android 10+ tuned effects; a short one-shot on Android 9. */
    private fun predefined(id: Int, ms: Long, amplitude: Int): VibrationEffect =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            VibrationEffect.createPredefined(id)
        } else {
            VibrationEffect.createOneShot(ms, amplitude)
        }
}
