package app.kabar

import android.app.Activity
import android.content.res.Configuration
import android.content.res.Resources
import android.graphics.Color
import android.os.Build
import android.view.View
import android.view.Window
import android.view.WindowManager
import android.view.WindowInsetsController

/**
 * Makes the status and navigation bars match the app's theme (Settings ›
 * Look): their colour, and light or dark icons on it. With True black the
 * bars are black too, so the whole OLED screen stays off around the content.
 *
 * Rust passes a day and a night colour; a fixed theme sends the same one
 * twice, and System themes pick by the phone's own dark mode here. Where the
 * app draws edge to edge (target SDK 35 on Android 15), the bar colours are
 * ignored and only the icon colours matter.
 */
object SystemBars {
    @JvmStatic
    fun apply(activity: Activity, dayArgb: Int, nightArgb: Int) {
        activity.runOnUiThread {
            val night = isNight(activity)
            val colour = if (night) nightArgb else dayArgb
            val lightBars = isLight(colour)
            val window = activity.window
            // Always set: Android 15 ignores these only when the app is
            // edge to edge, which depends on the target SDK, not the device.
            // An app targeting SDK 34 still takes them on an Android 15 phone.
            paintBars(window, colour)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                val mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                    WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
                window.insetsController?.setSystemBarsAppearance(if (lightBars) mask else 0, mask)
            } else {
                @Suppress("DEPRECATION")
                var flags = window.decorView.systemUiVisibility
                @Suppress("DEPRECATION")
                val light = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
                flags = if (lightBars) flags or light else flags and light.inv()
                @Suppress("DEPRECATION")
                window.decorView.systemUiVisibility = flags
            }
        }
    }

    /**
     * The phone's own dark mode. Read from the system resources, which follow
     * Settings › Display even when this activity's theme is light (the
     * WebView then reports `prefers-color-scheme: light`).
     */
    @JvmStatic
    fun isNight(@Suppress("UNUSED_PARAMETER") activity: Activity): Boolean =
        (Resources.getSystem().configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES

    @Suppress("DEPRECATION")
    private fun paintBars(window: Window, colour: Int) {
        // Translucent bars ignore the colour; the window must draw them.
        window.clearFlags(WindowManager.LayoutParams.FLAG_TRANSLUCENT_STATUS or WindowManager.LayoutParams.FLAG_TRANSLUCENT_NAVIGATION)
        window.addFlags(WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS)
        window.statusBarColor = colour
        window.navigationBarColor = colour
    }

    /** Light background: wants dark icons. */
    private fun isLight(argb: Int): Boolean {
        val lum = 0.2126 * Color.red(argb) + 0.7152 * Color.green(argb) + 0.0722 * Color.blue(argb)
        return lum > 140
    }
}
