package dev.local.organizer.reminders

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Re-arms alarms after the OS has thrown them away.
 *
 * A reboot clears every `PendingIntent`, and so does replacing the package on
 * update. The app process is not running to see either, so without this a
 * reminder set on Monday for Friday quietly stops existing the first time the
 * phone restarts.
 *
 * `BOOT_COMPLETED` arrives only after the user unlocks, which is what makes
 * reading [AlarmStore] from ordinary (credential-encrypted) storage safe here.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED -> Unit
            else -> return
        }

        val now = System.currentTimeMillis()
        var restored = 0
        var missed = 0
        var dropped = 0

        for (alarm in AlarmStore.all(context)) {
            val lateBy = now - alarm.triggerAtMillis
            val alarmToArm = when {
                lateBy <= 0L -> alarm
                // The phone was off when this was due. Firing it a moment after
                // the user turns the phone back on is what they asked for, just
                // late; saying nothing at all is the one outcome a reminder
                // must never have.
                lateBy <= MISSED_GRACE_MILLIS -> alarm.copy(triggerAtMillis = now + CATCH_UP_DELAY_MILLIS)
                // Too stale to be a reminder any more — after a week off, a
                // notification for last Tuesday is noise, not a service.
                else -> {
                    AlarmStore.forget(context, alarm.requestCode)
                    dropped++
                    continue
                }
            }

            try {
                AlarmScheduler.schedule(context, alarmToArm)
                if (lateBy > 0L) missed++ else restored++
            } catch (failure: Exception) {
                // One reminder that cannot be re-armed — a sound removed by an
                // update, say — must not cost the user all the others.
                Log.w(TAG, "не удалось восстановить будильник ${alarm.requestCode}", failure)
            }
        }

        Log.i(TAG, "будильники восстановлены: $restored, просрочено: $missed, снято: $dropped")
    }

    private companion object {
        const val TAG = "Organizer"

        /** How late a missed reminder may be and still be worth showing. */
        const val MISSED_GRACE_MILLIS = 24L * 60L * 60L * 1000L

        /** Long enough for the launcher to settle before the shade lights up. */
        const val CATCH_UP_DELAY_MILLIS = 10_000L
    }
}
