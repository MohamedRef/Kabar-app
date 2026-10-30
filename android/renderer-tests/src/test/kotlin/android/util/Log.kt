package android.util

object Log {
    val warnings = mutableListOf<String>()
    fun w(tag: String, message: String): Int { warnings.add("$tag: $message"); return 0 }
    fun w(tag: String, message: String, error: Throwable): Int = w(tag, "$message: $error")
}
