package app.sahifa.reader.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.produceState
import androidx.compose.ui.platform.LocalConfiguration
import app.sahifa.reader.core.Core
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import uniffi.reader_core.LibraryEntry
import uniffi.reader_core.PdfDoc
import uniffi.reader_core.ReflowDoc
import java.io.File
import java.nio.ByteBuffer

/** True on tablets and other large screens (Material "medium"/"expanded" width). */
@Composable
fun isWideScreen(): Boolean = LocalConfiguration.current.screenWidthDp >= 600

/**
 * Cover thumbnails for the library grid: the first page of a PDF, or the
 * book's own cover (EPUB, FB2, CBZ). Cached in memory and on disk.
 */
object Covers {
    private const val WIDTH = 360
    private val mem = object : LruCache<String, Bitmap>(24 * 1024 * 1024) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }
    private val gate = Semaphore(2)
    private val dir get() = File(Core.app.cacheDir, "covers").apply { mkdirs() }

    private fun key(e: LibraryEntry): String {
        val f = File(e.path)
        return "${e.id}-${f.lastModified()}-${f.length()}"
    }

    fun cached(e: LibraryEntry): Bitmap? = mem.get(key(e))

    suspend fun load(e: LibraryEntry): Bitmap? = withContext(Dispatchers.IO) {
        val k = key(e)
        mem.get(k)?.let { return@withContext it }
        val file = File(dir, "$k.jpg")
        val bmp = if (file.exists()) {
            BitmapFactory.decodeFile(file.path)
        } else gate.withPermit {
            runCatching { make(e) }.getOrNull()?.also { b ->
                runCatching { file.outputStream().use { b.compress(Bitmap.CompressFormat.JPEG, 88, it) } }
            }
        }
        bmp?.also { mem.put(k, it) }
    }

    private fun make(e: LibraryEntry): Bitmap? = when (e.format) {
        "PDF" -> {
            val doc = PdfDoc.open(e.path, null)
            try {
                val s = doc.info().sizes.firstOrNull() ?: return null
                val h = (WIDTH * s.height / s.width).toInt().coerceIn(1, WIDTH * 3)
                val px = doc.renderTile(0, WIDTH, h, 0, 0, WIDTH, h, true)
                Bitmap.createBitmap(WIDTH, h, Bitmap.Config.ARGB_8888).apply { copyPixelsFromBuffer(ByteBuffer.wrap(px)) }
            } finally {
                doc.close()
            }
        }
        "EPUB", "FB2", "CBZ" -> {
            val doc = ReflowDoc.open(e.path)
            try {
                val data = doc.cover()?.data ?: return null
                val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(data, 0, data.size, opts)
                var sample = 1
                while (opts.outWidth / (sample * 2) >= WIDTH) sample *= 2
                BitmapFactory.decodeByteArray(data, 0, data.size, BitmapFactory.Options().apply { inSampleSize = sample })
            } finally {
                doc.close()
            }
        }
        else -> null
    }
}

@Composable
fun rememberCover(e: LibraryEntry): Bitmap? =
    produceState(Covers.cached(e), e.id, e.path) { if (value == null) value = Covers.load(e) }.value
