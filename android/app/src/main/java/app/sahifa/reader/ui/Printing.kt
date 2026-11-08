package app.sahifa.reader.ui

import android.content.Context
import android.os.Bundle
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.print.PageRange
import android.print.PrintAttributes
import android.print.PrintDocumentAdapter
import android.print.PrintDocumentInfo
import android.print.PrintManager
import android.webkit.WebView
import java.io.File
import java.io.FileOutputStream

/** Sends a PDF file to Android's print service as-is (vector quality). */
fun printPdf(ctx: Context, path: String, title: String, pageCount: Int) {
    val pm = ctx.getSystemService(Context.PRINT_SERVICE) as PrintManager
    val name = title.ifBlank { File(path).nameWithoutExtension }
    pm.print(name, object : PrintDocumentAdapter() {
        override fun onLayout(
            old: PrintAttributes?, new: PrintAttributes, cancel: CancellationSignal,
            cb: LayoutResultCallback, extras: Bundle?,
        ) {
            if (cancel.isCanceled) { cb.onLayoutCancelled(); return }
            val info = PrintDocumentInfo.Builder("$name.pdf")
                .setContentType(PrintDocumentInfo.CONTENT_TYPE_DOCUMENT)
                .setPageCount(if (pageCount > 0) pageCount else PrintDocumentInfo.PAGE_COUNT_UNKNOWN)
                .build()
            cb.onLayoutFinished(info, old != new)
        }

        override fun onWrite(pages: Array<out PageRange>, dest: ParcelFileDescriptor, cancel: CancellationSignal, cb: WriteResultCallback) {
            try {
                File(path).inputStream().use { input ->
                    FileOutputStream(dest.fileDescriptor).use { out -> input.copyTo(out) }
                }
                cb.onWriteFinished(arrayOf(PageRange.ALL_PAGES))
            } catch (t: Throwable) {
                cb.onWriteFailed(t.message)
            }
        }
    }, PrintAttributes.Builder().build())
}

/**
 * Prints the chapter shown in the WebView. The reader is switched to a plain,
 * light, scrolling layout while the print dialog is open, then restored.
 */
fun printWebView(ctx: Context, web: WebView, title: String, before: () -> Unit, after: () -> Unit) {
    val pm = ctx.getSystemService(Context.PRINT_SERVICE) as PrintManager
    before()
    // give the layout change a frame to apply before Chromium snapshots it
    web.postDelayed({
        val inner = web.createPrintDocumentAdapter(title)
        pm.print(title, object : PrintDocumentAdapter() {
            override fun onStart() = inner.onStart()
            override fun onLayout(o: PrintAttributes?, n: PrintAttributes, c: CancellationSignal, cb: LayoutResultCallback, e: Bundle?) =
                inner.onLayout(o, n, c, cb, e)
            override fun onWrite(p: Array<out PageRange>, d: ParcelFileDescriptor, c: CancellationSignal, cb: WriteResultCallback) =
                inner.onWrite(p, d, c, cb)
            override fun onFinish() {
                inner.onFinish()
                after()
            }
        }, PrintAttributes.Builder().build())
    }, 250)
}
