package dev.local.organizer.reminders

import android.content.Context
import java.security.SecureRandom

/**
 * The labels of the user's trimmed sounds.
 *
 * The files themselves live in `filesDir/reminder_sounds`; this remembers what
 * to call each one, because a generated file name like `snd_3f9a2c1d.m4a` says
 * nothing to the person who picked "Утренний дождь.mp3".
 *
 * The storage is behind [KeyValueStore] so the logic is testable without
 * Android: in production it is `SharedPreferences`, in tests a map.
 */
internal class CustomSoundStore(private val store: KeyValueStore) {

    /** File name → label, one entry per kept sound, sorted by label. */
    fun all(): List<Entry> =
        store.all()
            .map { (fileName, label) -> Entry(fileName, label) }
            .sortedBy { it.label.lowercase() }

    fun put(fileName: String, label: String) {
        store.put(fileName, label)
    }

    fun label(fileName: String): String? = store.all()[fileName]

    fun remove(fileName: String) {
        store.remove(fileName)
    }

    data class Entry(val fileName: String, val label: String)

    interface KeyValueStore {
        fun all(): Map<String, String>
        fun put(key: String, value: String)
        fun remove(key: String)
    }

    companion object {
        private const val PREFS_NAME = "reminder_custom_sounds_v1"
        private val random = SecureRandom()

        fun open(context: Context): CustomSoundStore {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            return CustomSoundStore(object : KeyValueStore {
                override fun all(): Map<String, String> =
                    prefs.all.mapNotNull { (key, value) ->
                        (value as? String)?.let { key to it }
                    }.toMap()

                override fun put(key: String, value: String) {
                    prefs.edit().putString(key, value).apply()
                }

                override fun remove(key: String) {
                    prefs.edit().remove(key).apply()
                }
            })
        }

        /**
         * The label a picked file gets: its display name without the extension.
         * A file with no readable name still needs to be called something.
         */
        fun labelFrom(displayName: String?): String {
            val name = displayName?.trim().orEmpty()
            if (name.isEmpty()) {
                return "Свой звук"
            }
            val dot = name.lastIndexOf('.')
            return if (dot > 0) name.substring(0, dot) else name
        }

        /** A fresh file name that cannot collide with a previous pick. */
        fun newFileName(): String {
            val suffix = ByteArray(4).also { random.nextBytes(it) }
                .joinToString("") { "%02x".format(it) }
            return "snd_$suffix.m4a"
        }
    }
}
