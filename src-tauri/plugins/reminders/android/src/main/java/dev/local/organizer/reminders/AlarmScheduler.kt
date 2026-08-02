package dev.local.organizer.reminders

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build

/**
 * Arms and cancels alarms.
 *
 * `RTC_WAKEUP` is used rather than elapsed time because a reminder means a wall
 * clock instant: it must survive the device sleeping and must move with the
 * clock, not with uptime.
 */
internal object AlarmScheduler {

    /** Returns whether the alarm was actually armed as an exact one. */
    fun schedule(context: Context, alarm: ArmedAlarm): Boolean {
        val manager = context.getSystemService(AlarmManager::class.java)
            ?: throw IllegalStateException("AlarmManager недоступен")

        // Creating the channel here rather than at the call site means a reboot
        // replaying an alarm gets the same sound as the original, without
        // [BootReceiver] having to know that channels are how sound is chosen.
        val channelId = ReminderNotifications.ensureChannel(
            context = context,
            soundId = alarm.soundId,
            soundLabel = alarm.soundLabel,
            vibrate = alarm.vibrate,
        )

        val pending = pendingIntent(
            context = context,
            alarm = alarm,
            channelId = channelId,
            // An alarm being replaced must pick up the new title and time.
            flags = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val armedExact = alarm.exact && canScheduleExact(manager)
        if (armedExact) {
            manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, alarm.triggerAtMillis, pending)
        } else {
            // Doze may delay this by minutes. That is the documented price of
            // not holding the exact-alarm permission, and the caller is told.
            manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, alarm.triggerAtMillis, pending)
        }

        // Written only once the OS has accepted the alarm, so the journal never
        // claims an alarm that was never armed.
        AlarmStore.remember(context, alarm)
        return armedExact
    }

    fun cancel(context: Context, requestCode: Int) {
        AlarmStore.forget(context, requestCode)

        val manager = context.getSystemService(AlarmManager::class.java) ?: return

        // NO_CREATE returns null when nothing is armed, which is why cancelling
        // an alarm that never existed is not an error.
        val pending = PendingIntent.getBroadcast(
            context,
            requestCode,
            Intent(context, ReminderReceiver::class.java).setAction(ReminderIntents.ACTION_FIRE),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_NO_CREATE,
        ) ?: return

        manager.cancel(pending)
        pending.cancel()
    }

    fun canScheduleExact(manager: AlarmManager): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            manager.canScheduleExactAlarms()
        } else {
            true
        }

    private fun pendingIntent(
        context: Context,
        alarm: ArmedAlarm,
        channelId: String,
        flags: Int,
    ): PendingIntent {
        val intent = Intent(context, ReminderReceiver::class.java).apply {
            action = ReminderIntents.ACTION_FIRE
            putExtra(ReminderIntents.EXTRA_OCCURRENCE_ID, alarm.occurrenceId)
            putExtra(ReminderIntents.EXTRA_NOTE_ID, alarm.noteId)
            putExtra(ReminderIntents.EXTRA_REQUEST_CODE, alarm.requestCode)
            putExtra(ReminderIntents.EXTRA_SCHEDULED_AT, alarm.triggerAtMillis)
            putExtra(ReminderIntents.EXTRA_TITLE, alarm.title)
            putExtra(ReminderIntents.EXTRA_BODY, alarm.body)
            putExtra(ReminderIntents.EXTRA_CHANNEL_ID, channelId)
            putExtra(ReminderIntents.EXTRA_VIBRATE, alarm.vibrate)
        }
        return PendingIntent.getBroadcast(context, alarm.requestCode, intent, flags)
    }
}
