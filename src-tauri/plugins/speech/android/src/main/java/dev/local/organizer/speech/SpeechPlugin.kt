package dev.local.organizer.speech

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.speech.RecognitionListener
import android.speech.RecognitionSupport
import android.speech.RecognitionSupportCallback
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.concurrent.atomic.AtomicBoolean

@InvokeArg
internal class StartArgs {
    /** BCP 47 tag, e.g. `ru-RU`. */
    var language: String = ""
    var preferOffline: Boolean = true
    lateinit var onEvent: Channel
}

@InvokeArg
internal class LanguageArgs {
    var language: String = ""
}

/**
 * The Android half of the dictation plugin.
 *
 * It listens, and that is all. What the words mean — a title, a time, an alarm
 * half an hour earlier — is decided by the Rust core, which never sees the
 * microphone. Audio is not recorded, not buffered and not written anywhere: the
 * recogniser is handed the stream by the system and gives back text.
 */
@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.RECORD_AUDIO], alias = "microphone"),
    ],
)
class SpeechPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        /**
         * The action a launcher shortcut sends to open straight into dictation.
         *
         * Declared here rather than in the manifest's own words because both
         * sides of that agreement — `shortcuts.xml` and this plugin — have to
         * spell it identically, and only one of them can be tested.
         */
        const val ACTION_DICTATE = "dev.local.organizer.DICTATE"

        /**
         * How long to wait for Android to answer which languages it can
         * recognise offline.
         *
         * The callback is documented as asynchronous with no promise that it
         * arrives at all. A dictation screen that waits forever is worse than
         * one that falls back to the language it was asked for.
         */
        private const val LANGUAGE_QUERY_TIMEOUT_MS = 3_000L
    }

    /**
     * The live recogniser, or null between dictations.
     *
     * Touched only on the main thread: `SpeechRecognizer` throws when it is
     * created or driven from anywhere else, and the Tauri command threads are
     * not it.
     */
    private var recognizer: SpeechRecognizer? = null

    /** Where the current dictation's events go, or null when none is running. */
    private var events: Channel? = null

    /**
     * A shortcut tap waiting to be collected.
     *
     * Held rather than pushed, for the same reason the reminders plugin holds a
     * tapped note: the tap can start the app cold, long before any JavaScript
     * exists to hear about it.
     */
    private var dictationRequested = false

    override fun load(webView: WebView) {
        captureShortcut(activity.intent)
    }

    override fun onNewIntent(intent: Intent) {
        captureShortcut(intent)
    }

    private fun captureShortcut(intent: Intent?) {
        if (intent?.action != ACTION_DICTATE) {
            return
        }
        dictationRequested = true
        // Consumed once. Without this, every return from the background would
        // reopen the microphone, because the activity keeps the intent that
        // started it.
        intent.action = Intent.ACTION_MAIN
    }

    /** Whether the app was opened by the "Dictate" shortcut. Answering clears it. */
    @Command
    fun takeDictationRequest(invoke: Invoke) {
        val result = JSObject()
        result.put("requested", dictationRequested)
        dictationRequested = false
        invoke.resolve(result)
    }

    @Command
    fun availability(invoke: Invoke) {
        val result = JSObject()
        result.put("available", SpeechRecognizer.isRecognitionAvailable(activity))
        result.put("granted", isMicrophoneGranted())
        // Whether the promise on the tin can actually be kept. `EXTRA_PREFER_OFFLINE`
        // is a hint a service may ignore, and only the on-device recogniser —
        // Android 12 and up — has no network path to ignore it with. The screen
        // says so out loud rather than letting the app claim more than it can.
        result.put("offlineGuaranteed", hasOnDeviceRecognition())
        invoke.resolve(result)
    }

    /**
     * The languages this device can recognise without a network.
     *
     * Asked because the app speaks eight languages and Android recognises a
     * different eight: Tatar and Bashkir have no recogniser anywhere, and
     * dictating in an interface language nobody can transcribe is a dead
     * button. The caller picks from this list; the plugin does not choose.
     */
    @Command
    fun supportedLanguages(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            // Before Android 13 there is no way to ask. An empty list means
            // "unknown", not "none", and the caller keeps its own preference.
            invoke.resolve(languagesResult(emptyList(), emptyList(), known = false))
            return
        }

        activity.runOnUiThread {
            // Answered exactly once, whichever arrives first: Android's callback
            // or the deadline.
            val answered = AtomicBoolean(false)
            val probe = try {
                SpeechRecognizer.createOnDeviceSpeechRecognizer(activity)
            } catch (error: Exception) {
                invoke.resolve(languagesResult(emptyList(), emptyList(), known = false))
                return@runOnUiThread
            }

            fun finish(installed: List<String>, supported: List<String>, known: Boolean) {
                if (answered.compareAndSet(false, true)) {
                    probe.destroy()
                    invoke.resolve(languagesResult(installed, supported, known))
                }
            }

            Handler(Looper.getMainLooper()).postDelayed(
                { finish(emptyList(), emptyList(), known = false) },
                LANGUAGE_QUERY_TIMEOUT_MS,
            )

            try {
                probe.checkRecognitionSupport(
                    Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                        putExtra(
                            RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
                        )
                    },
                    activity.mainExecutor,
                    object : RecognitionSupportCallback {
                        override fun onSupportResult(support: RecognitionSupport) {
                            finish(
                                support.installedOnDeviceLanguages,
                                support.supportedOnDeviceLanguages,
                                known = true,
                            )
                        }

                        override fun onError(error: Int) {
                            finish(emptyList(), emptyList(), known = false)
                        }
                    },
                )
            } catch (error: Exception) {
                finish(emptyList(), emptyList(), known = false)
            }
        }
    }

    /**
     * Asks Android to fetch the offline model for a language.
     *
     * The one repair there is for «the offline model is not installed»: without
     * it the screen can only send the person into the system settings to hunt
     * for a menu most people have never opened.
     */
    @Command
    fun downloadLanguage(invoke: Invoke) {
        val args = invoke.parseArgs(LanguageArgs::class.java)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            invoke.reject("this Android version cannot be asked to fetch a model")
            return
        }

        activity.runOnUiThread {
            try {
                val downloader = SpeechRecognizer.createOnDeviceSpeechRecognizer(activity)
                downloader.triggerModelDownload(
                    Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                        putExtra(RecognizerIntent.EXTRA_LANGUAGE, args.language)
                        putExtra(
                            RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
                        )
                    },
                )
                // The download runs in the system, not here, and reports nothing
                // back on this API level. Resolving means "asked", which is all
                // that can honestly be said.
                downloader.destroy()
                invoke.resolve()
            } catch (error: Exception) {
                invoke.reject(error.message ?: "the model could not be requested")
            }
        }
    }

    /**
     * The system microphone prompt.
     *
     * Answered with the state afterwards rather than with whether the dialog
     * appeared: a permission already granted, or refused twice and no longer
     * asked for, both skip the dialog and the caller still needs the answer.
     */
    @Command
    fun requestMicrophone(invoke: Invoke) {
        requestPermissionForAlias("microphone", invoke, "microphoneResult")
    }

    @PermissionCallback
    fun microphoneResult(invoke: Invoke) {
        val granted = isMicrophoneGranted()
        val result = JSObject()
        result.put("granted", granted)
        // Android shows no dialog once a permission has been refused for good,
        // so a caller that only sees `granted: false` would offer a button that
        // does nothing. Told apart, the screen can point at the settings page
        // instead.
        result.put(
            "blocked",
            !granted &&
                !ActivityCompat.shouldShowRequestPermissionRationale(
                    activity,
                    Manifest.permission.RECORD_AUDIO,
                ),
        )
        invoke.resolve(result)
    }

    /** Opens this app's page in the Android settings, permissions and all. */
    @Command
    fun openAppSettings(invoke: Invoke) {
        val intent = Intent(
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            Uri.fromParts("package", activity.packageName, null),
        ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        try {
            activity.startActivity(intent)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "the settings screen could not be opened")
        }
    }

    @Command
    fun start(invoke: Invoke) {
        val args = invoke.parseArgs(StartArgs::class.java)

        if (!SpeechRecognizer.isRecognitionAvailable(activity)) {
            invoke.reject("this device has no speech recognition service")
            return
        }
        if (!isMicrophoneGranted()) {
            invoke.reject("the microphone permission is not granted")
            return
        }

        activity.runOnUiThread {
            try {
                // A dictation already running is replaced rather than refused:
                // the button that starts one is the same button in the same
                // place, and a stuck recogniser must not make it dead.
                releaseRecognizer()

                val recognizer = createRecognizer(args.preferOffline)
                events = args.onEvent
                recognizer.setRecognitionListener(listener)
                recognizer.startListening(recognitionIntent(args))
                this.recognizer = recognizer
                invoke.resolve()
            } catch (error: Exception) {
                releaseRecognizer()
                invoke.reject(error.message ?: "the recogniser refused to start")
            }
        }
    }

    /** Stops listening and keeps what was heard: the "done" button. */
    @Command
    fun stop(invoke: Invoke) {
        activity.runOnUiThread {
            // `stopListening` still delivers `onResults`, so the recogniser is
            // left alive here and released when that arrives.
            recognizer?.stopListening()
            invoke.resolve()
        }
    }

    /**
     * Stops listening and throws away what was heard.
     *
     * Cancelling when nothing is running succeeds. The screen calls this as it
     * unmounts, and making it check first would only move the same test.
     */
    @Command
    fun cancel(invoke: Invoke) {
        activity.runOnUiThread {
            releaseRecognizer()
            invoke.resolve()
        }
    }

    override fun onPause() {
        // The microphone must not stay open behind another app. The screen is
        // told, because otherwise it waits for a `final` that can no longer
        // come.
        activity.runOnUiThread {
            if (recognizer != null) {
                send("aborted")
            }
            releaseRecognizer()
        }
    }

    private fun isMicrophoneGranted(): Boolean =
        ActivityCompat.checkSelfPermission(activity, Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED

    private fun hasOnDeviceRecognition(): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            SpeechRecognizer.isOnDeviceRecognitionAvailable(activity)

    /**
     * Prefers the recogniser that cannot reach a server at all.
     *
     * From Android 12 the on-device recogniser is a separate object, and asking
     * for it is stronger than asking the ordinary one to stay offline: the flag
     * below is a preference the service may ignore, while this one has no
     * network path to ignore it with. Older devices fall back to the flag, and
     * `availability` tells the screen that the promise is weaker there.
     */
    private fun createRecognizer(preferOffline: Boolean): SpeechRecognizer {
        if (preferOffline && hasOnDeviceRecognition()) {
            return SpeechRecognizer.createOnDeviceSpeechRecognizer(activity)
        }
        return SpeechRecognizer.createSpeechRecognizer(activity)
    }

    private fun recognitionIntent(args: StartArgs): Intent =
        Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(
                RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
            )
            if (args.language.isNotEmpty()) {
                putExtra(RecognizerIntent.EXTRA_LANGUAGE, args.language)
            }
            // Words on the screen while they are being said are the only sign
            // that the app is hearing anything at all.
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
            putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, args.preferOffline)
            putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, activity.packageName)
        }

    private val listener = object : RecognitionListener {
        override fun onReadyForSpeech(params: Bundle?) = send("ready")

        override fun onBeginningOfSpeech() = send("speaking")

        override fun onRmsChanged(rmsdB: Float) {
            val payload = JSObject()
            payload.put("kind", "level")
            payload.put("level", SpeechSignals.level(rmsdB))
            events?.send(payload)
        }

        override fun onBufferReceived(buffer: ByteArray?) {
            // Raw audio. Deliberately dropped: nothing in this app has a use
            // for a recording, so none is kept even for a moment.
        }

        /** The person stopped talking; the recogniser is still working. */
        override fun onEndOfSpeech() = send("thinking")

        override fun onError(error: Int) {
            val payload = JSObject()
            payload.put("kind", "error")
            payload.put("code", SpeechSignals.errorCode(error))
            events?.send(payload)
            releaseRecognizer()
        }

        override fun onResults(results: Bundle?) {
            val payload = JSObject()
            payload.put("kind", "final")
            payload.put("text", firstResult(results))
            events?.send(payload)
            releaseRecognizer()
        }

        override fun onPartialResults(partialResults: Bundle?) {
            val text = firstResult(partialResults)
            if (text.isEmpty()) {
                return
            }
            val payload = JSObject()
            payload.put("kind", "partial")
            payload.put("text", text)
            events?.send(payload)
        }

        override fun onEvent(eventType: Int, params: Bundle?) {
            // Recogniser-specific extras. Nothing here depends on any of them.
        }
    }

    private fun send(kind: String) {
        val payload = JSObject()
        payload.put("kind", kind)
        events?.send(payload)
    }

    private fun firstResult(bundle: Bundle?): String =
        bundle
            ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
            ?.firstOrNull()
            .orEmpty()

    private fun languagesResult(
        installed: List<String>,
        supported: List<String>,
        known: Boolean,
    ): JSObject {
        val result = JSObject()
        result.put("known", known)
        result.put("installed", JSArray.from(installed.toTypedArray()))
        result.put("supported", JSArray.from(supported.toTypedArray()))
        return result
    }

    /**
     * Lets go of the recogniser and the channel.
     *
     * Both together: a channel left behind would deliver events from a
     * recogniser the screen has already forgotten about, and a recogniser left
     * behind holds the microphone.
     */
    private fun releaseRecognizer() {
        recognizer?.let { recognizer ->
            recognizer.setRecognitionListener(null)
            recognizer.cancel()
            recognizer.destroy()
        }
        recognizer = null
        events = null
    }
}
