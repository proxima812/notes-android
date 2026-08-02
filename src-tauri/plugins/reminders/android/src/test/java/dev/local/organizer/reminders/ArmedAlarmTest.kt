package dev.local.organizer.reminders

import org.json.JSONException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class ArmedAlarmTest {

    private val alarm = ArmedAlarm(
        occurrenceId = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e71",
        noteId = "0193b3b2-4d3c-7c9a-8f2e-1a2b3c4d5e70",
        requestCode = 1_234_567,
        triggerAtMillis = 1_800_000_000_000L,
        title = "Выпить воду",
        body = "Текст заметки",
        soundId = "death_and_rebirth",
        soundLabel = "Death & Rebirth",
        vibrate = false,
        exact = true,
    )

    @Test
    fun `an alarm survives being written down and read back`() {
        assertEquals(alarm, ArmedAlarm.fromJson(alarm.toJson()))
    }

    @Test
    fun `a title with quotes and newlines survives`() {
        // The journal is JSON precisely so that note titles cannot break it.
        val awkward = alarm.copy(title = "Позвонить \"маме\"\nи папе")

        assertEquals(awkward, ArmedAlarm.fromJson(awkward.toJson()))
    }

    @Test
    fun `a record missing a field the receiver cannot invent is rejected`() {
        // The sound decides which channel the notification uses, so a record
        // without one cannot be honoured and must not be guessed at.
        val without = """{"occurrence_id":"a","request_code":1,"trigger_at":2}"""

        assertThrows(JSONException::class.java) { ArmedAlarm.fromJson(without) }
    }

    @Test
    fun `optional text defaults to empty rather than failing`() {
        val minimal = """
            {"occurrence_id":"a","request_code":1,"trigger_at":2,"sound_id":"death_and_rebirth"}
        """.trimIndent()

        val restored = ArmedAlarm.fromJson(minimal)

        assertEquals("", restored.title)
        assertEquals("", restored.body)
        assertEquals("a reminder vibrates unless it was told otherwise", true, restored.vibrate)
        assertEquals("and asks for an exact alarm unless it was told otherwise", true, restored.exact)
    }
}
