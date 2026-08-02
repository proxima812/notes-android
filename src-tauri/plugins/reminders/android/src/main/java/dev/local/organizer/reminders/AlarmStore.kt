package dev.local.organizer.reminders

import android.content.Context
import android.util.Log
import org.json.JSONException
import org.json.JSONObject

/** Everything needed to arm one alarm and to build the notification it fires. */
internal data class ArmedAlarm(
    val occurrenceId: String,
    val noteId: String,
    val requestCode: Int,
    val triggerAtMillis: Long,
    val title: String,
    val body: String,
    val soundId: String,
    val soundLabel: String,
    val vibrate: Boolean,
    val exact: Boolean,
) {
    fun toJson(): String = JSONObject().apply {
        put(KEY_OCCURRENCE_ID, occurrenceId)
        put(KEY_NOTE_ID, noteId)
        put(KEY_REQUEST_CODE, requestCode)
        put(KEY_TRIGGER_AT, triggerAtMillis)
        put(KEY_TITLE, title)
        put(KEY_BODY, body)
        put(KEY_SOUND_ID, soundId)
        put(KEY_SOUND_LABEL, soundLabel)
        put(KEY_VIBRATE, vibrate)
        put(KEY_EXACT, exact)
    }.toString()

    internal companion object {
        private const val KEY_OCCURRENCE_ID = "occurrence_id"
        private const val KEY_NOTE_ID = "note_id"
        private const val KEY_REQUEST_CODE = "request_code"
        private const val KEY_TRIGGER_AT = "trigger_at"
        private const val KEY_TITLE = "title"
        private const val KEY_BODY = "body"
        private const val KEY_SOUND_ID = "sound_id"
        private const val KEY_SOUND_LABEL = "sound_label"
        private const val KEY_VIBRATE = "vibrate"
        private const val KEY_EXACT = "exact"

        fun fromJson(raw: String): ArmedAlarm = JSONObject(raw).let { json ->
            ArmedAlarm(
                occurrenceId = json.getString(KEY_OCCURRENCE_ID),
                noteId = json.optString(KEY_NOTE_ID),
                requestCode = json.getInt(KEY_REQUEST_CODE),
                triggerAtMillis = json.getLong(KEY_TRIGGER_AT),
                title = json.optString(KEY_TITLE),
                body = json.optString(KEY_BODY),
                soundId = json.getString(KEY_SOUND_ID),
                soundLabel = json.optString(KEY_SOUND_LABEL),
                vibrate = json.optBoolean(KEY_VIBRATE, true),
                exact = json.optBoolean(KEY_EXACT, true),
            )
        }
    }
}

/**
 * A record of the alarms this app currently has armed with `AlarmManager`.
 *
 * Android drops every `PendingIntent` on reboot and on package replacement, and
 * the app process is not running to notice. The core's SQLite rows survive, but
 * nothing re-arms them until the user next opens the app — by which time the
 * reminder they set for 09:00 has silently not happened.
 *
 * This is not a second source of truth about reminders: it is Kotlin remembering
 * what Kotlin itself told the OS, so [BootReceiver] can say it again without
 * reading the database or knowing a single rule about when things fire.
 */
internal object AlarmStore {
    private const val PREFS = "reminder_alarms_v1"
    private const val TAG = "Organizer"

    fun remember(context: Context, alarm: ArmedAlarm) {
        prefs(context).edit()
            .putString(alarm.requestCode.toString(), alarm.toJson())
            .apply()
    }

    fun forget(context: Context, requestCode: Int) {
        prefs(context).edit()
            .remove(requestCode.toString())
            .apply()
    }

    /**
     * Every alarm believed to be armed, worst records skipped rather than fatal.
     *
     * A single unreadable entry — a downgrade, a half-written preference file —
     * must not cost the user the rest of their reminders, so it is dropped and
     * the others are still returned.
     */
    fun all(context: Context): List<ArmedAlarm> =
        prefs(context).all.entries.mapNotNull { entry ->
            val raw = entry.value as? String ?: return@mapNotNull null
            try {
                ArmedAlarm.fromJson(raw)
            } catch (malformed: JSONException) {
                Log.w(TAG, "повреждённая запись будильника ${entry.key}, пропущена", malformed)
                null
            }
        }

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
