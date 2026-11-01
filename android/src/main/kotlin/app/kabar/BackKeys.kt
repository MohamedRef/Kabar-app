package app.kabar

/**
 * Decides what one Android Back press does, without Android types so the
 * JVM tests can drive it ([BackKey] feeds it the real key events).
 *
 * WebView's own history can't be trusted for Back: it skips entries the
 * page added with no tap in between, and then wry lets Android close
 * Kabar from any screen. So every Back press goes to the page instead
 * (`window.__kabarNativeBack()` in system-back.js), which closes the top
 * sheet or steps back one screen and answers "step", or answers "exit"
 * on Today with nothing open. Only then does the press reach Android.
 */
internal class BackKeys<E>(
    /** Asks the page for one Back step; the answer arrives later, on the UI thread. */
    private val ask: ((String) -> Unit) -> Unit,
    /** Sends a held-back press on to Android's own Back handling. */
    private val replay: (down: E, up: E) -> Unit,
) {
    private var down: E? = null

    /** A Back key event; true when it is consumed here. */
    fun onBack(event: E, isDown: Boolean, repeat: Int, canceled: Boolean): Boolean {
        if (isDown) {
            if (repeat == 0) down = event
            return true
        }
        // An up we never saw go down (installed mid-press) is not ours.
        val pressed = down ?: return false
        down = null
        if (canceled) return true
        ask { answer ->
            // "null": the page script isn't running (yet); keep Android's Back.
            val word = answer.trim().trim('"')
            if (word != "step") replay(pressed, event)
        }
        return true
    }
}
