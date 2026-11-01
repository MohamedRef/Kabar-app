package app.kabar

import android.app.Activity
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.webkit.WebView
import java.lang.reflect.InvocationHandler
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Method
import java.lang.reflect.Proxy

/**
 * Routes every Android Back press through Kabar's page first ([BackKeys]),
 * so Back always steps back one sheet or screen and leaves Kabar only from
 * Today with nothing open.
 *
 * The window's key callback sees Back before the WebView and before
 * wry's WryActivity.onKeyDown (which trusts WebView history). It is
 * wrapped with a [Proxy], so every other callback goes through unchanged
 * whatever the Android version adds to [Window.Callback].
 */
object BackKey {
    private const val TAG = "KabarBack"
    private const val TRIES = 40
    private const val RETRY_MS = 250L
    private const val SCRIPT =
        "window.__kabarNativeBack ? window.__kabarNativeBack() : null"

    @JvmStatic
    fun install(activity: Activity) {
        activity.runOnUiThread { attach(activity, 0) }
    }

    private fun attach(activity: Activity, tries: Int) {
        if (activity.isFinishing || activity.isDestroyed) return
        val window = activity.window
        val web = find(window.decorView)
        val inner = window.callback
        if (web == null || inner == null) {
            if (tries < TRIES) {
                Handler(Looper.getMainLooper()).postDelayed({ attach(activity, tries + 1) }, RETRY_MS)
            } else {
                Log.w(TAG, "No WebView for Back")
            }
            return
        }
        if (Proxy.isProxyClass(inner.javaClass) && Proxy.getInvocationHandler(inner) is Routed) return
        val keys = BackKeys<KeyEvent>(
            ask = { answer -> web.evaluateJavascript(SCRIPT) { answer(it ?: "null") } },
            replay = { down, up ->
                inner.dispatchKeyEvent(down)
                inner.dispatchKeyEvent(up)
            },
        )
        window.callback = Proxy.newProxyInstance(
            Window.Callback::class.java.classLoader,
            arrayOf(Window.Callback::class.java),
            Routed(inner, keys),
        ) as Window.Callback
    }

    private fun find(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) {
            for (i in 0 until view.childCount) find(view.getChildAt(i))?.let { return it }
        }
        return null
    }

    private class Routed(
        private val inner: Window.Callback,
        private val keys: BackKeys<KeyEvent>,
    ) : InvocationHandler {
        override fun invoke(proxy: Any, method: Method, args: Array<out Any?>?): Any? {
            if (method.name == "dispatchKeyEvent" && args?.size == 1) {
                val event = args[0] as? KeyEvent
                if (event != null && event.keyCode == KeyEvent.KEYCODE_BACK) {
                    val down = event.action == KeyEvent.ACTION_DOWN
                    if (down || event.action == KeyEvent.ACTION_UP) {
                        if (keys.onBack(event, down, event.repeatCount, event.isCanceled)) return true
                    }
                }
            }
            return try {
                if (args == null) method.invoke(inner) else method.invoke(inner, *args)
            } catch (e: InvocationTargetException) {
                throw e.targetException ?: e
            }
        }
    }
}
