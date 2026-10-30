package android.app

import android.content.Context
import android.content.PreferenceStore
import android.content.Intent
import android.content.pm.PackageManager
import android.view.FrameLayout
import android.view.Window

open class Activity(val preferenceStore: PreferenceStore = PreferenceStore()) : Context(preferenceStore) {
    val window = Window(FrameLayout())
    val packageManager = PackageManager()
    var isFinishing: Boolean = false
    var isDestroyed: Boolean = false
    val startedActivities = mutableListOf<Intent>()

    fun runOnUiThread(action: Runnable) = action.run()
    fun startActivity(intent: Intent) { startedActivities.add(intent) }
}

class ActivityManager {
    var history: List<ApplicationExitInfo> = emptyList()
    var failQuery: Boolean = false
    val queries = mutableListOf<Triple<String, Int, Int>>()

    fun getHistoricalProcessExitReasons(packageName: String, pid: Int, maxNum: Int): List<ApplicationExitInfo> {
        queries.add(Triple(packageName, pid, maxNum))
        if (failQuery) throw SecurityException("exit history unavailable")
        return history
    }

    class RunningAppProcessInfo {
        companion object { const val IMPORTANCE_FOREGROUND = 100 }
    }
}

class ApplicationExitInfo(
    val timestamp: Long,
    val reason: Int,
    val importance: Int,
    val description: String? = null,
) {
    companion object {
        const val REASON_EXIT_SELF = 1
        const val REASON_SIGNALED = 2
        const val REASON_LOW_MEMORY = 3
        const val REASON_CRASH = 4
        const val REASON_CRASH_NATIVE = 5
        const val REASON_ANR = 6
        const val REASON_INITIALIZATION_FAILURE = 7
        const val REASON_PERMISSION_CHANGE = 8
        const val REASON_EXCESSIVE_RESOURCE_USAGE = 9
        const val REASON_USER_REQUESTED = 10
        const val REASON_USER_STOPPED = 11
        const val REASON_DEPENDENCY_DIED = 12
        const val REASON_OTHER = 13
    }
}
