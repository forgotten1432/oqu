package app.sahifa.reader.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import uniffi.reader_core.LibraryEntry

/** Routes a document to the PDF engine (PDFium) or the reflow engine (WebView). */
@Composable
fun ViewerScreen(entry: LibraryEntry, onClose: () -> Unit) {
    key(entry.id) {
        if (entry.format == "PDF") PdfViewer(entry, onClose) else ReflowViewer(entry, onClose)
    }
}
