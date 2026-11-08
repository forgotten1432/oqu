package app.sahifa.reader.ui

import android.annotation.SuppressLint
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.BookmarkBorder
import androidx.compose.material.icons.outlined.BorderColor
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material.icons.outlined.TextFields
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import app.sahifa.reader.R
import app.sahifa.reader.core.Core
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uniffi.reader_core.AnnotationKind
import uniffi.reader_core.BookInfo
import uniffi.reader_core.LibraryEntry
import uniffi.reader_core.ReflowDoc
import uniffi.reader_core.ReflowHit
import uniffi.reader_core.TocItem
import uniffi.reader_core.locateInNodes
import java.io.ByteArrayInputStream
import java.io.File

private const val HOST = "book.local"
private const val HEAD_MARKER = "<!--READER_HEAD-->"

data class ReaderPrefs(
    val fs: Int = 19,
    val lh: Float = 1.7f,
    val margin: Int = 22,
    val theme: String = "light",
    val font: String = "book",
    val justify: Boolean = false,
    val mode: String = "paged",
    val dir: String = "auto",
) {
    fun save() {
        Core.setSetting("rf_fs", fs.toString()); Core.setSetting("rf_lh", lh.toString())
        Core.setSetting("rf_margin", margin.toString()); Core.setSetting("rf_theme", theme)
        Core.setSetting("rf_font", font); Core.setSetting("rf_justify", if (justify) "1" else "0")
        Core.setSetting("rf_mode", mode)
    }

    companion object {
        fun load() = ReaderPrefs(
            fs = Core.setting("rf_fs", "19").toIntOrNull() ?: 19,
            lh = Core.setting("rf_lh", "1.7").toFloatOrNull() ?: 1.7f,
            margin = Core.setting("rf_margin", "22").toIntOrNull() ?: 22,
            theme = Core.setting("rf_theme", "light"),
            font = Core.setting("rf_font", "book"),
            justify = Core.setting("rf_justify", "0") == "1",
            mode = Core.setting("rf_mode", "paged"),
        )
    }
}

private data class ThemeColors(val fg: String, val bg: String, val link: String, val themed: Boolean, val bgColor: Color)

private fun themeColors(t: String) = when (t) {
    "sepia" -> ThemeColors("#5b4636", "#f4ecd8", "#8c3b14", true, Color(0xFFF4ECD8))
    "dark" -> ThemeColors("#dcdcdc", "#1e1f22", "#ff8a80", true, Color(0xFF1E1F22))
    "black" -> ThemeColors("#c8c8c8", "#000000", "#ff8a80", true, Color.Black)
    else -> ThemeColors("#1b1b1b", "#ffffff", "#b3261e", false, Color.White)
}

/** Built-in font choices: key -> CSS font-family. */
private val FONTS = linkedMapOf(
    "book" to "",
    "ukij_tuz" to "'UKIJ Tuz', 'Sahifa Naskh', serif",
    "ukij_ekran" to "'UKIJ Ekran', 'UKIJ Tuz', sans-serif",
    "ukij_basma" to "'UKIJ Tuz Basma', 'UKIJ Tuz', serif",
    "naskh" to "'Sahifa Naskh', serif",
    "vazirmatn" to "'Sahifa Vazirmatn', sans-serif",
    "amiri" to "'Sahifa Amiri', serif",
    "nastaliq" to "'Sahifa Nastaliq', serif",
    "hebrew" to "'Sahifa Hebrew', sans-serif",
    "serif" to "serif",
    "sans" to "sans-serif",
)

private fun fontCss(key: String): String? = when {
    key.startsWith("user:") -> "'SahifaUser ${key.removePrefix("user:").replace("'", "")}', serif"
    else -> FONTS[key]?.takeIf { it.isNotEmpty() }
}

private sealed class Start {
    data class Fraction(val f: Double) : Start()
    data class Anchor(val id: String) : Start()
    data class Find(val q: String, val occ: Int) : Start()
}

private class Session(val doc: ReflowDoc, val info: BookInfo, val entry: LibraryEntry) {
    @Volatile var chapter = 0
    @Volatile var start: Start = Start.Fraction(0.0)
    @Volatile var prefs = ReaderPrefs.load()
    @Volatile var userHighlights: List<String> = emptyList()
    var nextLabel = "→"
    val css: String by lazy { Core.app.assets.open("reader/reader.css").bufferedReader().readText() }
    val js: String by lazy { Core.app.assets.open("reader/reader.js").bufferedReader().readText() }

    fun settingsJson(p: ReaderPrefs = prefs, start: Start? = null): JSONObject {
        val tc = themeColors(if (info.fixedLayout) "black" else p.theme)
        return JSONObject().apply {
            put("fs", p.fs); put("lh", p.lh.toDouble()); put("margin", p.margin)
            put("fg", tc.fg); put("bg", tc.bg); put("link", tc.link); put("themed", tc.themed)
            put("font", fontCss(p.font) ?: JSONObject.NULL)
            put("justify", p.justify)
            put("mode", p.mode)
            put("dir", p.dir)
            put("fixed", info.fixedLayout)
            put("hasNext", chapter < info.chapters.size - 1)
            put("nextLabel", nextLabel)
            put("user", JSONArray(userHighlights))
            if (start != null) put("start", when (start) {
                is Start.Fraction -> JSONObject().put("fraction", start.f)
                is Start.Anchor -> JSONObject().put("anchor", start.id)
                is Start.Find -> JSONObject().put("find", start.q).put("occ", start.occ)
            })
        }
    }

    fun headHtml(): String {
        val userFonts = Core.userFonts().joinToString("\n") { name ->
            "@font-face { font-family: 'SahifaUser ${name.replace("'", "")}'; src: url('/__userfonts/${Uri.encode(name)}'); }"
        }
        // Escape for both HTML and XHTML: no raw '<' or '&' inside the script.
        val json = settingsJson(start = start).toString()
            .replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026")
        return "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no\"/>" +
            "<style>/*<![CDATA[*/\n$css\n$userFonts\n/*]]>*/</style>" +
            "<script>//<![CDATA[\nwindow.__SAHIFA__=$json;\n$js\n//]]></script>"
    }

    fun response(path: String): WebResourceResponse? {
        if (path.startsWith("__fonts/")) {
            val name = path.removePrefix("__fonts/").substringAfterLast('/')
            return runCatching { WebResourceResponse("font/ttf", null, Core.app.assets.open("fonts/$name")) }.getOrNull()
        }
        if (path.startsWith("__userfonts/")) {
            val f = File(Core.fontsDir, path.removePrefix("__userfonts/").substringAfterLast('/'))
            return if (f.exists()) WebResourceResponse("font/ttf", null, f.inputStream()) else null
        }
        doc.chapterIndex(path)?.let { idx ->
            val res = doc.chapter(idx)
            val html = String(res.data, Charsets.UTF_8).replaceFirst(HEAD_MARKER, headHtml())
            return WebResourceResponse(res.mime, "utf-8", ByteArrayInputStream(html.toByteArray(Charsets.UTF_8)))
        }
        val r = doc.resource(path) ?: return null
        return WebResourceResponse(r.mime, if (r.mime.startsWith("text/")) "utf-8" else null, ByteArrayInputStream(r.data))
    }

    fun urlFor(i: Int) = "https://$HOST/" + info.chapters[i].path.split('/').joinToString("/") { Uri.encode(it) }

    fun chapterTitle(i: Int): String =
        info.chapters.getOrNull(i)?.title ?: info.toc.lastOrNull { it.chapter.toInt() <= i }?.title ?: ""
}

@Composable
fun ReflowViewer(entry: LibraryEntry, onClose: () -> Unit) {
    var session by remember { mutableStateOf<Session?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        try {
            session = withContext(Dispatchers.IO) {
                val d = ReflowDoc.open(entry.path)
                val info = d.info()
                Core.library.setMeta(entry.id, info.title, info.author, info.chapters.size)
                Session(d, info, entry)
            }
        } catch (t: Throwable) {
            error = t.message ?: t.toString()
        }
    }
    DisposableEffect(Unit) { onDispose { session?.doc?.close() } }
    when {
        error != null -> ErrorScreen(error!!, onClose)
        session == null -> LoadingScreen()
        else -> BookReader(session!!, onClose)
    }
}

@SuppressLint("SetJavaScriptEnabled")
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun BookReader(s: Session, onClose: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboardManager.current
    val main = remember { Handler(Looper.getMainLooper()) }
    s.nextLabel = stringResource(R.string.next_chapter)

    val n = s.info.chapters.size
    val initial = remember {
        val parts = s.entry.position?.split(':')
        if (parts != null && parts.size >= 3 && parts[0] == "ch") {
            (parts[1].toIntOrNull() ?: 0).coerceIn(0, n - 1) to (parts[2].toDoubleOrNull() ?: 0.0)
        } else 0 to 0.0
    }
    var chapter by remember { mutableIntStateOf(initial.first) }
    var fraction by remember { mutableDoubleStateOf(initial.second) }
    var page by remember { mutableIntStateOf(0) }
    var pages by remember { mutableIntStateOf(1) }
    var chrome by remember { mutableStateOf(true) }
    var prefs by remember { mutableStateOf(s.prefs) }
    var selection by remember { mutableStateOf("") }
    var sheet by remember { mutableStateOf<String?>(null) }
    var menu by remember { mutableStateOf(false) }
    var linkUri by remember { mutableStateOf<String?>(null) }
    var annotations by remember { mutableStateOf(Core.library.annotations(s.entry.id)) }

    var searchOpen by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    var hits by remember { mutableStateOf<List<ReflowHit>>(emptyList()) }
    var hitIndex by remember { mutableIntStateOf(0) }
    var searching by remember { mutableStateOf(false) }

    val webRef = remember { arrayOfNulls<WebView>(1) }

    fun userHighlightsFor(ch: Int) = annotations.filter { it.kind == AnnotationKind.HIGHLIGHT && it.position == "ch:$ch" }.mapNotNull { it.text }

    fun load(i: Int, start: Start) {
        val idx = i.coerceIn(0, n - 1)
        s.chapter = idx
        s.start = start
        s.userHighlights = userHighlightsFor(idx)
        chapter = idx
        webRef[0]?.loadUrl(s.urlFor(idx))
    }

    fun js(code: String) { webRef[0]?.evaluateJavascript(code, null) }

    fun applyPrefs(p: ReaderPrefs) {
        prefs = p; s.prefs = p; p.save()
        webRef[0]?.setBackgroundColor(themeColors(p.theme).bgColor.toArgb())
        js("window.Reader && Reader.apply(${s.settingsJson(p)});")
    }

    fun goHit(i: Int) {
        val h = hits.getOrNull(i) ?: return
        hitIndex = i
        if (h.chapter.toInt() == chapter) js("Reader.find(${JSONObject.quote(query)}, ${h.occurrence})")
        else load(h.chapter.toInt(), Start.Find(query, h.occurrence.toInt()))
    }

    val bridge = remember {
        object {
            @JavascriptInterface fun onProgress(f: Double, p: Int, total: Int) = main.post {
                fraction = f; page = p; pages = total
                val overall = ((s.chapter + f) / n).toFloat()
                Core.library.setProgress(s.entry.id, "ch:${s.chapter}:$f", overall)
            }.let { }
            @JavascriptInterface fun onReady(total: Int) = main.post { pages = total }.let { }
            @JavascriptInterface fun toggleChrome() = main.post { chrome = !chrome }.let { }
            @JavascriptInterface fun nextChapter() = main.post { if (s.chapter < n - 1) load(s.chapter + 1, Start.Fraction(0.0)) }.let { }
            @JavascriptInterface fun prevChapter() = main.post { if (s.chapter > 0) load(s.chapter - 1, Start.Fraction(1.0)) }.let { }
            @JavascriptInterface fun onSelection(t: String) = main.post { selection = t }.let { }
            @JavascriptInterface fun locate(nodesJson: String, q: String): String {
                val arr = JSONArray(nodesJson)
                val nodes = List(arr.length()) { arr.optString(it) }
                val out = JSONArray()
                for (r in locateInNodes(nodes, q, s.info.uyghur)) {
                    out.put(JSONArray().put(r.startNode.toInt()).put(r.startOffset.toInt()).put(r.endNode.toInt()).put(r.endOffset.toInt()))
                }
                return out.toString()
            }
        }
    }

    Box(Modifier.fillMaxSize().background(themeColors(prefs.theme).bgColor)) {
        AndroidView(
            modifier = Modifier.fillMaxSize().systemBarsPadding().padding(top = if (s.info.fixedLayout) 0.dp else 56.dp),
            factory = { c ->
                WebView(c).apply {
                    webRef[0] = this
                    setBackgroundColor(themeColors(prefs.theme).bgColor.toArgb())
                    settings.javaScriptEnabled = true
                    settings.allowFileAccess = false
                    settings.allowContentAccess = false
                    settings.domStorageEnabled = false
                    settings.textZoom = 100
                    settings.useWideViewPort = true
                    settings.loadWithOverviewMode = false
                    settings.setSupportZoom(false)
                    settings.builtInZoomControls = false
                    isVerticalScrollBarEnabled = false
                    isHorizontalScrollBarEnabled = false
                    overScrollMode = WebView.OVER_SCROLL_NEVER
                    if (androidx.webkit.WebViewFeature.isFeatureSupported(androidx.webkit.WebViewFeature.ALGORITHMIC_DARKENING)) {
                        androidx.webkit.WebSettingsCompat.setAlgorithmicDarkeningAllowed(settings, false)
                    }
                    addJavascriptInterface(bridge, "SahifaBridge")
                    webChromeClient = object : android.webkit.WebChromeClient() {
                        override fun onConsoleMessage(m: android.webkit.ConsoleMessage): Boolean {
                            android.util.Log.d("SahifaWeb", "${m.messageLevel()} ${m.message()} @${m.sourceId()}:${m.lineNumber()}")
                            return true
                        }
                    }
                    if (app.sahifa.reader.BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true)
                    webViewClient = object : WebViewClient() {
                        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
                            val u = request.url
                            if (u.host != HOST) return WebResourceResponse("text/plain", null, 404, "Blocked", null, ByteArrayInputStream(ByteArray(0)))
                            val path = (u.path ?: "").removePrefix("/")
                            return try {
                                s.response(path) ?: WebResourceResponse("text/plain", null, 404, "Not found", null, ByteArrayInputStream(ByteArray(0)))
                            } catch (t: Throwable) {
                                WebResourceResponse("text/plain", null, 500, "Error", null, ByteArrayInputStream((t.message ?: "").toByteArray()))
                            }
                        }

                        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                            val u = request.url
                            if (u.host == HOST) {
                                val idx = s.doc.chapterIndex((u.path ?: "").removePrefix("/"))
                                if (idx != null) {
                                    val frag = u.fragment
                                    if (idx.toInt() == s.chapter && frag != null) js("Reader.goAnchor(${JSONObject.quote(frag)})")
                                    else main.post { load(idx.toInt(), if (frag != null) Start.Anchor(frag) else Start.Fraction(0.0)) }
                                }
                                return true
                            }
                            main.post { linkUri = u.toString() }
                            return true
                        }
                    }
                    s.chapter = chapter
                    s.start = Start.Fraction(fraction)
                    s.userHighlights = userHighlightsFor(chapter)
                    loadUrl(s.urlFor(chapter))
                }
            },
            onRelease = { it.destroy(); webRef[0] = null },
        )

        val chapterLabel = s.chapterTitle(chapter).ifBlank { stringResource(R.string.chapter_n, chapter + 1) }
        PagePill(chrome && !s.info.fixedLayout, if (prefs.mode == "paged") "${page + 1} / $pages" else "${(fraction * 100).toInt()}%") { sheet = "toc" }

        ViewerTopBar(
            visible = chrome,
            title = s.info.title,
            onBack = onClose,
            search = if (searchOpen) SearchUi(
                query = query, onQuery = { query = it },
                onSubmit = {
                    val q = query.trim()
                    if (q.isNotEmpty()) {
                        searching = true
                        scope.launch {
                            hits = withContext(Dispatchers.IO) { s.doc.search(q, 1000u) }
                            searching = false
                            val first = hits.indexOfFirst { it.chapter.toInt() >= chapter }.let { if (it < 0) 0 else it }
                            if (hits.isNotEmpty()) goHit(first)
                        }
                    }
                },
                current = hitIndex, total = hits.size, busy = searching,
                onPrev = { if (hits.isNotEmpty()) goHit((hitIndex - 1 + hits.size) % hits.size) },
                hint = if (s.info.uyghur) stringResource(R.string.search_hint_uyghur) else null,
                onNext = { if (hits.isNotEmpty()) goHit((hitIndex + 1) % hits.size) },
                onClose = { searchOpen = false; hits = emptyList(); query = ""; js("Reader.clearFind()") },
            ) else null,
        ) {
            IconButton(onClick = { searchOpen = true }) { Icon(Icons.Filled.Search, stringResource(R.string.search)) }
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, stringResource(R.string.more)) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.share)) }, leadingIcon = { Icon(Icons.Outlined.Share, null) },
                        onClick = { menu = false; shareFile(ctx, s.entry.path, mimeFor(s.entry.format)) },
                    )
                }
            }
        }

        if (chrome && !searchOpen) {
            Text(
                chapterLabel, maxLines = 1, overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.labelMedium.copy(textDirection = TextDirection.Content),
                color = Color(android.graphics.Color.parseColor(themeColors(prefs.theme).fg)).copy(alpha = 0.7f),
                modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 84.dp, start = 32.dp, end = 32.dp),
            )
        }

        BottomTools(chrome && !searchOpen) {
            ToolButton(Icons.AutoMirrored.Filled.List, stringResource(R.string.outline)) { sheet = "toc" }
            ToolButton(Icons.Outlined.TextFields, stringResource(R.string.text_settings)) { sheet = "text" }
            if (selection.isNotBlank()) {
                ToolButton(Icons.Outlined.BorderColor, stringResource(R.string.highlight)) {
                    Core.library.addAnnotation(s.entry.id, AnnotationKind.HIGHLIGHT, "ch:$chapter", selection, null, 0x80FFD54Fu)
                    annotations = Core.library.annotations(s.entry.id)
                    s.userHighlights = userHighlightsFor(chapter)
                    js("window.getSelection().removeAllRanges(); Reader.userHighlights(${JSONArray(s.userHighlights)})")
                    selection = ""
                }
                ToolButton(Icons.Outlined.ContentCopy, stringResource(R.string.copy)) { clipboard.setText(AnnotatedString(selection)) }
            } else {
                val markHere = annotations.firstOrNull { it.kind == AnnotationKind.BOOKMARK && it.position.startsWith("ch:$chapter:") && kotlin.math.abs((it.position.substringAfterLast(':').toDoubleOrNull() ?: -1.0) - fraction) < 0.02 }
                ToolButton(if (markHere != null) Icons.Filled.Bookmark else Icons.Outlined.BookmarkBorder, stringResource(R.string.bookmarks), selected = markHere != null) {
                    if (markHere != null) Core.library.removeAnnotation(markHere.id)
                    else Core.library.addAnnotation(s.entry.id, AnnotationKind.BOOKMARK, "ch:$chapter:$fraction", chapterLabel, null, 0u)
                    annotations = Core.library.annotations(s.entry.id)
                }
            }
            ToolButton(Icons.AutoMirrored.Outlined.Notes, stringResource(R.string.annotations)) { sheet = "ann" }
        }

        if (sheet != null) {
            ModalBottomSheet(onDismissRequest = { sheet = null }) {
                when (sheet) {
                    "toc" -> TocSheet(s, chapter) { item -> sheet = null; load(item.chapter.toInt(), item.anchor?.let { Start.Anchor(it) } ?: Start.Fraction(0.0)) }
                    "text" -> TextSettingsSheet(prefs, s.info.fixedLayout) { applyPrefs(it) }
                    else -> AnnotationSheet(
                        annotations,
                        onGo = { a ->
                            sheet = null
                            val p = a.position.split(':')
                            val ch = p.getOrNull(1)?.toIntOrNull() ?: 0
                            if (a.kind == AnnotationKind.HIGHLIGHT && a.text != null) load(ch, Start.Find(a.text!!, 0))
                            else load(ch, Start.Fraction(p.getOrNull(2)?.toDoubleOrNull() ?: 0.0))
                        },
                        onDelete = {
                            Core.library.removeAnnotation(it.id)
                            annotations = Core.library.annotations(s.entry.id)
                            s.userHighlights = userHighlightsFor(chapter)
                            js("Reader.userHighlights(${JSONArray(s.userHighlights)})")
                        },
                        label = { a ->
                            val ch = a.position.split(':').getOrNull(1)?.toIntOrNull() ?: 0
                            s.chapterTitle(ch).ifBlank { "#${ch + 1}" }
                        },
                    )
                }
            }
        }
        linkUri?.let { LinkDialog(it, ctx) { linkUri = null } }
    }
}

@Composable
private fun TocSheet(s: Session, current: Int, onGo: (TocItem) -> Unit) {
    val items = s.info.toc.ifEmpty {
        s.info.chapters.mapIndexed { i, c -> TocItem(c.title ?: "${i + 1}", i.toUInt(), null, 0u) }
    }
    Text(stringResource(R.string.outline), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
    LazyColumn(Modifier.fillMaxHeight(0.8f)) {
        itemsIndexed(items) { _, t ->
            val active = t.chapter.toInt() == current
            Text(
                t.title,
                style = MaterialTheme.typography.bodyLarge.copy(textDirection = TextDirection.Content),
                fontWeight = if (t.level == 0u) FontWeight.Medium else FontWeight.Normal,
                color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                maxLines = 2, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.fillMaxWidth().clickable { onGo(t) }
                    .padding(start = (20 + 16 * t.level.toInt()).dp, end = 20.dp, top = 12.dp, bottom = 12.dp),
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TextSettingsSheet(p: ReaderPrefs, fixed: Boolean, onChange: (ReaderPrefs) -> Unit) {
    Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 32.dp)) {
        if (!fixed) {
            Label(stringResource(R.string.font_size) + "  ${p.fs}px")
            Slider(value = p.fs.toFloat(), onValueChange = { onChange(p.copy(fs = it.toInt())) }, valueRange = 12f..36f, steps = 23)
            Label(stringResource(R.string.line_spacing) + "  ${"%.1f".format(p.lh)}")
            Slider(value = p.lh, onValueChange = { onChange(p.copy(lh = (it * 10).toInt() / 10f)) }, valueRange = 1.1f..2.6f)
            Label(stringResource(R.string.margins) + "  ${p.margin}px")
            Slider(value = p.margin.toFloat(), onValueChange = { onChange(p.copy(margin = it.toInt())) }, valueRange = 4f..64f)

            Label(stringResource(R.string.theme))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("light" to R.string.theme_light, "sepia" to R.string.theme_sepia, "dark" to R.string.theme_dark, "black" to R.string.theme_black).forEach { (k, r) ->
                    FilterChip(selected = p.theme == k, onClick = { onChange(p.copy(theme = k)) }, label = { Text(stringResource(r)) })
                }
            }
            Label(stringResource(R.string.font))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val names = mapOf("ukij_tuz" to "UKIJ Tuz", "ukij_ekran" to "UKIJ Ekran", "ukij_basma" to "UKIJ Basma", "book" to stringResource(R.string.font_book), "naskh" to "نسخ Naskh", "vazirmatn" to "وزیرمتن Vazirmatn", "amiri" to "أميري Amiri", "nastaliq" to "نستعلیق Nastaliq", "hebrew" to "עברית Hebrew", "serif" to "Serif", "sans" to "Sans")
                FONTS.keys.forEach { k -> FilterChip(selected = p.font == k, onClick = { onChange(p.copy(font = k)) }, label = { Text(names[k] ?: k) }) }
                Core.userFonts().forEach { f ->
                    val k = "user:$f"
                    FilterChip(selected = p.font == k, onClick = { onChange(p.copy(font = k)) }, label = { Text(f.substringBeforeLast('.')) })
                }
            }
            Label(stringResource(R.string.direction))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("auto" to stringResource(R.string.dir_auto), "rtl" to "RTL ←", "ltr" to "LTR →").forEach { (k, l) ->
                    FilterChip(selected = p.dir == k, onClick = { onChange(p.copy(dir = k)) }, label = { Text(l) })
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.justify), Modifier.weight(1f))
                Switch(checked = p.justify, onCheckedChange = { onChange(p.copy(justify = it)) })
            }
        }
        Label(stringResource(R.string.layout))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = p.mode == "paged", onClick = { onChange(p.copy(mode = "paged")) }, label = { Text(stringResource(R.string.paged)) })
            FilterChip(selected = p.mode == "scroll", onClick = { onChange(p.copy(mode = "scroll")) }, label = { Text(stringResource(R.string.scrolled)) })
        }
    }
}

@Composable
private fun Label(t: String) {
    Text(t, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 14.dp, bottom = 4.dp))
}
