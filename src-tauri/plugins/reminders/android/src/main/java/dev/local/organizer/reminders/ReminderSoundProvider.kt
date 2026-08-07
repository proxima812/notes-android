package dev.local.organizer.reminders

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import java.io.File
import java.io.FileNotFoundException

/**
 * Read-only window onto the user's trimmed reminder sounds.
 *
 * A notification channel's sound is played by the system, not by this app, so
 * a plain path into `filesDir` is unreadable to it. This provider is the one
 * door: exported, but it only ever opens files that really live inside
 * `filesDir/reminder_sounds` — the canonical-path check is what stops a crafted
 * URI from walking out of that directory — and only for reading.
 */
class ReminderSoundProvider : ContentProvider() {

    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") {
            throw SecurityException("звуки напоминаний доступны только для чтения")
        }
        val file = resolve(uri) ?: throw FileNotFoundException("звук не найден: $uri")
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun query(
        uri: Uri,
        projection: Array<String>?,
        selection: String?,
        selectionArgs: Array<String>?,
        sortOrder: String?,
    ): Cursor {
        val file = resolve(uri)
        val columns = projection ?: arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)
        val cursor = MatrixCursor(columns)
        if (file != null) {
            cursor.addRow(
                columns.map { column ->
                    when (column) {
                        OpenableColumns.DISPLAY_NAME -> file.name
                        OpenableColumns.SIZE -> file.length()
                        else -> null
                    }
                },
            )
        }
        return cursor
    }

    override fun getType(uri: Uri): String = "audio/mp4"

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<String>?,
    ): Int = 0

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<String>?): Int = 0

    /** The file the URI names, or null if it is missing or points outside. */
    private fun resolve(uri: Uri): File? {
        val context = context ?: return null
        val name = uri.lastPathSegment ?: return null
        val directory = File(context.filesDir, SOUNDS_DIRECTORY)
        val file = File(directory, name)
        val inside = file.canonicalPath.startsWith(directory.canonicalPath + File.separator)
        return if (inside && file.isFile) file else null
    }

    companion object {
        const val AUTHORITY = "dev.local.organizer.reminder_sounds"
        const val SOUNDS_DIRECTORY = "reminder_sounds"

        fun uriFor(fileName: String): Uri =
            Uri.parse("content://$AUTHORITY/$fileName")

        fun directory(context: android.content.Context): File =
            File(context.filesDir, SOUNDS_DIRECTORY)
    }
}
