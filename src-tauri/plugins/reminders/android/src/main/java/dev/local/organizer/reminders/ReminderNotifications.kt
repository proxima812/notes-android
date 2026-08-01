package dev.local.organizer.reminders

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context

/**
 * The notification channel reminders are posted on.
 *
 * `minSdk` is 26, so channels always exist and there is no legacy path to keep
 * working. Importance is high because a reminder the user asked for at a
 * specific minute is worth an interruption.
 */
internal object ReminderNotifications {
    const val CHANNEL_ID = "reminders"

    fun ensureChannel(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) {
            return
        }

        val channel = NotificationChannel(
            CHANNEL_ID,
            "Напоминания",
            NotificationManager.IMPORTANCE_HIGH,
        ).apply {
            description = "Напоминания из заметок и задач"
            enableVibration(true)
            setShowBadge(true)
        }
        manager.createNotificationChannel(channel)
    }
}
