package dev.local.organizer.reminders

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/**
 * Turns a fired alarm into a notification, and the notification's own button
 * into another alarm.
 *
 * This runs on the main thread of a possibly cold-started process, so it does
 * no I/O and makes no decisions — everything it needs already arrived in the
 * intent, including how long "later" means.
 */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val alarm = read(intent) ?: return
        val channelId = intent.getStringExtra(ReminderIntents.EXTRA_CHANNEL_ID)
        if (channelId == null) {
            Log.w(TAG, "получен будильник без канала уведомлений")
            return
        }

        when (intent.action) {
            ReminderIntents.ACTION_SNOOZE -> snooze(context, alarm)
            ReminderIntents.ACTION_FIRE -> fire(context, alarm, channelId)
            else -> Log.w(TAG, "неизвестное действие ${intent.action}")
        }
    }

    private fun fire(context: Context, alarm: ArmedAlarm, channelId: String) {
        // It has fired, so there is nothing left for a reboot to restore. Done
        // before the notification is posted: a reminder shown twice after a
        // restart is a worse bug than one that fails to post and is forgotten.
        AlarmStore.forget(context, alarm.requestCode)

        val notification = NotificationCompat.Builder(context, channelId)
            .setSmallIcon(R.drawable.ic_reminder)
            .setColor(ContextCompat.getColor(context, R.color.reminder_accent))
            .setContentTitle(alarm.title.ifEmpty { "Напоминание" })
            .setContentText(alarm.body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(alarm.body))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setAutoCancel(true)
            .setOnlyAlertOnce(false)
            .setVibrate(if (alarm.vibrate) longArrayOf(0, 250, 150, 250) else longArrayOf(0))
            // The time the user picked, not the time the alarm arrived: an
            // inexact alarm can land minutes late, and showing that lateness as
            // the reminder's own time reads as the app getting it wrong.
            .apply {
                if (alarm.triggerAtMillis > 0L) {
                    setWhen(alarm.triggerAtMillis).setShowWhen(true)
                }
            }
            // No summary notification is posted: from Android N the system
            // bundles four or more on its own, and a hand-rolled summary would
            // need an id that cannot collide with any request code.
            .setGroup(ReminderIntents.NOTIFICATION_GROUP)
            .setContentIntent(openIntent(context, alarm))
            .addAction(
                R.drawable.ic_reminder,
                context.getString(R.string.reminder_snooze),
                snoozeIntent(context, alarm, channelId),
            )
            .build()

        try {
            NotificationManagerCompat.from(context).notify(alarm.requestCode, notification)
        } catch (denied: SecurityException) {
            // Posting was revoked between arming and firing. Losing the
            // notification is bad, crashing the receiver is worse.
            Log.w(TAG, "уведомление не показано: нет разрешения", denied)
        }
    }

    /**
     * Puts the same reminder back a few minutes out.
     *
     * The instant is computed here rather than carried, because "later" is
     * relative to the moment the button was pressed and nothing knows that in
     * advance. How much later is still the core's decision, and travelled with
     * the alarm.
     */
    private fun snooze(context: Context, alarm: ArmedAlarm) {
        NotificationManagerCompat.from(context).cancel(alarm.requestCode)

        val later = System.currentTimeMillis() + alarm.snoozeMinutes * 60_000L
        try {
            AlarmScheduler.schedule(context, alarm.copy(triggerAtMillis = later))
        } catch (failure: Exception) {
            Log.w(TAG, "не удалось отложить напоминание ${alarm.requestCode}", failure)
        }
    }

    /**
     * The tap target: the app, opened on the note.
     *
     * The plugin module cannot name the app's activity, so the launcher intent
     * is asked for by package and only its component is reused. The action is
     * deliberately not `MAIN`: a plain launcher intent lets the system resume an
     * existing task without delivering anything, and then `onNewIntent` never
     * runs and the tap opens the library instead of the note.
     */
    private fun openIntent(context: Context, alarm: ArmedAlarm): PendingIntent? {
        val launch = context.packageManager
            .getLaunchIntentForPackage(context.packageName)
            ?.component
            ?.let { component ->
                Intent(ReminderIntents.ACTION_OPEN).apply {
                    setComponent(component)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
                    putExtra(ReminderIntents.EXTRA_OCCURRENCE_ID, alarm.occurrenceId)
                    if (alarm.noteId.isNotEmpty()) {
                        putExtra(ReminderIntents.EXTRA_NOTE_ID, alarm.noteId)
                    }
                }
            } ?: return null

        return PendingIntent.getActivity(
            context,
            alarm.requestCode,
            launch,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    private fun snoozeIntent(context: Context, alarm: ArmedAlarm, channelId: String): PendingIntent {
        val intent = Intent(context, ReminderReceiver::class.java).apply {
            action = ReminderIntents.ACTION_SNOOZE
            putExtra(ReminderIntents.EXTRA_OCCURRENCE_ID, alarm.occurrenceId)
            putExtra(ReminderIntents.EXTRA_NOTE_ID, alarm.noteId)
            putExtra(ReminderIntents.EXTRA_REQUEST_CODE, alarm.requestCode)
            putExtra(ReminderIntents.EXTRA_SCHEDULED_AT, alarm.triggerAtMillis)
            putExtra(ReminderIntents.EXTRA_TITLE, alarm.title)
            putExtra(ReminderIntents.EXTRA_BODY, alarm.body)
            putExtra(ReminderIntents.EXTRA_CHANNEL_ID, channelId)
            putExtra(ReminderIntents.EXTRA_VIBRATE, alarm.vibrate)
            putExtra(ReminderIntents.EXTRA_SNOOZE_MINUTES, alarm.snoozeMinutes)
            putExtra(ReminderIntents.EXTRA_SOUND_ID, alarm.soundId)
            putExtra(ReminderIntents.EXTRA_SOUND_LABEL, alarm.soundLabel)
            putExtra(ReminderIntents.EXTRA_EXACT, alarm.exact)
        }
        // A request code of its own, so pressing "later" cannot replace the
        // alarm's own pending intent.
        return PendingIntent.getBroadcast(
            context,
            alarm.requestCode xor SNOOZE_REQUEST_MASK,
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    /** Everything the notification needs, as it arrived. */
    private fun read(intent: Intent): ArmedAlarm? {
        val occurrenceId = intent.getStringExtra(ReminderIntents.EXTRA_OCCURRENCE_ID)
        if (occurrenceId == null) {
            Log.w(TAG, "получен будильник без идентификатора срабатывания")
            return null
        }
        val soundId = intent.getStringExtra(ReminderIntents.EXTRA_SOUND_ID)
        if (soundId == null) {
            Log.w(TAG, "получен будильник без звука")
            return null
        }

        return ArmedAlarm(
            occurrenceId = occurrenceId,
            noteId = intent.getStringExtra(ReminderIntents.EXTRA_NOTE_ID).orEmpty(),
            requestCode = intent.getIntExtra(
                ReminderIntents.EXTRA_REQUEST_CODE,
                occurrenceId.hashCode(),
            ),
            triggerAtMillis = intent.getLongExtra(ReminderIntents.EXTRA_SCHEDULED_AT, 0L),
            title = intent.getStringExtra(ReminderIntents.EXTRA_TITLE).orEmpty(),
            body = intent.getStringExtra(ReminderIntents.EXTRA_BODY).orEmpty(),
            soundId = soundId,
            soundLabel = intent.getStringExtra(ReminderIntents.EXTRA_SOUND_LABEL).orEmpty(),
            vibrate = intent.getBooleanExtra(ReminderIntents.EXTRA_VIBRATE, true),
            exact = intent.getBooleanExtra(ReminderIntents.EXTRA_EXACT, true),
            snoozeMinutes = intent.getIntExtra(
                ReminderIntents.EXTRA_SNOOZE_MINUTES,
                ArmedAlarm.DEFAULT_SNOOZE_MINUTES,
            ),
        )
    }

    private companion object {
        const val TAG = "Organizer"

        /** Keeps the snooze button's request code clear of the alarm's own. */
        const val SNOOZE_REQUEST_MASK = 0x4000_0000
    }
}
