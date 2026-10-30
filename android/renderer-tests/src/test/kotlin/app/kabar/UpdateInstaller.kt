package app.kabar

// Only used by the public production entry point, which these tests never call.
// An accidental call should fail instead of silently masking a real process kill.
object UpdateInstaller {
    fun inForeground(): Boolean = error("Production foreground probe must not run in JVM tests")
}
