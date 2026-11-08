package app.sahifa.reader.core

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.reader_core.DocFormat
import uniffi.reader_core.DocLibrary
import uniffi.reader_core.LibraryEntry
import uniffi.reader_core.detectFormat
import uniffi.reader_core.pdfInit
import java.io.File

/** Process-wide access to the Rust core. */
object Core {
    lateinit var app: Context
        private set
    lateinit var library: DocLibrary
        private set
    var pdfError: String? = null
        private set

    fun init(ctx: Context) {
        app = ctx.applicationContext
        library = DocLibrary.open(File(app.filesDir, "library.json").path)
        try {
            pdfInit(File(app.applicationInfo.nativeLibraryDir, "libpdfium.so").path)
        } catch (t: Throwable) {
            pdfError = t.message
            Log.e("Sahifa", "PDFium init failed", t)
        }
    }

    val docsDir: File get() = File(app.filesDir, "docs").apply { mkdirs() }
    val fontsDir: File get() = File(app.filesDir, "fonts").apply { mkdirs() }

    fun setting(key: String, default: String): String = library.setting(key) ?: default
    fun setSetting(key: String, value: String) = library.setSetting(key, value)

    /** Copies a content:// document into app storage (once) and registers it. */
    suspend fun import(uri: Uri): LibraryEntry = withContext(Dispatchers.IO) {
        val cr = app.contentResolver
        var name = "document"
        var size = -1L
        if (uri.scheme == "file") {
            val f = File(uri.path ?: "")
            name = f.name
            size = f.length()
        } else {
            cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    c.getString(0)?.let { name = it }
                    if (!c.isNull(1)) size = c.getLong(1)
                }
            }
        }
        val safe = name.replace(Regex("[\\\\/:*?\"<>|\\u0000]"), "_").take(150).ifBlank { "document" }
        var dest = File(docsDir, safe)
        if (dest.exists() && size >= 0 && dest.length() != size) {
            val base = safe.substringBeforeLast('.')
            val ext = safe.substringAfterLast('.', "")
            var i = 2
            while (dest.exists()) {
                dest = File(docsDir, if (ext.isEmpty()) "$base ($i)" else "$base ($i).$ext")
                i++
            }
        }
        if (!dest.exists() || dest.length() != size) {
            val tmp = File(docsDir, ".${dest.name}.part")
            cr.openInputStream(uri)!!.use { input -> tmp.outputStream().use { input.copyTo(it, 256 * 1024) } }
            tmp.renameTo(dest)
        }
        library.touch(dest.path, name)
    }

    fun importFont(uri: Uri): String? {
        val cr = app.contentResolver
        var name = "font.ttf"
        cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0)?.let { name = it }
        }
        val lower = name.lowercase()
        if (!(lower.endsWith(".ttf") || lower.endsWith(".otf") || lower.endsWith(".woff") || lower.endsWith(".woff2"))) return null
        val dest = File(fontsDir, name.replace(Regex("[^\\p{L}\\p{N}._ -]"), "_"))
        cr.openInputStream(uri)?.use { i -> dest.outputStream().use { i.copyTo(it) } } ?: return null
        return dest.name
    }

    fun userFonts(): List<String> = fontsDir.listFiles()?.map { it.name }?.sorted() ?: emptyList()

    fun formatOf(path: String): DocFormat = detectFormat(path)
}
