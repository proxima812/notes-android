package dev.local.organizer.reminders

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat

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
        val vibrate = intent.getBooleanExtra(ReminderIntents.EXTRA_VIBRATE, true)

        // The plugin module cannot name the app's activity, so the launcher
        // intent is asked for by package instead.
        val launch = context.packageManager
            .getLaunchIntentForPackage(context.packageName)
            ?.apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
                putExtra(ReminderIntents.EXTRA_OCCURRENCE_ID, occurrenceId)
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
            .setSmallIcon(android.R.drawable.ic_popup_reminder)
            .setContentTitle(title.ifEmpty { "Напоминание" })
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setAutoCancel(true)
            .setOnlyAlertOnce(false)
            .setVibrate(if (vibrate) longArrayOf(0, 250, 150, 250) else longArrayOf(0))
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
