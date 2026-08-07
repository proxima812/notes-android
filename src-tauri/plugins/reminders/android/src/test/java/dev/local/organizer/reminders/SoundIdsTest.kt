package dev.local.organizer.reminders

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundIdsTest {

    @Test
    fun `a bare id is a preset`() {
        assertEquals(
            SoundSource.Preset("death_and_rebirth"),
            SoundIds.parse("death_and_rebirth"),
        )
    }

    @Test
    fun `a system id carries its content uri`() {
        assertEquals(
            SoundSource.System("content://media/internal/audio/media/42"),
            SoundIds.parse("system:content://media/internal/audio/media/42"),
        )
    }

    @Test
    fun `a custom id carries its file name`() {
        assertEquals(
            SoundSource.Custom("snd_3f9a2c1d.m4a"),
            SoundIds.parse("custom:snd_3f9a2c1d.m4a"),
        )
    }

    @Test
    fun `a bare id with strange characters is rejected`() {
        // A bare id becomes a resource lookup and part of a channel id, so
        // anything outside the safe alphabet must not get that far.
        assertThrows(IllegalArgumentException::class.java) {
            SoundIds.parse("../secret")
        }
    }

    @Test
    fun `a preset keeps the historic channel id`() {
        // Channels already exist on users' devices under this exact formula;
        // renaming one would silently reset its sound to the default.
        assertEquals(
            "reminders_sound_death_and_rebirth_v1",
            SoundIds.channelId("death_and_rebirth"),
        )
    }

    @Test
    fun `a prefixed id maps to a safe channel id`() {
        val channel = SoundIds.channelId("system:content://media/internal/audio/media/42")

        assertTrue(
            "a channel id must stay inside the safe alphabet, got $channel",
            Regex("^reminders_sound_x[a-z0-9_]+_\\d+_v1$").matches(channel),
        )
    }

    @Test
    fun `the same prefixed id always gets the same channel`() {
        val id = "custom:snd_3f9a2c1d.m4a"

        assertEquals(SoundIds.channelId(id), SoundIds.channelId(id))
    }

    @Test
    fun `different prefixed ids get different channels`() {
        // Sanitising alone would merge these: the hash is what keeps a
        // channel's sound from quietly becoming another file's.
        assertNotEquals(
            SoundIds.channelId("custom:snd_aaaaaaaa.m4a"),
            SoundIds.channelId("custom:snd_bbbbbbbb.m4a"),
        )
    }

    @Test
    fun `a system channel is not a custom channel`() {
        assertNotEquals(
            SoundIds.channelId("system:content://media/a"),
            SoundIds.channelId("custom:content://media/a"),
        )
    }
}
