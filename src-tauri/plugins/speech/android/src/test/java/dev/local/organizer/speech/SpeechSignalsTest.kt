package dev.local.organizer.speech

import android.speech.SpeechRecognizer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SpeechSignalsTest {

    @Test
    fun `a silent room reads as nothing on the meter`() {
        assertEquals(0.0, SpeechSignals.level(-2f), 0.001)
        assertEquals(0.0, SpeechSignals.level(-40f), 0.001)
    }

    @Test
    fun `speech close to the microphone fills the meter`() {
        assertEquals(1.0, SpeechSignals.level(10f), 0.001)
        assertEquals(1.0, SpeechSignals.level(120f), 0.001)
    }

    @Test
    fun `an ordinary voice lands in the middle rather than at an end`() {
        val level = SpeechSignals.level(4f)
        assertTrue("$level should read as half-loud", level > 0.4 && level < 0.6)
    }

    @Test
    fun `hearing nothing is its own answer rather than a failure`() {
        assertEquals("no_speech", SpeechSignals.errorCode(SpeechRecognizer.ERROR_NO_MATCH))
        assertEquals("no_speech", SpeechSignals.errorCode(SpeechRecognizer.ERROR_SPEECH_TIMEOUT))
    }

    @Test
    fun `a missing offline model keeps its own code because it has an answer`() {
        assertEquals(
            "language",
            SpeechSignals.errorCode(SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE),
        )
        assertEquals(
            "language",
            SpeechSignals.errorCode(SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED),
        )
    }

    @Test
    fun `every way of needing a server is the same news to the person holding the phone`() {
        val serverErrors = listOf(
            SpeechRecognizer.ERROR_NETWORK,
            SpeechRecognizer.ERROR_NETWORK_TIMEOUT,
            SpeechRecognizer.ERROR_SERVER,
            SpeechRecognizer.ERROR_SERVER_DISCONNECTED,
            SpeechRecognizer.ERROR_TOO_MANY_REQUESTS,
        )
        for (error in serverErrors) {
            assertEquals("offline", SpeechSignals.errorCode(error))
        }
    }

    @Test
    fun `the permission and the busy recogniser are told apart`() {
        assertEquals(
            "permission",
            SpeechSignals.errorCode(SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS),
        )
        assertEquals("busy", SpeechSignals.errorCode(SpeechRecognizer.ERROR_RECOGNIZER_BUSY))
    }

    @Test
    fun `an error this build has never heard of still has a code`() {
        assertEquals("unknown", SpeechSignals.errorCode(9999))
    }
}
