package app.sahifa.reader.ui

import android.graphics.Bitmap
import android.util.LruCache
import android.widget.Toast
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.automirrored.filled.MenuBook
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.BookmarkBorder
import androidx.compose.material.icons.outlined.BorderColor
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.outlined.GridView
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.sahifa.reader.R
import app.sahifa.reader.core.Core
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.reader_core.Annotation
import uniffi.reader_core.PageOp
import uniffi.reader_core.PdfEdit
import uniffi.reader_core.PdfEditKind
import uniffi.reader_core.pdfSaveEdits
import androidx.activity.compose.BackHandler
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material.icons.outlined.Draw
import androidx.compose.material.icons.outlined.FormatColorReset
import androidx.compose.material.icons.outlined.Gesture
import androidx.compose.material.icons.outlined.Print
import androidx.compose.material.icons.automirrored.outlined.RotateRight
import androidx.compose.material.icons.outlined.Save
import androidx.compose.material.icons.outlined.Title
import androidx.compose.material.icons.outlined.Today
import uniffi.reader_core.AnnotationKind
import uniffi.reader_core.LibraryEntry
import uniffi.reader_core.NormRect
import uniffi.reader_core.OutlineItem
import uniffi.reader_core.PdfDoc
import uniffi.reader_core.PdfInfo
import uniffi.reader_core.PdfLink
import uniffi.reader_core.PdfSearchHit
import uniffi.reader_core.ReaderException
import uniffi.reader_core.TextDirection as CoreDir
import java.nio.ByteBuffer
import java.util.concurrent.Executors
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** Single render thread: requests that get cancelled while queued never reach PDFium. */
private val renderDispatcher = Executors.newSingleThreadExecutor { r -> Thread(r, "pdf-render").apply { priority = Thread.NORM_PRIORITY + 1 } }.asCoroutineDispatcher()
private val textDispatcher = Executors.newSingleThreadExecutor { r -> Thread(r, "pdf-text") }.asCoroutineDispatcher()

private val bitmapCache = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 4).toInt()) {
    override fun sizeOf(key: String, value: Bitmap) = value.byteCount
}

class PdfSession(val doc: PdfDoc, val info: PdfInfo, val entry: LibraryEntry, val version: Int = 0) {
    private val best = HashMap<Int, Bitmap>()

    fun aspect(page: Int): Float {
        val s = info.sizes.getOrNull(page) ?: return 1.414f
        return if (s.width > 0) s.height / s.width else 1.414f
    }

    fun cached(page: Int, w: Int): Bitmap? = bitmapCache.get("${entry.id}:v$version:$page@$w")
    fun anyCached(page: Int): Bitmap? = synchronized(best) { best[page]?.takeIf { !it.isRecycled } }

    suspend fun render(page: Int, w: Int): Bitmap {
        cached(page, w)?.let { return it }
        return withContext(renderDispatcher) {
            cached(page, w) ?: run {
                val h = max(1, (w * aspect(page)).roundToInt())
                val bytes = doc.renderTile(page, w, h, 0, 0, w, h, true)
                val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
                bmp.copyPixelsFromBuffer(ByteBuffer.wrap(bytes))
                bitmapCache.put("${entry.id}:v$version:$page@$w", bmp)
                synchronized(best) { best[page] = bmp }
                bmp
            }
        }
    }

    private val linkCache = HashMap<Int, List<PdfLink>>()
    suspend fun links(page: Int): List<PdfLink> = linkCache[page] ?: withContext(textDispatcher) {
        runCatching { doc.links(page) }.getOrDefault(emptyList()).also { linkCache[page] = it }
    }
}

private val invertMatrix = ColorMatrix(
    floatArrayOf(
        -0.9f, 0f, 0f, 0f, 240f,
        0f, -0.9f, 0f, 0f, 240f,
        0f, 0f, -0.9f, 0f, 240f,
        0f, 0f, 0f, 1f, 0f,
    )
)

private enum class Tool { None, Highlight, Note, Sign }

private val LocalEdit = staticCompositionLocalOf<EditState?> { null }

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun PdfViewer(entry: LibraryEntry, onClose: () -> Unit) {
    val ctx = LocalContext.current
    var session by remember { mutableStateOf<PdfSession?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var needPassword by remember { mutableStateOf(false) }
    var password by remember { mutableStateOf<String?>(null) }
    var version by remember { mutableIntStateOf(0) }

    LaunchedEffect(password, version) {
        try {
            val fresh = Core.library.get(entry.id) ?: entry
            val s = withContext(textDispatcher) {
                val d = PdfDoc.open(entry.path, password)
                PdfSession(d, d.info(), fresh, version)
            }
            Core.library.setMeta(entry.id, s.info.title, s.info.author, s.info.pageCount)
            session = s
        } catch (e: ReaderException.Password) {
            needPassword = true
        } catch (t: Throwable) {
            error = t.message ?: t.toString()
        }
    }
    DisposableEffect(Unit) { onDispose { session?.doc?.close() } }

    when {
        needPassword && session == null -> {
            Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background))
            PasswordDialog(onSubmit = { needPassword = false; password = it }, onDismiss = onClose)
        }
        error != null -> ErrorScreen(error!!, onClose)
        session == null -> LoadingScreen()
        else -> key(version) {
            PdfReader(session!!, password, onClose, onReload = {
                session = null
                version++
            })
        }
    }
}

@Composable
fun LoadingScreen() {
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
}

@Composable
fun ErrorScreen(msg: String, onClose: () -> Unit) {
    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(stringResource(R.string.error_open), style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(8.dp))
        Text(msg, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(16.dp))
        Button(onClick = onClose) { Text(stringResource(R.string.back)) }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
private fun PdfReader(s: PdfSession, password: String?, onClose: () -> Unit, onReload: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val count = s.info.pageCount
    val startPage = remember { s.entry.position?.removePrefix("page:")?.substringBefore(':')?.toIntOrNull()?.coerceIn(0, max(0, count - 1)) ?: 0 }

    var chrome by remember { mutableStateOf(true) }
    var night by remember { mutableStateOf(Core.setting("pdf_night", "0") == "1") }
    var single by remember { mutableStateOf(Core.setting("pdf_single", "0") == "1") }
    var rtlPages by remember { mutableStateOf(s.info.direction == CoreDir.RTL) }
    var readingMode by remember { mutableStateOf(false) }
    var currentPage by remember { mutableIntStateOf(startPage) }
    var tool by remember { mutableStateOf(Tool.None) }
    var sheet by remember { mutableStateOf<String?>(null) }
    var goTo by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var linkUri by remember { mutableStateOf<String?>(null) }
    var noteAt by remember { mutableStateOf<Pair<Int, Offset>?>(null) }
    var openNote by remember { mutableStateOf<Annotation?>(null) }
    var annotations by remember { mutableStateOf(Core.library.annotations(s.entry.id)) }
    fun reloadAnn() { annotations = Core.library.annotations(s.entry.id) }
    val edit = remember { EditState() }
    var saving by remember { mutableStateOf(false) }
    var textDialog by remember { mutableStateOf(false) }
    var signDialog by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    var confirmLeave by remember { mutableStateOf(false) }
    val savedMsg = stringResource(R.string.saved)
    val failedMsg = stringResource(R.string.edit_failed)

    /**
     * Writes pending stamps, the in-app highlights/notes and page operations
     * into the PDF (via the Rust core), replaces the file and reloads.
     */
    fun commit(ops: List<PageOp>, then: () -> Unit = {}) {
        if (saving) return
        saving = true
        scope.launch {
            val anns = Core.library.annotations(s.entry.id)
            val embed = anns.filter { it.kind == AnnotationKind.HIGHLIGHT || it.kind == AnnotationKind.NOTE }
            val edits = mutableListOf<PdfEdit>()
            for (st in edit.stamps) {
                val kind = st.bitmap?.let { PdfEditKind.Image(it.toRgba(), it.width.toUInt(), it.height.toUInt()) } ?: PdfEditKind.WhiteOut
                edits += PdfEdit(st.page, st.rect, kind)
            }
            for (a in embed) {
                val page = a.position.removePrefix("page:").substringBefore(':').toIntOrNull() ?: continue
                if (a.kind == AnnotationKind.HIGHLIGHT) {
                    parseRect(a.position)?.let { edits += PdfEdit(page, it, PdfEditKind.Highlight(0xFFFFD54Fu)) }
                } else {
                    parsePoint(a.position)?.let { edits += PdfEdit(page, NormRect(it.x, it.y, it.x, it.y), PdfEditKind.Note(a.note ?: "", 0xFFFFC107u)) }
                }
            }
            val src = java.io.File(s.entry.path)
            val tmp = java.io.File(src.parentFile, ".${src.name}.saving")
            try {
                withContext(textDispatcher) { pdfSaveEdits(src.path, password, tmp.path, edits, ops) }
                s.doc.release()
                if (!tmp.renameTo(src)) throw java.io.IOException("rename failed")
                embed.forEach { Core.library.removeAnnotation(it.id) }
                // keep bookmarks pointing at the right pages after deletions
                ops.filterIsInstance<PageOp.Delete>().sortedByDescending { it.page }.forEach { del ->
                    Core.library.annotations(s.entry.id).filter { it.kind == AnnotationKind.BOOKMARK }.forEach { b ->
                        val p = b.position.removePrefix("page:").toIntOrNull() ?: return@forEach
                        if (p >= del.page) {
                            Core.library.removeAnnotation(b.id)
                            if (p > del.page) Core.library.addAnnotation(s.entry.id, AnnotationKind.BOOKMARK, "page:${p - 1}", b.text, b.note, b.color)
                        }
                    }
                }
                Core.library.setProgress(s.entry.id, "page:${currentPage.coerceAtMost(max(0, count - 1 - ops.count { it is PageOp.Delete }))}", s.entry.progress)
                edit.stamps.clear()
                Toast.makeText(ctx, savedMsg, Toast.LENGTH_SHORT).show()
                then()
                onReload()
            } catch (t: Throwable) {
                tmp.delete()
                Toast.makeText(ctx, failedMsg + ": " + (t.message ?: t), Toast.LENGTH_LONG).show()
            } finally {
                saving = false
            }
        }
    }
    BackHandler(enabled = edit.dirty) { confirmLeave = true }

    // search
    var searchOpen by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    val hits = remember { mutableStateListOf<PdfSearchHit>() }
    var hitIndex by remember { mutableIntStateOf(0) }
    var searching by remember { mutableStateOf(false) }
    var searchJob by remember { mutableStateOf<Job?>(null) }

    val listState = rememberLazyListState(initialFirstVisibleItemIndex = startPage)
    val pagerState = rememberPagerState(initialPage = startPage) { count }

    fun jump(page: Int) {
        val p = page.coerceIn(0, count - 1)
        scope.launch { if (single) pagerState.scrollToPage(p) else listState.scrollToItem(p) }
        currentPage = p
    }

    fun runSearch() {
        searchJob?.cancel()
        hits.clear(); hitIndex = 0
        val q = query.trim()
        if (q.isEmpty()) return
        searching = true
        searchJob = scope.launch {
            val order = (currentPage until count) + (0 until currentPage)
            var jumped = false
            for (p in order) {
                val found = withContext(textDispatcher) { runCatching { s.doc.searchPage(p, q) }.getOrDefault(emptyList()) }
                if (found.isNotEmpty()) {
                    hits.addAll(found)
                    if (!jumped) { jumped = true; jump(found[0].page) }
                }
            }
            searching = false
        }
    }

    // track page + persist progress
    LaunchedEffect(single) {
        if (single) {
            snapshotFlow { pagerState.currentPage }.distinctUntilChanged().collect { currentPage = it }
        } else {
            snapshotFlow {
                val info = listState.layoutInfo
                val center = info.viewportEndOffset / 2
                info.visibleItemsInfo.firstOrNull { it.offset <= center && it.offset + it.size > center }?.index ?: listState.firstVisibleItemIndex
            }.distinctUntilChanged().collect { currentPage = it }
        }
    }
    LaunchedEffect(currentPage) {
        delay(400)
        Core.library.setProgress(s.entry.id, "page:$currentPage", if (count > 0) (currentPage + 1f) / count else 0f)
    }
    var pillVisible by remember { mutableStateOf(true) }
    LaunchedEffect(currentPage, chrome) { pillVisible = true; delay(2500); if (!chrome) pillVisible = false }

    val pageHits = remember(hits.size) { hits.groupBy { it.page } }
    val currentHit = hits.getOrNull(hitIndex)
    val bookmarked = annotations.any { it.kind == AnnotationKind.BOOKMARK && it.position == "page:$currentPage" }

    val pageCallbacks = PageCallbacks(
        onTap = { page, pos, sizePx ->
            if (tool == Tool.Sign) {
                if (edit.tool == SignTool.Place && edit.pending != null) {
                    val sz = s.info.sizes.getOrNull(page)
                    edit.place(page, Offset(pos.x / sizePx.width, pos.y / sizePx.height), sz?.width ?: 612f, sizePx.height / sizePx.width)
                    edit.tool = SignTool.None
                    edit.pending = null
                } else {
                    edit.selected = null
                }
                return@PageCallbacks
            }
            if (tool == Tool.Note) {
                noteAt = page to Offset(pos.x / sizePx.width, pos.y / sizePx.height)
                return@PageCallbacks
            }
            scope.launch {
                val nx = pos.x / sizePx.width
                val ny = pos.y / sizePx.height
                val note = annotations.firstOrNull { a ->
                    a.kind == AnnotationKind.NOTE && a.position.startsWith("page:$page:") &&
                        parsePoint(a.position)?.let { (it.x - nx) * sizePx.width in -60f..60f && (it.y - ny) * sizePx.height in -60f..60f } == true
                }
                if (note != null) { openNote = note; return@launch }
                val link = s.links(page).firstOrNull { nx in it.rect.left..it.rect.right && ny in it.rect.top..it.rect.bottom }
                when {
                    link?.page != null -> jump(link.page!!)
                    link?.uri != null -> linkUri = link.uri
                    else -> chrome = !chrome
                }
            }
        },
        onHighlight = { page, rect ->
            if (tool == Tool.Sign) {
                edit.stamps += Stamp(edit.newId(), page, rect, null)
                edit.tool = SignTool.None
                return@PageCallbacks
            }
            scope.launch {
                val text = withContext(textDispatcher) { runCatching { s.doc.textInRect(page, rect) }.getOrDefault("") }
                Core.library.addAnnotation(
                    s.entry.id, AnnotationKind.HIGHLIGHT,
                    "page:$page:${rect.left},${rect.top},${rect.right},${rect.bottom}", text.ifBlank { null }, null, 0x66FFD54Fu,
                )
                reloadAnn()
            }
        },
    )

    Box(Modifier.fillMaxSize().background(if (night) Color(0xFF111111) else Color(0xFFE6E6E8))) {
        CompositionLocalProvider(LocalEdit provides edit) { Column(Modifier.fillMaxSize()) {
            // leave room for the bar so the first page isn't hidden
            if (readingMode) {
                Spacer(Modifier.statusBarsPadding().height(64.dp))
                PdfReadingMode(s, currentPage, night)
            } else if (single) {
                CompositionLocalProvider(androidx.compose.ui.platform.LocalLayoutDirection provides if (rtlPages) LayoutDirection.Rtl else LayoutDirection.Ltr) {
                    HorizontalPager(state = pagerState, modifier = Modifier.fillMaxSize(), beyondViewportPageCount = 1, key = { it }) { page ->
                        CompositionLocalProvider(androidx.compose.ui.platform.LocalLayoutDirection provides LayoutDirection.Ltr) {
                            SinglePage(s, page, night, pageHits[page], currentHit, annotations, tool, pageCallbacks)
                        }
                    }
                }
            } else {
                ContinuousPages(s, listState, night, pageHits, currentHit, annotations, tool, pageCallbacks)
            }
        } }

        if (tool == Tool.Highlight || tool == Tool.Note) {
            Box(Modifier.align(Alignment.TopCenter).statusBarsPadding().padding(top = 64.dp)) {
                HintBanner(stringResource(if (tool == Tool.Highlight) R.string.highlight_mode_hint else R.string.note_mode_hint)) { tool = Tool.None }
            }
        }
        if (tool == Tool.Sign && edit.tool != SignTool.None) {
            Box(Modifier.align(Alignment.TopCenter).statusBarsPadding().padding(top = 64.dp)) {
                HintBanner(stringResource(if (edit.tool == SignTool.WhiteOut) R.string.whiteout_hint else R.string.place_hint)) {
                    edit.tool = SignTool.None; edit.pending = null
                }
            }
        }
        if (saving) LinearProgressIndicator(Modifier.fillMaxWidth().align(Alignment.TopCenter).statusBarsPadding())

        PagePill(pillVisible && !readingMode, "${currentPage + 1} / $count") { goTo = true }

        ViewerTopBar(
            visible = chrome,
            title = s.entry.title ?: s.info.title ?: s.entry.displayName,
            onBack = onClose,
            search = if (searchOpen) SearchUi(
                query = query, onQuery = { query = it }, onSubmit = { runSearch() },
                current = hitIndex, total = hits.size, busy = searching,
                onPrev = { if (hits.isNotEmpty()) { hitIndex = (hitIndex - 1 + hits.size) % hits.size; jump(hits[hitIndex].page) } },
                onNext = { if (hits.isNotEmpty()) { hitIndex = (hitIndex + 1) % hits.size; jump(hits[hitIndex].page) } },
                hint = if (s.info.language == "ug") stringResource(R.string.search_hint_uyghur) else null,
                onClose = { searchOpen = false; searchJob?.cancel(); hits.clear(); query = "" },
            ) else null,
        ) {
            IconButton(onClick = { searchOpen = true }) { Icon(Icons.Filled.Search, stringResource(R.string.search)) }
            IconButton(onClick = { readingMode = !readingMode }) {
                Icon(Icons.AutoMirrored.Filled.MenuBook, stringResource(R.string.reading_mode), tint = if (readingMode) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
            }
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, stringResource(R.string.more)) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    CheckItem(stringResource(R.string.night_mode), night) { night = it; Core.setSetting("pdf_night", if (it) "1" else "0") }
                    CheckItem(stringResource(R.string.view_continuous), !single) { if (it) { single = false; Core.setSetting("pdf_single", "0"); scope.launch { listState.scrollToItem(currentPage) } } }
                    CheckItem(stringResource(R.string.view_single), single) { if (it) { single = true; Core.setSetting("pdf_single", "1"); scope.launch { pagerState.scrollToPage(currentPage) } } }
                    CheckItem(stringResource(R.string.page_direction_rtl), rtlPages) { rtlPages = it }
                    DropdownMenuItem(text = { Text(stringResource(R.string.go_to_page)) }, onClick = { menu = false; goTo = true })
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.annotations)) }, leadingIcon = { Icon(Icons.AutoMirrored.Outlined.Notes, null) },
                        onClick = { menu = false; sheet = "ann" },
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.rotate_page)) }, leadingIcon = { Icon(Icons.AutoMirrored.Outlined.RotateRight, null) },
                        onClick = { menu = false; commit(listOf(PageOp.Rotate(currentPage, 1))) },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.delete_page)) }, leadingIcon = { Icon(Icons.Outlined.DeleteOutline, null) },
                        enabled = count > 1,
                        onClick = { menu = false; confirmDelete = true },
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.print)) }, leadingIcon = { Icon(Icons.Outlined.Print, null) },
                        onClick = {
                            menu = false
                            if (edit.dirty) commit(emptyList()) { printPdf(ctx, s.entry.path, s.entry.title ?: s.entry.displayName, count) }
                            else printPdf(ctx, s.entry.path, s.entry.title ?: s.entry.displayName, count)
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.share)) }, leadingIcon = { Icon(Icons.Outlined.Share, null) },
                        onClick = { menu = false; shareFile(ctx, s.entry.path, "application/pdf") },
                    )
                }
            }
        }

        BottomTools(chrome && !searchOpen && tool == Tool.Sign, scrollable = true) {
            ToolButton(Icons.Outlined.Title, stringResource(R.string.add_text)) { textDialog = true }
            ToolButton(Icons.Outlined.Gesture, stringResource(R.string.signature)) { signDialog = true }
            ToolButton(Icons.Outlined.Today, stringResource(R.string.date)) {
                val d = java.text.DateFormat.getDateInstance(java.text.DateFormat.MEDIUM, java.util.Locale.getDefault()).format(java.util.Date())
                edit.pending = renderTextBitmap(d, 0xFF111111.toInt()); edit.pendingWidthPt = 14f * edit.pending!!.width / 128f; edit.tool = SignTool.Place
            }
            ToolButton(Icons.Outlined.Check, "✓") {
                edit.pending = renderTextBitmap("✓", 0xFF111111.toInt(), 160f); edit.pendingWidthPt = 16f; edit.tool = SignTool.Place
            }
            ToolButton(Icons.Outlined.Close, "✗") {
                edit.pending = renderTextBitmap("✗", 0xFF111111.toInt(), 160f); edit.pendingWidthPt = 16f; edit.tool = SignTool.Place
            }
            ToolButton(Icons.Outlined.FormatColorReset, stringResource(R.string.whiteout), selected = edit.tool == SignTool.WhiteOut) {
                edit.tool = if (edit.tool == SignTool.WhiteOut) SignTool.None else SignTool.WhiteOut
            }
            ToolButton(Icons.Outlined.Save, stringResource(R.string.save)) { commit(emptyList()) { tool = Tool.None } }
            ToolButton(Icons.Outlined.Close, stringResource(R.string.done)) {
                if (edit.dirty) confirmLeave = true else tool = Tool.None
            }
        }

        BottomTools(chrome && !searchOpen && tool != Tool.Sign) {
            ToolButton(Icons.AutoMirrored.Filled.List, stringResource(R.string.outline)) { sheet = "outline" }
            ToolButton(Icons.Outlined.GridView, stringResource(R.string.thumbnails)) { sheet = "thumbs" }
            ToolButton(Icons.Outlined.BorderColor, stringResource(R.string.highlight), selected = tool == Tool.Highlight) {
                tool = if (tool == Tool.Highlight) Tool.None else Tool.Highlight
                if (tool != Tool.None && readingMode) readingMode = false
            }
            ToolButton(Icons.Outlined.ChatBubbleOutline, stringResource(R.string.note), selected = tool == Tool.Note) {
                tool = if (tool == Tool.Note) Tool.None else Tool.Note
                if (tool != Tool.None && readingMode) readingMode = false
            }
            ToolButton(if (bookmarked) Icons.Filled.Bookmark else Icons.Outlined.BookmarkBorder, stringResource(R.string.bookmarks), selected = bookmarked) {
                val existing = annotations.firstOrNull { it.kind == AnnotationKind.BOOKMARK && it.position == "page:$currentPage" }
                if (existing != null) Core.library.removeAnnotation(existing.id)
                else Core.library.addAnnotation(s.entry.id, AnnotationKind.BOOKMARK, "page:$currentPage", null, null, 0u)
                reloadAnn()
            }
            ToolButton(Icons.Outlined.Draw, stringResource(R.string.fill_sign)) {
                tool = Tool.Sign
                readingMode = false
            }
        }

        if (sheet != null) {
            ModalBottomSheet(onDismissRequest = { sheet = null }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = sheet == "thumbs")) {
                when (sheet) {
                    "outline" -> OutlineSheet(s) { jump(it); sheet = null }
                    "thumbs" -> ThumbSheet(s, currentPage, night) { jump(it); sheet = null }
                    else -> AnnotationSheet(annotations, onGo = { a ->
                        a.position.removePrefix("page:").substringBefore(':').toIntOrNull()?.let(::jump); sheet = null
                    }, onDelete = { Core.library.removeAnnotation(it.id); reloadAnn() })
                }
            }
        }
        if (goTo) GoToPageDialog(count, onGo = ::jump, onDismiss = { goTo = false })
        if (textDialog) TextStampDialog(onDone = { t, c ->
            textDialog = false
            edit.pending = renderTextBitmap(t, c); edit.pendingWidthPt = 12f * edit.pending!!.width / 128f; edit.tool = SignTool.Place
        }, onDismiss = { textDialog = false })
        if (signDialog) SignaturePadDialog(onDone = { bmp ->
            signDialog = false
            edit.pending = bmp; edit.pendingWidthPt = 150f; edit.tool = SignTool.Place
        }, onDismiss = { signDialog = false })
        if (confirmDelete) AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(stringResource(R.string.delete_page)) },
            text = { Text(stringResource(R.string.delete_page_confirm, currentPage + 1)) },
            confirmButton = { TextButton(onClick = { confirmDelete = false; commit(listOf(PageOp.Delete(currentPage))) }) { Text(stringResource(R.string.delete)) } },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(stringResource(R.string.cancel)) } },
        )
        if (confirmLeave) AlertDialog(
            onDismissRequest = { confirmLeave = false },
            title = { Text(stringResource(R.string.unsaved_title)) },
            text = { Text(stringResource(R.string.unsaved_body)) },
            confirmButton = { TextButton(onClick = { confirmLeave = false; commit(emptyList()) { tool = Tool.None } }) { Text(stringResource(R.string.save)) } },
            dismissButton = { TextButton(onClick = { confirmLeave = false; edit.stamps.clear(); edit.tool = SignTool.None; tool = Tool.None }) { Text(stringResource(R.string.discard)) } },
        )
        linkUri?.let { LinkDialog(it, ctx) { linkUri = null } }
        noteAt?.let { (page, p) ->
            NoteDialog(onSave = { text ->
                Core.library.addAnnotation(s.entry.id, AnnotationKind.NOTE, "page:$page:${p.x},${p.y}", null, text, 0xFFFFC107u)
                reloadAnn()
            }, onDismiss = { noteAt = null; tool = Tool.None })
        }
        openNote?.let { n ->
            NoteDialog(initial = n.note ?: "", onSave = { text ->
                Core.library.removeAnnotation(n.id)
                Core.library.addAnnotation(s.entry.id, AnnotationKind.NOTE, n.position, null, text, n.color)
                reloadAnn()
            }, onDismiss = { openNote = null }, onDelete = { Core.library.removeAnnotation(n.id); reloadAnn() })
        }
    }
}

@Composable
private fun CheckItem(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        trailingIcon = { Checkbox(checked = checked, onCheckedChange = null) },
        onClick = { onChange(!checked) },
    )
}

private class PageCallbacks(
    val onTap: (page: Int, pos: Offset, size: Size) -> Unit,
    val onHighlight: (page: Int, rect: NormRect) -> Unit,
)

private fun parsePoint(pos: String): Offset? {
    val parts = pos.split(':')
    if (parts.size < 3) return null
    val xy = parts[2].split(',').mapNotNull { it.toFloatOrNull() }
    return if (xy.size == 2) Offset(xy[0], xy[1]) else null
}

private fun parseRect(pos: String): NormRect? {
    val parts = pos.split(':')
    if (parts.size < 3) return null
    val v = parts[2].split(',').mapNotNull { it.toFloatOrNull() }
    return if (v.size == 4) NormRect(v[0], v[1], v[2], v[3]) else null
}

/** Continuous vertical scroll with pinch-zoom that re-lays out (not just scales) pages. */
@Composable
private fun ContinuousPages(
    s: PdfSession,
    listState: androidx.compose.foundation.lazy.LazyListState,
    night: Boolean,
    pageHits: Map<Int, List<PdfSearchHit>>,
    currentHit: PdfSearchHit?,
    annotations: List<Annotation>,
    tool: Tool,
    cb: PageCallbacks,
) {
    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    var zoom by remember { mutableFloatStateOf(1f) }
    val hScroll = rememberScrollState()
    var pendingH by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(zoom, pendingH) { pendingH?.let { hScroll.scrollTo(it); pendingH = null } }

    BoxWithConstraints(
        Modifier.fillMaxSize().clipToBounds().pointerInput(Unit) {
            awaitEachGesture {
                awaitFirstDown(requireUnconsumed = false)
                do {
                    val ev = awaitPointerEvent(PointerEventPass.Initial)
                    if (ev.changes.count { it.pressed } >= 2) {
                        val zc = ev.calculateZoom()
                        val c = ev.calculateCentroid(useCurrent = true)
                        if (zc != 1f && c != Offset.Unspecified) {
                            val nz = (zoom * zc).coerceIn(1f, 6f)
                            val ratio = nz / zoom
                            if (ratio != 1f) {
                                zoom = nz
                                pendingH = ((hScroll.value + c.x) * ratio - c.x).roundToInt().coerceAtLeast(0)
                                val dy = (c.y + listState.firstVisibleItemScrollOffset) * (ratio - 1f)
                                scope.launch { listState.scrollBy(dy) }
                            }
                        }
                        ev.changes.forEach { it.consume() }
                    }
                } while (ev.changes.any { it.pressed })
            }
        }
    ) {
        val viewportW = constraints.maxWidth
        val pageW = (viewportW * zoom).roundToInt()
        val pageWDp = with(density) { pageW.toDp() }
        val renderCap = min(4096, (viewportW * 2.5f).roundToInt())
        Box(Modifier.fillMaxSize().horizontalScroll(hScroll, enabled = zoom > 1.01f)) {
            LazyColumn(
                state = listState,
                modifier = Modifier.width(pageWDp).fillMaxHeight(),
                contentPadding = WindowInsets.systemBars.asPaddingValues().let { PaddingValues(top = it.calculateTopPadding() + 64.dp, bottom = it.calculateBottomPadding() + 96.dp) },
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(s.info.pageCount, key = { it }) { page ->
                    val hDp = pageWDp * s.aspect(page)
                    PageView(
                        s, page, pageW, renderCap, night,
                        pageHits[page], currentHit, annotations, tool, cb,
                        Modifier.width(pageWDp).height(hDp),
                        onDoubleTap = { zoom = if (zoom > 1.2f) 1f else 2.5f },
                    )
                }
            }
        }
    }
}

@Composable
private fun SinglePage(
    s: PdfSession, page: Int, night: Boolean, hits: List<PdfSearchHit>?, currentHit: PdfSearchHit?,
    annotations: List<Annotation>, tool: Tool, cb: PageCallbacks,
) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    val ts = rememberTransformableState { zc, pan, _ ->
        scale = (scale * zc).coerceIn(1f, 5f)
        offset = if (scale == 1f) Offset.Zero else offset + pan
    }
    BoxWithConstraints(
        Modifier.fillMaxSize().clipToBounds()
            .transformable(ts, canPan = { scale > 1f })
            .padding(top = 64.dp),
        contentAlignment = Alignment.Center,
    ) {
        val maxW = constraints.maxWidth
        val maxH = constraints.maxHeight
        val aspect = s.aspect(page)
        val w = min(maxW.toFloat(), maxH / aspect).roundToInt()
        val density = LocalDensity.current
        val wDp = with(density) { w.toDp() }
        PageView(
            s, page, (w * scale).roundToInt(), 4096, night, hits, currentHit, annotations, tool, cb,
            Modifier.width(wDp).height(wDp * aspect).graphicsLayer {
                scaleX = scale; scaleY = scale; translationX = offset.x; translationY = offset.y
            },
            onDoubleTap = { if (scale > 1f) { scale = 1f; offset = Offset.Zero } else scale = 2.5f },
        )
    }
}

@Composable
private fun PageView(
    s: PdfSession,
    page: Int,
    widthPx: Int,
    renderCap: Int,
    night: Boolean,
    hits: List<PdfSearchHit>?,
    currentHit: PdfSearchHit?,
    annotations: List<Annotation>,
    tool: Tool,
    cb: PageCallbacks,
    modifier: Modifier,
    onDoubleTap: () -> Unit,
) {
    // Bucket the render width so zooming doesn't re-render every frame.
    val target = min(renderCap, max(256, (ceil(widthPx / 256f) * 256).toInt()))
    var bmp by remember(page) { mutableStateOf(s.cached(page, target) ?: s.anyCached(page)) }
    LaunchedEffect(page, target) {
        if (bmp?.width == target) return@LaunchedEffect
        if (bmp != null) delay(160)
        bmp = runCatching { s.render(page, target) }.getOrNull() ?: bmp
    }
    val pageAnn = remember(annotations, page) { annotations.filter { it.position.startsWith("page:$page:") } }
    var dragRect by remember { mutableStateOf<Rect?>(null) }

    val edit = LocalEdit.current
    val dragTool = tool == Tool.Highlight || (tool == Tool.Sign && edit?.tool == SignTool.WhiteOut)
    val gestures = when {
        dragTool -> Modifier.pointerInput(tool, page, edit?.tool) {
            var start = Offset.Zero
            detectDragGestures(
                onDragStart = { start = it; dragRect = Rect(it, it) },
                onDrag = { ch, _ -> dragRect = Rect(start, ch.position) ; ch.consume() },
                onDragEnd = {
                    dragRect?.let { r ->
                        val n = NormRect(
                            min(r.left, r.right) / size.width, min(r.top, r.bottom) / size.height,
                            max(r.left, r.right) / size.width, max(r.top, r.bottom) / size.height,
                        )
                        if (n.right - n.left > 0.01f && n.bottom - n.top > 0.005f) cb.onHighlight(page, n)
                    }
                    dragRect = null
                },
                onDragCancel = { dragRect = null },
            )
        }
        else -> Modifier.pointerInput(tool, page, edit?.tool) {
            detectTapGestures(
                onTap = { cb.onTap(page, it, Size(size.width.toFloat(), size.height.toFloat())) },
                onDoubleTap = { onDoubleTap() },
            )
        }
    }

    Box(modifier.shadow(2.dp).background(if (night) Color(0xFF1E1E1E) else Color.White).then(gestures)) {
        bmp?.let {
            Image(
                it.asImageBitmap(), contentDescription = null, contentScale = ContentScale.FillBounds,
                colorFilter = if (night) ColorFilter.colorMatrix(invertMatrix) else null,
                modifier = Modifier.fillMaxSize(),
            )
        } ?: Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("${page + 1}", color = Color.Gray)
        }
        Canvas(Modifier.fillMaxSize()) {
            fun r(n: NormRect) = Rect(n.left * size.width, n.top * size.height, n.right * size.width, n.bottom * size.height)
            for (a in pageAnn) {
                when (a.kind) {
                    AnnotationKind.HIGHLIGHT -> parseRect(a.position)?.let { n ->
                        val rr = r(n)
                        drawRect(Color(0x66FFD54F), rr.topLeft, rr.size)
                    }
                    AnnotationKind.NOTE -> parsePoint(a.position)?.let { p ->
                        val c = Offset(p.x * size.width, p.y * size.height)
                        drawCircle(Color(0xFFFFC107), 14.dp.toPx(), c)
                        drawCircle(Color(0xFF8D6E00), 14.dp.toPx(), c, style = Stroke(1.5.dp.toPx()))
                    }
                    else -> {}
                }
            }
            hits?.forEach { h ->
                val cur = h === currentHit
                h.rects.forEach { n ->
                    val rr = r(n)
                    val pad = 2f
                    drawRect(if (cur) Color(0x99FF6D00) else Color(0x66FFEB3B), Offset(rr.left - pad, rr.top - pad), Size(rr.width + 2 * pad, rr.height + 2 * pad))
                }
            }
            dragRect?.let { d ->
                drawRect(Color(0x55FFD54F), Offset(min(d.left, d.right), min(d.top, d.bottom)), Size(kotlin.math.abs(d.width), kotlin.math.abs(d.height)))
            }
        }
        if (edit != null) StampLayer(edit, page)
        if (annotations.any { it.kind == AnnotationKind.BOOKMARK && it.position == "page:$page" }) {
            Icon(Icons.Filled.Bookmark, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.align(Alignment.TopEnd).padding(6.dp).size(22.dp))
        }
    }
}

@Composable
private fun OutlineSheet(s: PdfSession, onGo: (Int) -> Unit) {
    val items by produceState<List<OutlineItem>?>(null) { value = withContext(textDispatcher) { runCatching { s.doc.outline() }.getOrDefault(emptyList()) } }
    Text(stringResource(R.string.outline), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
    when {
        items == null -> LinearProgressIndicator(Modifier.fillMaxWidth().padding(16.dp))
        items!!.isEmpty() -> Text(stringResource(R.string.no_results), Modifier.padding(20.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
        else -> LazyColumn(Modifier.fillMaxHeight(0.8f)) {
            items(items!!) { o ->
                Row(
                    Modifier.fillMaxWidth().clickable(enabled = o.page >= 0) { onGo(o.page) }
                        .padding(start = (20 + 16 * o.level.toInt()).dp, end = 20.dp, top = 12.dp, bottom = 12.dp),
                ) {
                    Text(
                        o.title, Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.bodyLarge.copy(textDirection = TextDirection.Content),
                        fontWeight = if (o.level == 0u) FontWeight.Medium else FontWeight.Normal,
                    )
                    if (o.page >= 0) Text("${o.page + 1}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
private fun ThumbSheet(s: PdfSession, current: Int, night: Boolean, onGo: (Int) -> Unit) {
    val grid = rememberLazyGridState(initialFirstVisibleItemIndex = current)
    LazyVerticalGrid(GridCells.Adaptive(104.dp), state = grid, modifier = Modifier.fillMaxHeight(0.92f), contentPadding = PaddingValues(12.dp)) {
        itemsIndexed(List(s.info.pageCount) { it }) { _, page ->
            Column(Modifier.padding(6.dp).clickable { onGo(page) }, horizontalAlignment = Alignment.CenterHorizontally) {
                var bmp by remember(page) { mutableStateOf(s.cached(page, 256) ?: s.anyCached(page)) }
                LaunchedEffect(page) { if (bmp == null) bmp = runCatching { s.render(page, 256) }.getOrNull() }
                Box(
                    Modifier.fillMaxWidth().aspectRatio(1f / s.aspect(page)).background(Color.White)
                        .border(if (page == current) 2.dp else 0.5.dp, if (page == current) MaterialTheme.colorScheme.primary else Color(0x33000000)),
                ) {
                    bmp?.let { Image(it.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds, colorFilter = if (night) ColorFilter.colorMatrix(invertMatrix) else null) }
                }
                Text("${page + 1}", style = MaterialTheme.typography.labelSmall, modifier = Modifier.padding(top = 4.dp))
            }
        }
    }
}

@Composable
fun AnnotationSheet(list: List<Annotation>, onGo: (Annotation) -> Unit, onDelete: (Annotation) -> Unit, label: (Annotation) -> String = { a ->
    val p = a.position.removePrefix("page:").substringBefore(':').toIntOrNull()
    if (p != null) "${p + 1}" else ""
}) {
    Text(stringResource(R.string.annotations), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
    if (list.isEmpty()) {
        Text(stringResource(R.string.no_annotations), Modifier.padding(20.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(40.dp))
        return
    }
    LazyColumn(Modifier.fillMaxHeight(0.7f)) {
        items(list, key = { it.id }) { a ->
            Row(Modifier.fillMaxWidth().clickable { onGo(a) }.padding(horizontal = 20.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    when (a.kind) {
                        AnnotationKind.BOOKMARK -> Icons.Filled.Bookmark
                        AnnotationKind.HIGHLIGHT -> Icons.Outlined.BorderColor
                        AnnotationKind.NOTE -> Icons.Outlined.ChatBubbleOutline
                    },
                    null, tint = if (a.kind == AnnotationKind.BOOKMARK) MaterialTheme.colorScheme.primary else Color(0xFFF2A600),
                )
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f)) {
                    Text(label(a), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    (a.note ?: a.text)?.let {
                        Text(it, maxLines = 3, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium.copy(textDirection = TextDirection.Content))
                    }
                }
                IconButton(onClick = { onDelete(a) }) { Icon(Icons.Filled.Delete, stringResource(R.string.delete)) }
            }
        }
    }
}

/** Reflowed page text (rebuilt in logical order by the Rust core), Acrobat "Liquid Mode"-style. */
@Composable
private fun PdfReadingMode(s: PdfSession, startPage: Int, night: Boolean) {
    val state = rememberLazyListState(initialFirstVisibleItemIndex = startPage)
    var fontSize by remember { mutableFloatStateOf(Core.setting("pdf_rm_size", "18").toFloatOrNull() ?: 18f) }
    val clipboard = LocalClipboardManager.current
    val fg = if (night) Color(0xFFDDDDDD) else Color(0xFF1B1B1B)
    val bg = if (night) Color(0xFF121212) else Color(0xFFFFFFFF)
    Column(Modifier.fillMaxSize().background(bg)) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("A", fontSize = 13.sp, color = fg)
            Slider(
                value = fontSize, onValueChange = { fontSize = it }, valueRange = 12f..32f,
                onValueChangeFinished = { Core.setSetting("pdf_rm_size", fontSize.toString()) },
                modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
            )
            Text("A", fontSize = 22.sp, color = fg)
        }
        SelectionContainer {
            LazyColumn(state = state, contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 120.dp)) {
                items(s.info.pageCount, key = { it }) { page ->
                    val text by produceState<String?>(null, page) {
                        value = withContext(textDispatcher) { runCatching { s.doc.pageText(page) }.getOrDefault("") }
                    }
                    Column(Modifier.fillMaxWidth().padding(vertical = 12.dp)) {
                        Text(stringResource(R.string.page_n, page + 1), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                        Spacer(Modifier.height(6.dp))
                        val paragraphs = remember(text) { reflowParagraphs(text ?: "") }
                        if (text == null) LinearProgressIndicator(Modifier.fillMaxWidth())
                        paragraphs.forEach { para ->
                            Text(
                                para,
                                style = TextStyle(fontSize = fontSize.sp, lineHeight = (fontSize * 1.65f).sp, color = fg, textDirection = TextDirection.Content, localeList = if (s.info.language == "ug") androidx.compose.ui.text.intl.LocaleList("ug") else null),
                                modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp),
                            )
                        }
                        HorizontalDivider(Modifier.padding(top = 8.dp), color = fg.copy(alpha = 0.12f))
                    }
                }
            }
        }
    }
}

/** Joins hard-wrapped lines into paragraphs; keeps short lines (headings) separate. */
private fun reflowParagraphs(text: String): List<String> {
    val lines = text.split('\n').map { it.trim() }
    val out = mutableListOf<String>()
    val cur = StringBuilder()
    val avg = lines.filter { it.isNotEmpty() }.map { it.length }.average().takeIf { !it.isNaN() } ?: 0.0
    for (l in lines) {
        if (l.isEmpty()) {
            if (cur.isNotEmpty()) { out += cur.toString(); cur.clear() }
            continue
        }
        if (cur.isNotEmpty()) {
            if (cur.endsWith("-")) cur.setLength(cur.length - 1) else cur.append(' ')
        }
        cur.append(l)
        val endsSentence = l.last() in ".!?؟۔:»\"”)"
        if (l.length < avg * 0.6 || endsSentence && l.length < avg * 0.9) {
            out += cur.toString(); cur.clear()
        }
    }
    if (cur.isNotEmpty()) out += cur.toString()
    return out
}
