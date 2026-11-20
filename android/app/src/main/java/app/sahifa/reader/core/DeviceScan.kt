package app.sahifa.reader.core

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.storage.StorageManager
import android.provider.Settings
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Finds the documents on the device (internal storage and SD cards) so the
 * Files tab can list them without the user picking each one.
 *
 * Android 11+ only lets an app see other apps' PDFs/EPUBs with "All files
 * access"; Android 8–10 need the storage permission. Results are cached in
 * app storage so the list shows instantly and is refreshed in the background.
 */
object DeviceScan {
    data class Found(val path: String, val name: String, val size: Long, val modified: Long) {
        val ext: String get() = name.substringAfterLast('.', "").lowercase()
        val folder: String get() = File(path).parentFile?.name ?: ""
    }

    /** Extensions the reader opens, mapped to the label shown in the filter chips. */
    val FORMATS = linkedMapOf(
        "pdf" to "PDF", "epub" to "EPUB", "docx" to "DOCX", "odt" to "ODT", "fb2" to "FB2",
        "html" to "HTML", "htm" to "HTML", "xhtml" to "HTML", "md" to "MD", "markdown" to "MD",
        "txt" to "TXT", "cbz" to "CBZ",
    )

    /** Text-like formats are everywhere as app junk (logs, licences); skip tiny ones. */
    private val SMALL_SKIP = setOf("txt", "html", "htm", "xhtml", "md", "markdown")
    private const val MIN_TEXT_BYTES = 2 * 1024L
    private const val MAX_RESULTS = 10_000
    private const val MAX_DEPTH = 16

    private val _results = MutableStateFlow<List<Found>>(emptyList())
    val results: StateFlow<List<Found>> = _results
    private val _scanning = MutableStateFlow(false)
    val scanning: StateFlow<Boolean> = _scanning
    private val lock = Mutex()
    private var loaded = false

    private val cacheFile: File get() = File(Core.app.filesDir, "device_scan.json")

    fun hasAccess(ctx: Context): Boolean =
        if (Build.VERSION.SDK_INT >= 30) Environment.isExternalStorageManager()
        else ctx.checkSelfPermission(Manifest.permission.READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED

    /** Android 11+: the system "All files access" screen for this app (falls back to the general list). */
    fun allFilesAccessIntents(ctx: Context): List<Intent> = listOf(
        Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:${ctx.packageName}")),
        Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION),
    )

    /** Shows the last scan right away (from the cache file). */
    fun loadCached() {
        if (loaded) return
        loaded = true
        val list = runCatching {
            val arr = JSONArray(cacheFile.readText())
            List(arr.length()) { i ->
                val o = arr.getJSONObject(i)
                Found(o.getString("p"), o.getString("n"), o.optLong("s"), o.optLong("m"))
            }
        }.getOrDefault(emptyList())
        if (_results.value.isEmpty()) _results.value = list
    }

    /** Walks every storage volume; no-op if another scan is running. */
    suspend fun scan(ctx: Context) {
        if (!hasAccess(ctx) || !lock.tryLock()) return
        _scanning.value = true
        try {
            val found = withContext(Dispatchers.IO) {
                val out = ArrayList<Found>()
                for (root in roots(ctx)) walk(root, 0, out)
                out.distinctBy { it.path }.sortedByDescending { it.modified }
            }
            _results.value = found
            withContext(Dispatchers.IO) { save(found) }
        } catch (t: Throwable) {
            Log.w("Sahifa", "device scan failed", t)
        } finally {
            _scanning.value = false
            lock.unlock()
        }
    }

    private fun roots(ctx: Context): List<File> {
        val list = mutableListOf<File>()
        @Suppress("DEPRECATION")
        Environment.getExternalStorageDirectory()?.let { list += it }
        if (Build.VERSION.SDK_INT >= 30) {
            val sm = ctx.getSystemService(StorageManager::class.java)
            sm?.storageVolumes?.forEach { v -> v.directory?.let { list += it } }
        }
        return list.filter { it.canRead() }.distinctBy { runCatching { it.canonicalPath }.getOrDefault(it.path) }
    }

    private fun walk(dir: File, depth: Int, out: MutableList<Found>) {
        if (depth > MAX_DEPTH || out.size >= MAX_RESULTS) return
        val children = dir.listFiles() ?: return
        for (f in children) {
            if (out.size >= MAX_RESULTS) return
            val name = f.name
            if (name.startsWith(".")) continue
            if (f.isDirectory) {
                // Android/data and Android/obb hold other apps' private files
                if (depth == 0 && name == "Android") continue
                walk(f, depth + 1, out)
            } else {
                val ext = name.substringAfterLast('.', "").lowercase()
                if (ext !in FORMATS) continue
                val size = f.length()
                if (size <= 0 || (ext in SMALL_SKIP && size < MIN_TEXT_BYTES)) continue
                out += Found(f.path, name, size, f.lastModified())
            }
        }
    }

    private fun save(list: List<Found>) {
        val arr = JSONArray()
        list.forEach { arr.put(JSONObject().put("p", it.path).put("n", it.name).put("s", it.size).put("m", it.modified)) }
        val tmp = File(cacheFile.path + ".part")
        tmp.writeText(arr.toString())
        tmp.renameTo(cacheFile)
    }
}
