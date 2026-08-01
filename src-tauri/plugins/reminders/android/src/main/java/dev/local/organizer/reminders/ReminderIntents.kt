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

    const val EXTRA_OCCURRENCE_ID = "occurrence_id"
    const val EXTRA_REQUEST_CODE = "request_code"
    const val EXTRA_TITLE = "title"
    const val EXTRA_BODY = "body"
    const val EXTRA_CHANNEL_ID = "channel_id"
    const val EXTRA_VIBRATE = "vibrate"
}
