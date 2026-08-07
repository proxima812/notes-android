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

    /**
     * The URI the system will play for a sound id, whatever kind it is.
     *
     * The same resolution serves the channel and the in-app preview, so what
     * the user hears when trying a sound is what the notification plays.
     */
    fun soundUri(context: Context, soundId: String): Uri =
        when (val source = SoundIds.parse(soundId)) {
            is SoundSource.System -> Uri.parse(source.uri)
            is SoundSource.Custom -> ReminderSoundProvider.uriFor(source.fileName)
            is SoundSource.Preset -> {
                val resourceId = context.resources
                    .getIdentifier(source.name, "raw", context.packageName)
                require(resourceId != 0) { "звук ${source.name} не найден" }
                Uri.parse("android.resource://${context.packageName}/$resourceId")
            }
        }

    fun ensureChannel(
        context: Context,
        soundId: String,
        soundLabel: String,
        vibrate: Boolean,
    ): String {
        val uri = soundUri(context, soundId)

        val channelId = SoundIds.channelId(soundId)
        val manager = context.getSystemService(NotificationManager::class.java)
            ?: error("NotificationManager недоступен")
        if (manager.getNotificationChannel(channelId) != null) {
            return channelId
        }

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
