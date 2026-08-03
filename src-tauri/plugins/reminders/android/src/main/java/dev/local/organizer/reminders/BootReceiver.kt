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
 *
 * The decision about what is still worth showing lives in [RestorePlanner];
 * this only carries it out.
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
            when (val plan = RestorePlanner.plan(alarm, now)) {
                is RestorePlan.Drop -> {
                    AlarmStore.forget(context, alarm.requestCode)
                    dropped++
                }

                is RestorePlan.Arm -> try {
                    AlarmScheduler.schedule(context, plan.alarm)
                    if (plan.wasMissed) missed++ else restored++
                } catch (failure: Exception) {
                    // One reminder that cannot be re-armed — a sound removed by
                    // an update, say — must not cost the user all the others.
                    Log.w(TAG, "не удалось восстановить будильник ${alarm.requestCode}", failure)
                }
            }
        }

        Log.i(TAG, "будильники восстановлены: $restored, просрочено: $missed, снято: $dropped")
    }

    private companion object {
        const val TAG = "Organizer"
    }
}
