package dev.local.organizer.reminders

/**
 * Where a sound id points to.
 *
 * The id format is a contract shared with the core and the frontend:
 * a bare name is a preset from `res/raw`, `system:<uri>` is a device
 * notification sound, `custom:<file>` is a user's trimmed file in
 * `filesDir/reminder_sounds`.
 */
internal sealed class SoundSource {
    /** A bundled sound: the id is the `res/raw` resource name. */
    data class Preset(val name: String) : SoundSource()

    /** A device notification sound: the remainder is a content URI. */
    data class System(val uri: String) : SoundSource()

    /** A user's trimmed file: the remainder is a file name, no path. */
    data class Custom(val fileName: String) : SoundSource()
}

internal object SoundIds {
    const val SYSTEM_PREFIX = "system:"
    const val CUSTOM_PREFIX = "custom:"

    private val PRESET_ID = Regex("^[a-z0-9_]+$")
    private val UNSAFE = Regex("[^a-z0-9_]")

    fun parse(id: String): SoundSource = when {
        id.startsWith(SYSTEM_PREFIX) -> SoundSource.System(id.removePrefix(SYSTEM_PREFIX))
        id.startsWith(CUSTOM_PREFIX) -> SoundSource.Custom(id.removePrefix(CUSTOM_PREFIX))
        else -> {
            require(PRESET_ID.matches(id)) { "некорректный ID звука" }
            SoundSource.Preset(id)
        }
    }

    /**
     * The notification channel a sound posts on.
     *
     * A channel's sound cannot be changed once created, so every sound gets a
     * channel of its own, named after the id. Presets keep the exact historic
     * formula — the channels already exist on users' devices under those ids.
     * Prefixed ids contain characters a channel id should not, so they are
     * flattened to a safe alphabet with the hash keeping distinct ids distinct.
     */
    fun channelId(soundId: String): String = when (val source = parse(soundId)) {
        is SoundSource.Preset -> "reminders_sound_${source.name}_v1"
        else -> {
            val sanitized = UNSAFE.replace(soundId.lowercase(), "_")
            "reminders_sound_x${sanitized}_${Math.abs(soundId.hashCode())}_v1"
        }
    }
}
