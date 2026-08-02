package dev.local.organizer.reminders

import android.media.AudioAttributes
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.net.Uri

/**
 * The notification channel reminders are posted on.
 *
 * `minSdk` is 26, so channels always exist and there is no legacy path to keep
 * working. Importance is high because a reminder the user asked for at a
 * specific minute is worth an interruption.
 */
internal object ReminderNotifications {
    private val SOUND_ID = Regex("^[a-z0-9_]+$")

    fun ensureChannel(
        context: Context,
        soundId: String,
        soundLabel: String,
        vibrate: Boolean,
    ): String {
        require(SOUND_ID.matches(soundId)) { "некорректный ID звука" }
        val resourceId = context.resources.getIdentifier(soundId, "raw", context.packageName)
        require(resourceId != 0) { "звук $soundId не найден" }

        val channelId = "reminders_sound_${soundId}_v1"
        val manager = context.getSystemService(NotificationManager::class.java)
            ?: error("NotificationManager недоступен")
        if (manager.getNotificationChannel(channelId) != null) {
            return channelId
        }

        val uri = Uri.parse("android.resource://${context.packageName}/$resourceId")
        val attributes = AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_ALARM)
            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
            .build()
        val channel = NotificationChannel(
            channelId,
            "Напоминания — $soundLabel",
            NotificationManager.IMPORTANCE_HIGH,
        ).apply {
            description = "Напоминания из заметок и задач"
            setSound(uri, attributes)
            enableVibration(vibrate)
            setShowBadge(true)
        }
        manager.createNotificationChannel(channel)
        return channelId
    }
}
