package dev.local.organizer.reminders

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RestorePlannerTest {

    private fun alarm(triggerAt: Long) = ArmedAlarm(
        occurrenceId = "occurrence",
        noteId = "note",
        requestCode = 7,
        triggerAtMillis = triggerAt,
        title = "Позвонить",
        body = "",
        soundId = "death_and_rebirth",
        soundLabel = "Death & Rebirth",
        vibrate = true,
        exact = true,
        snoozeMinutes = 10,
    )

    private val now = 1_800_000_000_000L

    @Test
    fun `an alarm still ahead of us is put back untouched`() {
        val original = alarm(now + 60_000)

        val plan = RestorePlanner.plan(original, now)

        assertEquals(RestorePlan.Arm(original, wasMissed = false), plan)
    }

    @Test
    fun `an alarm missed while the phone was off fires shortly after it comes back`() {
        val plan = RestorePlanner.plan(alarm(now - 60_000), now)

        assertTrue(plan is RestorePlan.Arm)
        val armed = plan as RestorePlan.Arm
        assertTrue("a missed reminder has to be reported as missed", armed.wasMissed)
        assertEquals(
            "it fires just after the launcher has settled, not at its original time",
            now + RestorePlanner.CATCH_UP_DELAY_MILLIS,
            armed.alarm.triggerAtMillis,
        )
    }

    @Test
    fun `an alarm missed by almost a day is still shown`() {
        val plan = RestorePlanner.plan(alarm(now - RestorePlanner.MISSED_GRACE_MILLIS + 1), now)

        assertTrue(plan is RestorePlan.Arm)
    }

    @Test
    fun `an alarm older than the grace period is dropped rather than shown late`() {
        val plan = RestorePlanner.plan(alarm(now - RestorePlanner.MISSED_GRACE_MILLIS - 1), now)

        assertEquals(RestorePlan.Drop, plan)
    }

    @Test
    fun `an alarm due at this very moment is armed as it stands`() {
        // Arming it for an instant that has just arrived makes the OS deliver
        // it immediately, so there is nothing to catch up on.
        val original = alarm(now)

        val plan = RestorePlanner.plan(original, now)

        assertEquals(RestorePlan.Arm(original, wasMissed = false), plan)
    }
}
