package dev.local.organizer.reminders

/** What to do with one remembered alarm after a reboot. */
internal sealed interface RestorePlan {
    /** Arm it again, at the instant carried here. */
    data class Arm(val alarm: ArmedAlarm, val wasMissed: Boolean) : RestorePlan

    /** Too stale to be worth showing; forget it. */
    data object Drop : RestorePlan
}

/**
 * Decides the fate of a remembered alarm once the phone is back.
 *
 * Separated from the receiver so the rule can be tested without an Android
 * runtime: it is the one part of the reboot path that makes a decision rather
 * than calling an API.
 */
internal object RestorePlanner {

    /** How late a missed reminder may be and still be worth showing. */
    const val MISSED_GRACE_MILLIS = 24L * 60L * 60L * 1000L

    /** Long enough for the launcher to settle before the shade lights up. */
    const val CATCH_UP_DELAY_MILLIS = 10_000L

    fun plan(alarm: ArmedAlarm, now: Long): RestorePlan {
        val lateBy = now - alarm.triggerAtMillis
        return when {
            // Still ahead of us: put it back exactly as it was.
            lateBy <= 0L -> RestorePlan.Arm(alarm, wasMissed = false)

            // The phone was off when this was due. Firing it a moment after the
            // user turns the phone back on is what they asked for, just late;
            // saying nothing at all is the one outcome a reminder must never
            // have.
            lateBy <= MISSED_GRACE_MILLIS -> RestorePlan.Arm(
                alarm.copy(triggerAtMillis = now + CATCH_UP_DELAY_MILLIS),
                wasMissed = true,
            )

            // After a week off, a notification for last Tuesday is noise.
            else -> RestorePlan.Drop
        }
    }
}
