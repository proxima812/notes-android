package dev.local.organizer.reminders

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.content.Intent
import android.webkit.WebView
import androidx.core.app.NotificationManagerCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

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
}

@InvokeArg
internal class CancelArgs {
    var requestCode: Int = 0
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
            val channelId = ReminderNotifications.ensureChannel(
                context = activity,
                soundId = args.soundId,
                soundLabel = args.soundLabel,
                vibrate = args.vibrate,
            )
            val armedExact = AlarmScheduler.schedule(
                context = activity,
                occurrenceId = args.occurrenceId,
                noteId = args.noteId,
                requestCode = args.requestCode,
                triggerAtMillis = args.triggerAtMillis,
                title = args.title,
                body = args.body,
                exact = args.exact,
                channelId = channelId,
                vibrate = args.vibrate,
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
}
