package app.kabar

import android.app.Activity
import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.view.View
import android.view.WindowInsetsController

/**
 * Makes the status and navigation bars match the app's theme (Settings ›
 * Look): their colour, and light or dark icons on it. With True black the
 * bars are black too, so the whole OLED screen stays off around the content.
 *
 * Rust passes a day and a night colour; a fixed theme sends the same one
 * twice, and System themes pick by the phone's own dark mode here. Where the
 * app draws edge to edge (Android 15), the bar colours are ignored and only
 * the icon colours matter.
 */
object SystemBars {
    @JvmStatic
    fun apply(activity: Activity, dayArgb: Int, nightArgb: Int) {
        activity.runOnUiThread {
            val night = (activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
                Configuration.UI_MODE_NIGHT_YES
            val colour = if (night) nightArgb else dayArgb
            val lightBars = isLight(colour)
            val window = activity.window
            @Suppress("DEPRECATION")
            // 35 = Android 15 (VANILLA_ICE_CREAM; the compile SDK may predate the name).
            if (Build.VERSION.SDK_INT < 35) {
                window.statusBarColor = colour
                window.navigationBarColor = colour
            }
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

    /** Light background: wants dark icons. */
    private fun isLight(argb: Int): Boolean {
        val lum = 0.2126 * Color.red(argb) + 0.7152 * Color.green(argb) + 0.0722 * Color.blue(argb)
        return lum > 140
    }
}
