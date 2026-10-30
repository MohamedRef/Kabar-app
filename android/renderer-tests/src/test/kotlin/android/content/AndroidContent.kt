package android.content

// A second Activity using the same store simulates preferences across process starts.
class PreferenceStore {
    internal val values = mutableMapOf<String, MemoryPreferences>()
}

open class Context(private val store: PreferenceStore = PreferenceStore()) {
    companion object { const val MODE_PRIVATE = 0 }

    open val applicationContext: Context get() = this
    open val packageName: String get() = "app.kabar"
    private val services = mutableMapOf<Class<*>, Any>()

    fun getSharedPreferences(name: String, @Suppress("UNUSED_PARAMETER") mode: Int): SharedPreferences =
        store.values.getOrPut(name) { MemoryPreferences() }

    fun <T : Any> setService(type: Class<T>, service: T) { services[type] = service }

    @Suppress("UNCHECKED_CAST")
    fun <T : Any> getSystemService(type: Class<T>): T? = services[type] as? T
}

interface SharedPreferences {
    fun getString(key: String, defaultValue: String?): String?
    fun getLong(key: String, defaultValue: Long): Long
    fun edit(): Editor

    interface Editor {
        fun putString(key: String, value: String): Editor
        fun putLong(key: String, value: Long): Editor
        fun remove(key: String): Editor
        fun commit(): Boolean
        fun apply()
    }
}

class MemoryPreferences : SharedPreferences {
    private val data = mutableMapOf<String, Any>()
    var commits: Int = 0
        private set

    override fun getString(key: String, defaultValue: String?): String? = data[key] as? String ?: defaultValue
    override fun getLong(key: String, defaultValue: Long): Long = data[key] as? Long ?: defaultValue
    override fun edit(): SharedPreferences.Editor = Editor()

    private inner class Editor : SharedPreferences.Editor {
        private val writes = mutableMapOf<String, Any>()
        private val removals = mutableSetOf<String>()

        override fun putString(key: String, value: String) = apply { writes[key] = value; removals.remove(key) }
        override fun putLong(key: String, value: Long) = apply { writes[key] = value; removals.remove(key) }
        override fun remove(key: String) = apply { writes.remove(key); removals.add(key) }
        override fun commit(): Boolean { flush(); commits++; return true }
        override fun apply() = flush()
        private fun flush() { removals.forEach(data::remove); data.putAll(writes) }
    }
}

class Intent {
    var flags = 0
    fun addFlags(more: Int): Intent = apply { flags = flags or more }
    companion object {
        const val FLAG_ACTIVITY_NEW_TASK = 0x10000000
        const val FLAG_ACTIVITY_CLEAR_TASK = 0x00008000
    }
}
