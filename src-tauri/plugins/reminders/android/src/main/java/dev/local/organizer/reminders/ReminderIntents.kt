package dev.local.organizer.reminders

/**
 * Keys carried by the alarm intent.
 *
 * The notification is built from these rather than from a database read: the
 * receiver can run with the app process dead, and opening SQLite from a
 * broadcast would put business logic on the Kotlin side.
 */
internal object ReminderIntents {
    const val ACTION_FIRE = "dev.local.organizer.reminders.FIRE"
    /** Sent by the notification's own button, not by an alarm. */
    const val ACTION_SNOOZE = "dev.local.organizer.reminders.SNOOZE"
    /** Carried by the tap intent that reopens the app on a note. */
    const val ACTION_OPEN = "dev.local.organizer.reminders.OPEN"

    const val EXTRA_OCCURRENCE_ID = "occurrence_id"
    /** Note to open when the notification is tapped. */
    const val EXTRA_NOTE_ID = "note_id"
    const val EXTRA_REQUEST_CODE = "request_code"
    const val EXTRA_TITLE = "title"
    const val EXTRA_BODY = "body"
    const val EXTRA_CHANNEL_ID = "channel_id"
    const val EXTRA_VIBRATE = "vibrate"

    /** How long "later" means, decided by the core and carried along. */
    const val EXTRA_SNOOZE_MINUTES = "snooze_minutes"

    /** Sound of the reminder, so a snoozed one comes back the same way. */
    const val EXTRA_SOUND_ID = "sound_id"
    const val EXTRA_SOUND_LABEL = "sound_label"
    const val EXTRA_EXACT = "exact"

    /**
     * Instant the reminder was set for.
     *
     * Shown instead of the moment the alarm arrived, so that an inexact alarm
     * delayed by Doze still reads as the time the user chose.
     */
    const val EXTRA_SCHEDULED_AT = "scheduled_at"

    /** Bundles reminders together once several are in the shade at once. */
    const val NOTIFICATION_GROUP = "dev.local.organizer.reminders"
}
