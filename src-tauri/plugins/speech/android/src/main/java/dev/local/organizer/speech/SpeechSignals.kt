package dev.local.organizer.speech

import android.speech.SpeechRecognizer

/**
 * The two translations between Android's recogniser and the screen.
 *
 * Both are pure functions in their own file so they can be tested without a
 * device: the rest of the plugin is callbacks from a system service, and an
 * emulator is a poor place to pin down what a loudness reading means.
 */
object SpeechSignals {

    /**
     * The quietest reading treated as sound at all, and the loudest that still
     * moves the meter.
     *
     * `onRmsChanged` reports decibels, and the documentation does not promise a
     * range. In practice a silent room sits around -2 and speech close to the
     * microphone reaches about 10, so those are the ends of the bar. A person
     * shouting does not need extra travel on the meter — they need to see that
     * the app is hearing them.
     */
    private const val QUIETEST_DB = -2.0
    private const val LOUDEST_DB = 10.0

    /**
     * A loudness reading as a number between 0 and 1, for a bar or a ring.
     *
     * Clamped rather than scaled to whatever arrives: a reading below the floor
     * is silence, and one above the ceiling is still just "loud".
     */
    fun level(rms: Float): Double {
        val normalised = (rms - QUIETEST_DB) / (LOUDEST_DB - QUIETEST_DB)
        return normalised.coerceIn(0.0, 1.0)
    }

    /**
     * A recogniser error as a code the interface has a sentence for.
     *
     * Grouped by what the person can do about it, not by what went wrong
     * inside: three different network failures all mean "this needed a server
     * and the app does not use one", and telling them apart helps nobody
     * holding the phone.
     */
    fun errorCode(error: Int): String = when (error) {
        SpeechRecognizer.ERROR_NO_MATCH,
        SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "no_speech"

        SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "permission"

        SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "busy"

        SpeechRecognizer.ERROR_AUDIO -> "audio"

        // The offline model for this language is not on the device. This is the
        // one error with a real answer — install it in the system settings —
        // so it keeps its own code.
        SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE,
        SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED -> "language"

        SpeechRecognizer.ERROR_NETWORK,
        SpeechRecognizer.ERROR_NETWORK_TIMEOUT,
        SpeechRecognizer.ERROR_SERVER,
        SpeechRecognizer.ERROR_SERVER_DISCONNECTED,
        SpeechRecognizer.ERROR_TOO_MANY_REQUESTS -> "offline"

        else -> "unknown"
    }
}
