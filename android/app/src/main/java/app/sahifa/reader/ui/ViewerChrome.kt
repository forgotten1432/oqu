package app.sahifa.reader.ui

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import app.sahifa.reader.R
import java.io.File

/** Acrobat-style top bar: back, title, actions; or an inline search field. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ViewerTopBar(
    visible: Boolean,
    title: String,
    onBack: () -> Unit,
    search: SearchUi?,
    actions: @Composable RowScope.() -> Unit,
) {
    AnimatedVisibility(visible || search != null, enter = slideInVertically { -it } + fadeIn(), exit = slideOutVertically { -it } + fadeOut()) {
        Surface(color = MaterialTheme.colorScheme.surface, shadowElevation = 3.dp) {
            Column(Modifier.statusBarsPadding()) {
                if (search == null) {
                    TopAppBar(
                        title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 17.sp) },
                        navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                        actions = actions,
                        windowInsets = WindowInsets(0),
                    )
                } else {
                    SearchRow(search)
                }
            }
        }
    }
}

class SearchUi(
    val query: String,
    val onQuery: (String) -> Unit,
    val onSubmit: () -> Unit,
    val current: Int,
    val total: Int,
    val busy: Boolean,
    val onPrev: () -> Unit,
    val onNext: () -> Unit,
    val onClose: () -> Unit,
    val hint: String? = null,
)

@Composable
private fun SearchRow(s: SearchUi) {
    val fr = remember { FocusRequester() }
    LaunchedEffect(Unit) { fr.requestFocus() }
    Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = s.onClose) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) }
        TextField(
            value = s.query, onValueChange = s.onQuery, singleLine = true,
            placeholder = { Text(s.hint ?: stringResource(R.string.search_in_doc), maxLines = 1) },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { s.onSubmit() }),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent,
                focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent,
            ),
            modifier = Modifier.weight(1f).focusRequester(fr),
        )
        if (s.busy) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
        if (s.total > 0) {
            Text(stringResource(R.string.results_count, s.current + 1, s.total), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 6.dp))
        } else if (!s.busy && s.query.isNotBlank()) {
            Text(stringResource(R.string.no_results), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        IconButton(onClick = s.onPrev, enabled = s.total > 0) { Icon(Icons.Filled.KeyboardArrowUp, null) }
        IconButton(onClick = s.onNext, enabled = s.total > 0) { Icon(Icons.Filled.KeyboardArrowDown, null) }
    }
}

/** Floating rounded tool bar at the bottom, like Acrobat's quick tools. */
@Composable
fun BoxScope.BottomTools(visible: Boolean, content: @Composable RowScope.() -> Unit) {
    AnimatedVisibility(
        visible, modifier = Modifier.align(Alignment.BottomCenter),
        enter = slideInVertically { it } + fadeIn(), exit = slideOutVertically { it } + fadeOut(),
    ) {
        Row(
            Modifier.navigationBarsPadding().padding(bottom = 14.dp, start = 12.dp, end = 12.dp)
                .shadow(8.dp, RoundedCornerShape(28.dp))
                .clip(RoundedCornerShape(28.dp))
                .background(MaterialTheme.colorScheme.surface)
                .padding(horizontal = 6.dp, vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            content = content,
        )
    }
}

@Composable
fun ToolButton(icon: ImageVector, label: String, selected: Boolean = false, onClick: () -> Unit) {
    val tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
    Column(
        Modifier.clip(RoundedCornerShape(16.dp)).clickable(onClick = onClick)
            .background(if (selected) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.6f) else Color.Transparent)
            .padding(horizontal = 10.dp, vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, label, tint = tint, modifier = Modifier.size(22.dp))
        Text(label, fontSize = 10.sp, color = tint, maxLines = 1)
    }
}

/** "12 / 240" pill shown while reading; tap to jump. */
@Composable
fun BoxScope.PagePill(visible: Boolean, text: String, onClick: () -> Unit) {
    AnimatedVisibility(visible, modifier = Modifier.align(Alignment.TopEnd), enter = fadeIn(), exit = fadeOut()) {
        Text(
            text,
            color = Color.White, fontSize = 13.sp,
            modifier = Modifier.statusBarsPadding().padding(top = 72.dp, end = 12.dp)
                .clip(RoundedCornerShape(14.dp)).background(Color(0xCC303030))
                .clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 5.dp),
        )
    }
}

@Composable
fun GoToPageDialog(count: Int, onGo: (Int) -> Unit, onDismiss: () -> Unit) {
    var v by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.go_to_page)) },
        text = {
            OutlinedTextField(
                value = v, onValueChange = { v = it.filter(Char::isDigit).take(6) },
                label = { Text("${stringResource(R.string.page_number)} (1–$count)") },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true,
            )
        },
        confirmButton = { TextButton(onClick = { v.toIntOrNull()?.let { onGo((it - 1).coerceIn(0, count - 1)) }; onDismiss() }) { Text(stringResource(R.string.ok)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun PasswordDialog(onSubmit: (String) -> Unit, onDismiss: () -> Unit) {
    var v by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.password_title)) },
        text = {
            OutlinedTextField(
                value = v, onValueChange = { v = it }, label = { Text(stringResource(R.string.password_hint)) },
                visualTransformation = PasswordVisualTransformation(), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            )
        },
        confirmButton = { TextButton(onClick = { onSubmit(v) }) { Text(stringResource(R.string.open)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun NoteDialog(initial: String = "", onSave: (String) -> Unit, onDismiss: () -> Unit, onDelete: (() -> Unit)? = null) {
    var v by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.note)) },
        text = { OutlinedTextField(value = v, onValueChange = { v = it }, minLines = 3, modifier = Modifier.fillMaxWidth()) },
        confirmButton = { TextButton(onClick = { if (v.isNotBlank()) onSave(v.trim()); onDismiss() }) { Text(stringResource(R.string.save)) } },
        dismissButton = {
            Row {
                if (onDelete != null) TextButton(onClick = { onDelete(); onDismiss() }) { Text(stringResource(R.string.delete)) }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
            }
        },
    )
}

@Composable
fun LinkDialog(uri: String, ctx: Context, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.open_link)) },
        text = { Text(uri) },
        confirmButton = {
            TextButton(onClick = {
                try { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(uri))) } catch (_: ActivityNotFoundException) {}
                onDismiss()
            }) { Text(stringResource(R.string.open)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun HintBanner(text: String, onDone: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.primaryContainer).padding(horizontal = 16.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, Modifier.weight(1f), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onPrimaryContainer)
        TextButton(onClick = onDone) { Text(stringResource(R.string.done)) }
    }
}

fun shareFile(ctx: Context, path: String, mime: String) {
    val f = File(path)
    val uri = FileProvider.getUriForFile(ctx, ctx.packageName + ".files", f)
    val i = Intent(Intent.ACTION_SEND).setType(mime).putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    ctx.startActivity(Intent.createChooser(i, f.name))
}

fun mimeFor(format: String) = when (format) {
    "PDF" -> "application/pdf"
    "EPUB" -> "application/epub+zip"
    "DOCX" -> "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    "ODT" -> "application/vnd.oasis.opendocument.text"
    "HTML" -> "text/html"
    "TXT", "MD" -> "text/plain"
    else -> "application/octet-stream"
}
