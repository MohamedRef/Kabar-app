package android.os

object Build {
    object VERSION { var SDK_INT = 30 }
    object VERSION_CODES { const val R = 30 }
}

class Looper private constructor() {
    companion object {
        private val main = Looper()
        fun getMainLooper() = main
    }
}

// Single-threaded, virtual delayed-post queue. No sleeping or real Android looper.
object UiTasks {
    private val tasks = mutableListOf<Pair<Long, Runnable>>()
    val pending: Int get() = tasks.size
    fun clear() = tasks.clear()
    fun schedule(delayMillis: Long, action: Runnable) { tasks.add(delayMillis to action) }
    fun step(): Long {
        val (delay, action) = tasks.removeAt(0)
        action.run()
        return delay
    }
}

class Handler(@Suppress("UNUSED_PARAMETER") looper: Looper) {
    fun postDelayed(action: Runnable, delayMillis: Long): Boolean {
        UiTasks.schedule(delayMillis, action)
        return true
    }
}

object Process {
    fun myPid(): Int = 4242
    fun killProcess(pid: Int): Nothing = error("Tests must inject RendererRecovery; killProcess($pid) must never run")
}
