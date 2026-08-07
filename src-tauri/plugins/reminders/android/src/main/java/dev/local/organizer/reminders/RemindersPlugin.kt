package dev.local.organizer.reminders

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.content.Intent
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.core.app.NotificationManagerCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
internal class ScheduleArgs {
    var occurrenceId: String = ""
    var noteId: String = ""
    var requestCode: Int = 0
    var triggerAtMillis: Long = 0
    var title: String = ""
    var body: String = ""
    var exact: Boolean = true
    var soundId: String = ""
    var soundLabel: String = ""
    var vibrate: Boolean = true
    var snoozeMinutes: Int = ArmedAlarm.DEFAULT_SNOOZE_MINUTES
}

@InvokeArg
internal class CancelArgs {
    var requestCode: Int = 0
}

@InvokeArg
internal class SoundIdArgs {
    var id: String = ""
}

/**
 * The Android half of the reminders plugin.
 *
 * It contains no rule about when anything fires: the core decides the instant
 * and the wording, and this only carries both to `AlarmManager`.
 */
@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications"),
    ],
)
class RemindersPlugin(private val activity: Activity) : Plugin(activity) {

    /**
     * Note a notification tap asked to open, waiting to be collected.
     *
     * Held here rather than pushed to the WebView because the tap can start the
     * app cold, long before any JavaScript exists to receive an event.
     */
    private var pendingNoteId: String? = null

    override fun load(webView: WebView) {
        // A cold start: the intent that launched the activity is the tap.
        capture(activity.intent)
    }

    override fun onNewIntent(intent: Intent) {
        // The app was already running. `launchMode` is `singleTask`, so the tap
        // arrives here instead of through a fresh `onCreate`.
        capture(intent)
    }

    private fun capture(intent: Intent?) {
        if (intent?.action != ReminderIntents.ACTION_OPEN) {
            return
        }
        val noteId = intent.getStringExtra(ReminderIntents.EXTRA_NOTE_ID) ?: return
        pendingNoteId = noteId
        // Consumed once. Without this the same note would reopen every time the
        // app is brought back from the background, because the activity keeps
        // the intent that started it.
        intent.removeExtra(ReminderIntents.EXTRA_NOTE_ID)
    }

    /** Hands the pending note to the core and forgets it. */
    @Command
    fun takeLaunchTarget(invoke: Invoke) {
        val result = JSObject()
        result.put("noteId", pendingNoteId)
        pendingNoteId = null
        invoke.resolve(result)
    }

    @Command
    fun schedule(invoke: Invoke) {
        val args = invoke.parseArgs(ScheduleArgs::class.java)
        if (args.occurrenceId.isEmpty()) {
            invoke.reject("не задан идентификатор срабатывания")
            return
        }

        try {
            val armedExact = AlarmScheduler.schedule(
                context = activity,
                alarm = ArmedAlarm(
                    occurrenceId = args.occurrenceId,
                    noteId = args.noteId,
                    requestCode = args.requestCode,
                    triggerAtMillis = args.triggerAtMillis,
                    title = args.title,
                    body = args.body,
                    soundId = args.soundId,
                    soundLabel = args.soundLabel,
                    vibrate = args.vibrate,
                    exact = args.exact,
                    snoozeMinutes = args.snoozeMinutes,
                ),
            )

            val result = JSObject()
            result.put("scheduledExact", armedExact)
            invoke.resolve(result)
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "не удалось поставить будильник")
        }
    }

    @Command
    fun cancel(invoke: Invoke) {
        val args = invoke.parseArgs(CancelArgs::class.java)
        AlarmScheduler.cancel(activity, args.requestCode)
        invoke.resolve(JSObject())
    }

    /**
     * Cancels every alarm this app has armed.
     *
     * Restoring a backup replaces the reminders the core knows about, which
     * leaves the OS holding alarms for occurrences that no longer exist. The
     * journal is the only record of those, so it is also the only way to take
     * them back.
     */
    @Command
    fun cancelAll(invoke: Invoke) {
        var cancelled = 0
        for (alarm in AlarmStore.all(activity)) {
            AlarmScheduler.cancel(activity, alarm.requestCode)
            cancelled++
        }

        val result = JSObject()
        result.put("cancelled", cancelled)
        invoke.resolve(result)
    }

    @Command
    fun permissionState(invoke: Invoke) {
        // `areNotificationsEnabled` is the truthful answer: it also covers the
        // user switching the app's notifications off in system settings, which
        // a permission check alone would miss.
        val enabled = NotificationManagerCompat.from(activity).areNotificationsEnabled()
        val manager = activity.getSystemService(AlarmManager::class.java)

        val result = JSObject()
        result.put("notifications", if (enabled) "granted" else "prompt")
        result.put("exactAlarms", manager != null && AlarmScheduler.canScheduleExact(manager))
        invoke.resolve(result)
    }

    // --- Sound picking -------------------------------------------------------

    /** The one preview allowed at a time; a second request replaces the first. */
    private var preview: MediaPlayer? = null

    /**
     * Everything the sound picker can offer beyond the bundled presets: the
     * device's own notification sounds and the files the user brought in.
     */
    @Command
    fun listDeviceSounds(invoke: Invoke) {
        val system = JSArray()
        try {
            val manager = RingtoneManager(activity)
            manager.setType(RingtoneManager.TYPE_NOTIFICATION)
            val cursor = manager.cursor
            while (cursor.moveToNext()) {
                val uri = manager.getRingtoneUri(cursor.position) ?: continue
                val entry = JSObject()
                entry.put("id", "${SoundIds.SYSTEM_PREFIX}$uri")
                entry.put("label", cursor.getString(RingtoneManager.TITLE_COLUMN_INDEX))
                system.put(entry)
            }
        } catch (failure: Exception) {
            // A device without a ringtone catalogue still has presets and
            // custom files to offer, so the list degrades instead of failing.
        }

        val custom = JSArray()
        for (sound in CustomSoundStore.open(activity).all()) {
            val entry = JSObject()
            entry.put("id", "${SoundIds.CUSTOM_PREFIX}${sound.fileName}")
            entry.put("label", sound.label)
            custom.put(entry)
        }

        val result = JSObject()
        result.put("system", system)
        result.put("custom", custom)
        invoke.resolve(result)
    }

    /** Opens the document picker on anything that might contain audio. */
    @Command
    fun pickCustomSound(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            // The generic type with an audio-leaning filter: some file managers
            // mislabel music files, and a video's audio track is fair game too.
            type = "*/*"
            putExtra(Intent.EXTRA_MIME_TYPES, arrayOf("audio/*", "video/mp4", "video/*"))
        }
        startActivityForResult(invoke, intent, "onCustomSoundPicked")
    }

    @ActivityCallback
    fun onCustomSoundPicked(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            // Backing out of the picker is an ordinary thing to do, not a
            // failure.
            val outcome = JSObject()
            outcome.put("completed", false)
            // An explicit JSON null: `put(key, null)` would drop the key.
            outcome.put("sound", org.json.JSONObject.NULL)
            invoke.resolve(outcome)
            return
        }

        // Decoding a song takes real time, and this callback is on the main
        // thread of the UI the picker just returned to.
        Thread {
            val temp = File(activity.cacheDir, "reminder_sound_import.tmp")
            try {
                activity.contentResolver.openInputStream(uri).use { input ->
                    if (input == null) {
                        invoke.reject("выбранный файл недоступен для чтения")
                        return@Thread
                    }
                    temp.outputStream().use { input.copyTo(it) }
                }

                val directory = ReminderSoundProvider.directory(activity)
                directory.mkdirs()
                val fileName = CustomSoundStore.newFileName()
                val trimmed = File(directory, fileName)
                try {
                    AudioTrimmer.trim(temp, trimmed)
                } catch (failure: Exception) {
                    trimmed.delete()
                    invoke.reject(
                        "не удалось обработать аудиофайл: ${failure.message ?: "формат не поддерживается"}",
                    )
                    return@Thread
                }

                val label = CustomSoundStore.labelFrom(displayName(uri))
                CustomSoundStore.open(activity).put(fileName, label)

                val sound = JSObject()
                sound.put("id", "${SoundIds.CUSTOM_PREFIX}$fileName")
                sound.put("label", label)
                val outcome = JSObject()
                outcome.put("completed", true)
                outcome.put("sound", sound)
                invoke.resolve(outcome)
            } catch (failure: Exception) {
                invoke.reject(failure.message ?: "не удалось обработать аудиофайл")
            } finally {
                temp.delete()
            }
        }.start()
    }

    @Command
    fun deleteCustomSound(invoke: Invoke) {
        val args = invoke.parseArgs(SoundIdArgs::class.java)
        val source = try {
            SoundIds.parse(args.id)
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "некорректный ID звука")
            return
        }
        if (source !is SoundSource.Custom) {
            invoke.reject("удалить можно только свой звук")
            return
        }

        // A missing file means the deletion already happened — which is the
        // state that was asked for, not an error.
        File(ReminderSoundProvider.directory(activity), source.fileName).delete()
        CustomSoundStore.open(activity).remove(source.fileName)
        invoke.resolve(JSObject())
    }

    /**
     * Plays a sound once so the user can judge it before committing.
     *
     * Resolves immediately: the frontend's job is to show the list, not to
     * wait out ten seconds of audio.
     */
    @Command
    fun previewSound(invoke: Invoke) {
        val args = invoke.parseArgs(SoundIdArgs::class.java)
        val uri: Uri
        try {
            uri = ReminderNotifications.soundUri(activity, args.id)
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "некорректный ID звука")
            return
        }

        stopPreviewPlayer()
        val player = MediaPlayer()
        try {
            player.setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_NOTIFICATION)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                    .build(),
            )
            player.setDataSource(activity, uri)
            player.setOnCompletionListener { finished ->
                finished.release()
                if (preview === finished) {
                    preview = null
                }
            }
            player.setOnErrorListener { failed, _, _ ->
                failed.release()
                if (preview === failed) {
                    preview = null
                }
                true
            }
            player.prepare()
            player.start()
            preview = player
        } catch (failure: Exception) {
            player.release()
            invoke.reject(failure.message ?: "не удалось проиграть звук")
            return
        }
        invoke.resolve(JSObject())
    }

    @Command
    fun stopPreview(invoke: Invoke) {
        stopPreviewPlayer()
        invoke.resolve(JSObject())
    }

    private fun stopPreviewPlayer() {
        val player = preview ?: return
        preview = null
        runCatching { player.stop() }
        runCatching { player.release() }
    }

    /** The name the user sees for the file they picked, for the label only. */
    private fun displayName(uri: Uri): String? =
        runCatching {
            activity.contentResolver
                .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor ->
                    val column = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (column >= 0 && cursor.moveToFirst()) cursor.getString(column) else null
                }
        }.getOrNull()
}
