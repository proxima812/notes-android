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
    fun schedule(
        context: Context,
        occurrenceId: String,
        requestCode: Int,
        triggerAtMillis: Long,
        title: String,
        body: String,
        exact: Boolean,
    ): Boolean {
        val manager = context.getSystemService(AlarmManager::class.java)
            ?: throw IllegalStateException("AlarmManager недоступен")

        val pending = pendingIntent(
            context = context,
            occurrenceId = occurrenceId,
            requestCode = requestCode,
            title = title,
            body = body,
            // An alarm being replaced must pick up the new title and time.
            flags = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val armedExact = exact && canScheduleExact(manager)
        if (armedExact) {
            manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAtMillis, pending)
        } else {
            // Doze may delay this by minutes. That is the documented price of
            // not holding the exact-alarm permission, and the caller is told.
            manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAtMillis, pending)
        }
        return armedExact
    }

    fun cancel(context: Context, requestCode: Int) {
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
        occurrenceId: String,
        requestCode: Int,
        title: String,
        body: String,
        flags: Int,
    ): PendingIntent {
        val intent = Intent(context, ReminderReceiver::class.java).apply {
            action = ReminderIntents.ACTION_FIRE
            putExtra(ReminderIntents.EXTRA_OCCURRENCE_ID, occurrenceId)
            putExtra(ReminderIntents.EXTRA_REQUEST_CODE, requestCode)
            putExtra(ReminderIntents.EXTRA_TITLE, title)
            putExtra(ReminderIntents.EXTRA_BODY, body)
        }
        return PendingIntent.getBroadcast(context, requestCode, intent, flags)
    }
}
