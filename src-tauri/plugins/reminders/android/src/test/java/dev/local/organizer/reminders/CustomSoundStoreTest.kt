package dev.local.organizer.reminders

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CustomSoundStoreTest {

    /** The production store over `SharedPreferences`, here over a map. */
    private class MapStore : CustomSoundStore.KeyValueStore {
        private val entries = mutableMapOf<String, String>()
        override fun all(): Map<String, String> = entries.toMap()
        override fun put(key: String, value: String) {
            entries[key] = value
        }
        override fun remove(key: String) {
            entries.remove(key)
        }
    }

    @Test
    fun `a stored label is found by its file name`() {
        val store = CustomSoundStore(MapStore())

        store.put("snd_3f9a2c1d.m4a", "Утренний дождь")

        assertEquals("Утренний дождь", store.label("snd_3f9a2c1d.m4a"))
    }

    @Test
    fun `removing a sound forgets its label`() {
        val store = CustomSoundStore(MapStore())
        store.put("snd_3f9a2c1d.m4a", "Утренний дождь")

        store.remove("snd_3f9a2c1d.m4a")

        assertNull(store.label("snd_3f9a2c1d.m4a"))
        assertEquals(emptyList<CustomSoundStore.Entry>(), store.all())
    }

    @Test
    fun `the list is sorted by label, not by file name`() {
        val store = CustomSoundStore(MapStore())
        store.put("snd_aaaaaaaa.m4a", "Ветер")
        store.put("snd_bbbbbbbb.m4a", "Аккорд")

        assertEquals(listOf("Аккорд", "Ветер"), store.all().map { it.label })
    }

    @Test
    fun `the label is the display name without its extension`() {
        assertEquals("Утренний дождь", CustomSoundStore.labelFrom("Утренний дождь.mp3"))
        assertEquals("clip.final", CustomSoundStore.labelFrom("clip.final.mp4"))
    }

    @Test
    fun `a name that is only an extension keeps its dot`() {
        // ".mp3" has no base name to cut down to: stripping would leave
        // nothing to show.
        assertEquals(".mp3", CustomSoundStore.labelFrom(".mp3"))
    }

    @Test
    fun `a missing display name still yields something to show`() {
        assertEquals("Свой звук", CustomSoundStore.labelFrom(null))
        assertEquals("Свой звук", CustomSoundStore.labelFrom("   "))
    }

    @Test
    fun `generated file names are m4a and do not repeat`() {
        val names = (1..100).map { CustomSoundStore.newFileName() }.toSet()

        assertEquals(100, names.size)
        assertTrue(names.all { Regex("^snd_[0-9a-f]{8}\\.m4a$").matches(it) })
    }
}
