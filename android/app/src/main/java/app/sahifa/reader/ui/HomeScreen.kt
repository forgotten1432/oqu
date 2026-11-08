package app.sahifa.reader.ui

import android.net.Uri
import android.text.format.Formatter
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.Image
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.FontDownload
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.StarBorder
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.LifecycleResumeEffect
import app.sahifa.reader.R
import app.sahifa.reader.core.Core
import uniffi.reader_core.LibraryEntry

private val OPEN_MIME = arrayOf(
    "application/pdf", "application/epub+zip",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text", "application/x-fictionbook+xml",
    "text/html", "application/xhtml+xml", "text/plain", "text/markdown",
    "application/zip", "application/x-cbz", "application/octet-stream", "*/*",
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    importing: Boolean,
    onOpenUri: (Uri) -> Unit,
    onOpenEntry: (LibraryEntry) -> Unit,
    appTheme: String,
    onAppTheme: (String) -> Unit,
    onLanguage: (String) -> Unit = {},
) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val wide = isWideScreen()
    var searching by rememberSaveable { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    var refresh by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) { refresh++; onPauseOrDispose { } }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri -> uri?.let(onOpenUri) }

    Scaffold(
        topBar = {
            if (searching) {
                TopAppBar(
                    title = {
                        TextField(
                            value = query, onValueChange = { query = it },
                            placeholder = { Text(stringResource(R.string.search_files)) },
                            singleLine = true,
                            colors = TextFieldDefaults.colors(
                                focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent,
                                focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent,
                            ),
                            modifier = Modifier.fillMaxWidth(),
                        )
                    },
                    navigationIcon = {
                        IconButton(onClick = { searching = false; query = "" }) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) }
                    },
                )
            } else {
                TopAppBar(
                    title = {
                        Text(
                            when (tab) { 0 -> stringResource(R.string.home); 1 -> stringResource(R.string.files); else -> stringResource(R.string.settings) },
                            fontWeight = FontWeight.SemiBold,
                        )
                    },
                    actions = {
                        if (tab == 0) IconButton(onClick = { searching = true }) { Icon(Icons.Filled.Search, stringResource(R.string.search)) }
                    },
                )
            }
        },
        bottomBar = {
            if (!wide) NavigationBar {
                NavigationBarItem(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, null) }, label = { Text(stringResource(R.string.home)) })
                NavigationBarItem(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.Outlined.Folder, null) }, label = { Text(stringResource(R.string.files)) })
                NavigationBarItem(selected = tab == 2, onClick = { tab = 2 }, icon = { Icon(Icons.Outlined.Settings, null) }, label = { Text(stringResource(R.string.settings)) })
            }
        },
        floatingActionButton = {
            if (tab != 2 && !wide) {
                ExtendedFloatingActionButton(
                    onClick = { picker.launch(OPEN_MIME) },
                    icon = { Icon(Icons.Filled.Add, null) },
                    text = { Text(stringResource(R.string.open_file)) },
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                )
            }
        },
    ) { pad ->
        Row(Modifier.padding(pad).fillMaxSize()) {
            if (wide) {
                // Tablets: side navigation rail (mirrors to the right in Uyghur/Arabic)
                NavigationRail(
                    header = {
                        FloatingActionButton(
                            onClick = { picker.launch(OPEN_MIME) },
                            containerColor = MaterialTheme.colorScheme.primary,
                            contentColor = MaterialTheme.colorScheme.onPrimary,
                        ) { Icon(Icons.Filled.Add, stringResource(R.string.open_file)) }
                    },
                ) {
                    Spacer(Modifier.height(12.dp))
                    NavigationRailItem(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, null) }, label = { Text(stringResource(R.string.home)) })
                    NavigationRailItem(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.Outlined.Folder, null) }, label = { Text(stringResource(R.string.files)) })
                    NavigationRailItem(selected = tab == 2, onClick = { tab = 2 }, icon = { Icon(Icons.Outlined.Settings, null) }, label = { Text(stringResource(R.string.settings)) })
                }
            }
            Box(Modifier.weight(1f).fillMaxHeight()) {
                when (tab) {
                    0 -> RecentTab(query, refresh, wide, onOpenEntry) { refresh++ }
                    1 -> Centered(wide) { FilesTab(onPick = { picker.launch(OPEN_MIME) }) }
                    else -> Centered(wide) { SettingsTab(appTheme, onAppTheme, onLanguage) }
                }
                if (importing) {
                    LinearProgressIndicator(Modifier.fillMaxWidth().align(Alignment.TopCenter))
                }
            }
        }
    }
}

@Composable
private fun RecentTab(query: String, refresh: Int, wide: Boolean, onOpen: (LibraryEntry) -> Unit, onChanged: () -> Unit) {
    var sub by rememberSaveable { mutableIntStateOf(0) }
    val all = remember(query, refresh) { Core.library.recent(query.ifBlank { null }) }
    val list = if (sub == 1) all.filter { it.starred } else all
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = sub == 0, onClick = { sub = 0 }, label = { Text(stringResource(R.string.recent)) })
            FilterChip(selected = sub == 1, onClick = { sub = 1 }, label = { Text(stringResource(R.string.starred)) },
                leadingIcon = { Icon(Icons.Filled.Star, null, Modifier.size(16.dp)) })
        }
        if (list.isEmpty()) {
            EmptyState()
        } else if (wide) {
            // Tablets: a bookshelf of covers
            LazyVerticalGrid(
                columns = GridCells.Adaptive(168.dp),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 32.dp),
                horizontalArrangement = Arrangement.spacedBy(20.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                items(list, key = { it.id }) { e -> DocCard(e, onOpen, onChanged) }
            }
        } else {
            LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) {
                items(list, key = { it.id }) { e -> DocRow(e, onOpen, onChanged) }
            }
        }
    }
}

/** Keeps settings-style pages at a readable width on large screens. */
@Composable
private fun Centered(wide: Boolean, content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Box(if (wide) Modifier.widthIn(max = 720.dp).fillMaxHeight() else Modifier.fillMaxSize()) { content() }
    }
}

@Composable
private fun DocCard(e: LibraryEntry, onOpen: (LibraryEntry) -> Unit, onChanged: () -> Unit) {
    val ctx = LocalContext.current
    val cover = rememberCover(e)
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickable { onOpen(e) }) {
        Box(
            Modifier.fillMaxWidth().aspectRatio(0.72f).shadow(3.dp, RoundedCornerShape(8.dp))
                .clip(RoundedCornerShape(8.dp)).background(MaterialTheme.colorScheme.surface),
            contentAlignment = Alignment.Center,
        ) {
            if (cover != null) {
                Image(cover.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop, alignment = Alignment.TopCenter)
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    FormatBadge(e.format, Modifier.scale(1.6f))
                    Spacer(Modifier.height(20.dp))
                    Text(
                        e.title ?: e.displayName, maxLines = 3, overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.bodySmall.copy(textDirection = TextDirection.Content),
                        modifier = Modifier.padding(horizontal = 12.dp),
                    )
                }
            }
            // format chip + star in the corners
            Text(
                e.format, color = Color.White, fontSize = 10.sp, fontWeight = FontWeight.Bold,
                modifier = Modifier.align(Alignment.TopStart).padding(8.dp).clip(RoundedCornerShape(4.dp))
                    .background(formatColor(e.format)).padding(horizontal = 6.dp, vertical = 2.dp),
            )
            if (e.starred) Icon(Icons.Filled.Star, null, tint = Color(0xFFF2A600), modifier = Modifier.align(Alignment.TopEnd).padding(6.dp).size(20.dp))
            if (e.progress > 0f) {
                LinearProgressIndicator(
                    progress = { e.progress }, modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(4.dp),
                    trackColor = Color(0x33000000), drawStopIndicator = {},
                )
            }
        }
        Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.Top) {
            Column(Modifier.weight(1f)) {
                Text(
                    e.title ?: e.displayName, maxLines = 2, overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium.copy(textDirection = TextDirection.Content), fontWeight = FontWeight.Medium,
                )
                Text(
                    "${Formatter.formatShortFileSize(ctx, e.sizeBytes.toLong())} • ${relativeTime(e.lastOpenedMs)}",
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1,
                )
            }
            Box {
                IconButton(onClick = { menu = true }, modifier = Modifier.size(32.dp)) { Icon(Icons.Filled.MoreVert, stringResource(R.string.more)) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(if (e.starred) R.string.unstar else R.string.star)) },
                        leadingIcon = { Icon(if (e.starred) Icons.Filled.Star else Icons.Outlined.StarBorder, null) },
                        onClick = { Core.library.setStarred(e.id, !e.starred); menu = false; onChanged() },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.remove_from_recent)) },
                        leadingIcon = { Icon(Icons.Filled.Close, null) },
                        onClick = { Core.library.remove(e.id); menu = false; onChanged() },
                    )
                }
            }
        }
    }
}

@Composable
private fun EmptyState() {
    Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.size(88.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primaryContainer), contentAlignment = Alignment.Center) {
            Icon(Icons.Outlined.Folder, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.primary)
        }
        Spacer(Modifier.height(16.dp))
        Text(stringResource(R.string.empty_title), style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(6.dp))
        Text(stringResource(R.string.empty_body), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun FormatBadge(format: String, modifier: Modifier = Modifier) {
    Box(
        modifier.size(width = 40.dp, height = 48.dp).clip(RoundedCornerShape(6.dp)).background(formatColor(format)),
        contentAlignment = Alignment.BottomCenter,
    ) {
        Text(format, color = Color.White, fontSize = 10.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 6.dp))
    }
}

@Composable
private fun DocRow(e: LibraryEntry, onOpen: (LibraryEntry) -> Unit, onChanged: () -> Unit) {
    val ctx = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().clickable { onOpen(e) }.padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FormatBadge(e.format)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(e.title ?: e.displayName, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium)
            val meta = buildString {
                append(e.format)
                append(" • ")
                append(Formatter.formatShortFileSize(ctx, e.sizeBytes.toLong()))
                append(" • ")
                append(relativeTime(e.lastOpenedMs))
            }
            Text(meta, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
            if (e.progress > 0f) {
                Spacer(Modifier.height(6.dp))
                LinearProgressIndicator(
                    progress = { e.progress }, modifier = Modifier.fillMaxWidth(0.6f).height(3.dp).clip(RoundedCornerShape(2.dp)),
                    trackColor = MaterialTheme.colorScheme.surfaceVariant, drawStopIndicator = {},
                )
            }
        }
        if (e.starred) Icon(Icons.Filled.Star, null, tint = Color(0xFFF2A600), modifier = Modifier.size(18.dp))
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, stringResource(R.string.more)) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(if (e.starred) R.string.unstar else R.string.star)) },
                    leadingIcon = { Icon(if (e.starred) Icons.Filled.Star else Icons.Outlined.StarBorder, null) },
                    onClick = { Core.library.setStarred(e.id, !e.starred); menu = false; onChanged() },
                )
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.remove_from_recent)) },
                    leadingIcon = { Icon(Icons.Filled.Close, null) },
                    onClick = { Core.library.remove(e.id); menu = false; onChanged() },
                )
            }
        }
    }
}

@Composable
private fun relativeTime(ms: Long): String {
    val d = (System.currentTimeMillis() - ms).coerceAtLeast(0) / 60_000
    return when {
        d < 1 -> stringResource(R.string.just_now)
        d < 60 -> stringResource(R.string.minutes_ago, d.toInt())
        d < 60 * 24 -> stringResource(R.string.hours_ago, (d / 60).toInt())
        else -> stringResource(R.string.days_ago, (d / 1440).toInt())
    }
}

@Composable
private fun FilesTab(onPick: () -> Unit) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp)) {
        ElevatedCard(onClick = onPick, modifier = Modifier.fillMaxWidth()) {
            Row(Modifier.padding(20.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(48.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primaryContainer), contentAlignment = Alignment.Center) {
                    Icon(Icons.Outlined.Folder, null, tint = MaterialTheme.colorScheme.primary)
                }
                Spacer(Modifier.width(16.dp))
                Column {
                    Text(stringResource(R.string.open_file), style = MaterialTheme.typography.titleMedium)
                    Text(stringResource(R.string.open_file_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        Spacer(Modifier.height(24.dp))
        Text(stringResource(R.string.supported_formats), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(12.dp))
        val formats = listOf("PDF", "EPUB", "DOCX", "ODT", "FB2", "HTML", "MD", "TXT", "CBZ")
        formats.chunked(5).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.padding(bottom = 12.dp)) {
                row.forEach { FormatBadge(it) }
            }
        }
    }
}

@Composable
private fun SettingsTab(appTheme: String, onAppTheme: (String) -> Unit, onLanguage: (String) -> Unit) {
    var fonts by remember { mutableStateOf(Core.userFonts()) }
    val fontPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let { Core.importFont(it); fonts = Core.userFonts() }
    }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp)) {
        Text(stringResource(R.string.language), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(8.dp))
        val lang = remember { Core.setting("app_lang", "") }
        @OptIn(ExperimentalLayoutApi::class)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // Endonyms, so everyone can find their language whatever the UI is set to.
            listOf("" to stringResource(R.string.lang_system), "ug" to "ئۇيغۇرچە", "tr" to "Türkçe", "en" to "English", "ar" to "العربية").forEach { (tag, label) ->
                FilterChip(selected = lang == tag, onClick = { if (lang != tag) onLanguage(tag) }, label = { Text(label) })
            }
        }
        Spacer(Modifier.height(24.dp))
        Text(stringResource(R.string.app_theme), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("system" to R.string.theme_system, "light" to R.string.theme_light, "dark" to R.string.theme_dark).forEach { (k, r) ->
                FilterChip(selected = appTheme == k, onClick = { onAppTheme(k) }, label = { Text(stringResource(r)) })
            }
        }
        Spacer(Modifier.height(24.dp))
        Text(stringResource(R.string.imported_fonts), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(8.dp))
        fonts.forEach { Text("• $it", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(vertical = 2.dp)) }
        OutlinedButton(onClick = { fontPicker.launch(arrayOf("font/ttf", "font/otf", "application/x-font-ttf", "application/font-sfnt", "application/octet-stream", "*/*")) }) {
            Icon(Icons.Outlined.FontDownload, null)
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.import_font))
        }
        Spacer(Modifier.height(24.dp))
        Text(stringResource(R.string.about), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.about_body), style = MaterialTheme.typography.bodyMedium)
        Spacer(Modifier.height(8.dp))
        Text(uniffi.reader_core.coreVersion(), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Core.pdfError?.let { Text("PDFium: $it", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
    }
}
