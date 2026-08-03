package dev.local.organizer.documents

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
internal class ExportArgs {
    /** File the core has already written and wants copied out. */
    var sourcePath: String = ""
    var suggestedName: String = ""
    var mimeType: String = "application/octet-stream"
}

@InvokeArg
internal class ImportArgs {
    /** Where the chosen file should land so the core can read it. */
    var destinationPath: String = ""
    var mimeType: String = "*/*"
}

/**
 * Moves files between the app's private directory and wherever the user keeps
 * their own files.
 *
 * Everything goes through the system document picker, so the app never holds a
 * storage permission: it gets access to exactly the one file the user pointed
 * at, for as long as that takes. Deciding *what* to write and whether a file
 * read back is acceptable stays in the Rust core — this only carries bytes.
 */
@TauriPlugin
class DocumentsPlugin(private val activity: Activity) : Plugin(activity) {

    /** Path the pending import should be written to, kept across the picker. */
    private var pendingDestination: String? = null

    /** File the pending export should copy out, kept across the picker. */
    private var pendingSource: String? = null

    @Command
    fun exportDocument(invoke: Invoke) {
        val args = invoke.parseArgs(ExportArgs::class.java)
        val source = File(args.sourcePath)
        if (!source.isFile) {
            invoke.reject("файл для экспорта не найден")
            return
        }
        pendingSource = args.sourcePath

        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = args.mimeType
            putExtra(Intent.EXTRA_TITLE, args.suggestedName)
        }
        startActivityForResult(invoke, intent, "onExportPicked")
    }

    @ActivityCallback
    fun onExportPicked(invoke: Invoke, result: ActivityResult) {
        val source = pendingSource
        pendingSource = null

        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null || source == null) {
            // Backing out of the picker is an ordinary thing to do, not a
            // failure: the core reports "not saved" rather than an error.
            invoke.resolve(outcome(saved = false, name = null))
            return
        }

        try {
            activity.contentResolver.openOutputStream(uri, "wt").use { output ->
                if (output == null) {
                    invoke.reject("выбранный файл недоступен для записи")
                    return
                }
                File(source).inputStream().use { it.copyTo(output) }
            }
            invoke.resolve(outcome(saved = true, name = displayName(uri)))
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "не удалось записать файл")
        }
    }

    @Command
    fun importDocument(invoke: Invoke) {
        val args = invoke.parseArgs(ImportArgs::class.java)
        if (args.destinationPath.isEmpty()) {
            invoke.reject("не задан путь для импорта")
            return
        }
        pendingDestination = args.destinationPath

        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            // A backup has no registered type on most devices, so narrowing this
            // to one MIME type would grey out the very file the user came for.
            type = "*/*"
        }
        startActivityForResult(invoke, intent, "onImportPicked")
    }

    @ActivityCallback
    fun onImportPicked(invoke: Invoke, result: ActivityResult) {
        val destination = pendingDestination
        pendingDestination = null

        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null || destination == null) {
            invoke.resolve(outcome(saved = false, name = null))
            return
        }

        try {
            val target = File(destination)
            target.parentFile?.mkdirs()
            activity.contentResolver.openInputStream(uri).use { input ->
                if (input == null) {
                    invoke.reject("выбранный файл недоступен для чтения")
                    return
                }
                target.outputStream().use { input.copyTo(it) }
            }
            invoke.resolve(outcome(saved = true, name = displayName(uri)))
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "не удалось прочитать файл")
        }
    }

    private fun outcome(saved: Boolean, name: String?): JSObject {
        val result = JSObject()
        result.put("completed", saved)
        result.put("displayName", name)
        return result
    }

    /**
     * The name the user sees for the file they picked.
     *
     * Only for showing back to them — the URI, not this, is what was read or
     * written, so a missing or duplicated name changes nothing.
     */
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
