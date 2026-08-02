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
 * Turns a fired alarm into a notification.
 *
 * This runs on the main thread of a possibly cold-started process, so it does
 * no I/O and makes no decisions — everything it needs already arrived in the
 * intent.
 */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val occurrenceId = intent.getStringExtra(ReminderIntents.EXTRA_OCCURRENCE_ID)
        if (occurrenceId == null) {
            Log.w(TAG, "получен будильник без идентификатора срабатывания")
            return
        }

        val requestCode = intent.getIntExtra(ReminderIntents.EXTRA_REQUEST_CODE, occurrenceId.hashCode())
        val title = intent.getStringExtra(ReminderIntents.EXTRA_TITLE).orEmpty()
        val body = intent.getStringExtra(ReminderIntents.EXTRA_BODY).orEmpty()
        val channelId = intent.getStringExtra(ReminderIntents.EXTRA_CHANNEL_ID)
        if (channelId == null) {
            Log.w(TAG, "получен будильник без канала уведомлений")
            return
        }
        // It has fired, so there is nothing left for a reboot to restore. Done
        // before the notification is posted: a reminder shown twice after a
        // restart is a worse bug than one that fails to post and is forgotten.
        AlarmStore.forget(context, requestCode)

        val vibrate = intent.getBooleanExtra(ReminderIntents.EXTRA_VIBRATE, true)
        val noteId = intent.getStringExtra(ReminderIntents.EXTRA_NOTE_ID)
        val scheduledAt = intent.getLongExtra(ReminderIntents.EXTRA_SCHEDULED_AT, 0L)

        // The plugin module cannot name the app's activity, so the launcher
        // intent is asked for by package and only its component is reused. The
        // action is deliberately not `MAIN`: a plain launcher intent lets the
        // system resume an existing task without delivering anything, and then
        // `onNewIntent` never runs and the tap opens the library instead of the
        // note.
        val launch = context.packageManager
            .getLaunchIntentForPackage(context.packageName)
            ?.component
            ?.let { component ->
                Intent(ReminderIntents.ACTION_OPEN).apply {
                    setComponent(component)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
                    putExtra(ReminderIntents.EXTRA_OCCURRENCE_ID, occurrenceId)
                    if (noteId != null) {
                        putExtra(ReminderIntents.EXTRA_NOTE_ID, noteId)
                    }
                }
            }
        val contentIntent = launch?.let {
            PendingIntent.getActivity(
                context,
                requestCode,
                it,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        }

        val notification = NotificationCompat.Builder(context, channelId)
            .setSmallIcon(R.drawable.ic_reminder)
            .setColor(ContextCompat.getColor(context, R.color.reminder_accent))
            .setContentTitle(title.ifEmpty { "Напоминание" })
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setAutoCancel(true)
            .setOnlyAlertOnce(false)
            .setVibrate(if (vibrate) longArrayOf(0, 250, 150, 250) else longArrayOf(0))
            // The time the user picked, not the time the alarm arrived: an
            // inexact alarm can land minutes late, and showing that lateness as
            // the reminder's own time reads as the app getting it wrong.
            .apply { if (scheduledAt > 0L) setWhen(scheduledAt).setShowWhen(true) }
            // No summary notification is posted: from Android N the system
            // bundles four or more on its own, and a hand-rolled summary would
            // need an id that cannot collide with any request code.
            .setGroup(ReminderIntents.NOTIFICATION_GROUP)
            .setContentIntent(contentIntent)
            .build()

        try {
            NotificationManagerCompat.from(context).notify(requestCode, notification)
        } catch (denied: SecurityException) {
            // Posting was revoked between arming and firing. Losing the
            // notification is bad, crashing the receiver is worse.
            Log.w(TAG, "уведомление не показано: нет разрешения", denied)
        }
    }

    private companion object {
        const val TAG = "Organizer"
    }
}
