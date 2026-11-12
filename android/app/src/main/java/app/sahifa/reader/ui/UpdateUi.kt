package app.sahifa.reader.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.unit.dp
import app.sahifa.reader.R
import app.sahifa.reader.core.Core
import app.sahifa.reader.core.Updater
import kotlinx.coroutines.launch
import java.io.File

/** Release notes are Markdown with a download table; keep only readable lines. */
private fun plainNotes(md: String): String {
    val body = md.substringAfter("\n---\n", md)
    return body.lines()
        .filterNot { it.trim().startsWith("|") }
        .joinToString("\n") { it.replace("**", "").replace("`", "").replace(Regex("^#+\\s*"), "").replace(Regex("^- "), "• ") }
        .replace(Regex("\n{3,}"), "\n\n")
        .trim()
        .take(1500)
}

/**
 * "New version" dialog: shows what's new, downloads the APK for this device,
 * verifies it and opens Android's installer.
 */
@Composable
fun UpdateDialog(release: Updater.Release, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var progress by remember { mutableStateOf<Float?>(null) }
    var file by remember { mutableStateOf<File?>(null) }
    var needPermission by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val failed = stringResource(R.string.update_failed)

    fun installNow(f: File) {
        needPermission = !Updater.install(ctx, f)
    }

    AlertDialog(
        onDismissRequest = { if (progress == null) onDismiss() },
        title = { Text(stringResource(R.string.update_available_title, release.version)) },
        text = {
            Column {
                Text(
                    stringResource(R.string.version_n, Updater.currentVersion) + "  →  " + release.version,
                    style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary,
                )
                Spacer(Modifier.height(10.dp))
                Box(Modifier.heightIn(max = 260.dp).verticalScroll(rememberScrollState())) {
                    Text(plainNotes(release.notes), style = MaterialTheme.typography.bodyMedium.copy(textDirection = TextDirection.Content))
                }
                progress?.let { p ->
                    Spacer(Modifier.height(14.dp))
                    Text(stringResource(R.string.downloading), style = MaterialTheme.typography.labelMedium)
                    Spacer(Modifier.height(6.dp))
                    LinearProgressIndicator(progress = { p }, modifier = Modifier.fillMaxWidth())
                }
                if (needPermission) {
                    Spacer(Modifier.height(12.dp))
                    Text(stringResource(R.string.allow_install_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary)
                }
                error?.let {
                    Spacer(Modifier.height(12.dp))
                    Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = progress == null || file != null,
                onClick = {
                    error = null
                    file?.let { installNow(it); return@TextButton }
                    progress = 0f
                    scope.launch {
                        try {
                            val f = Updater.download(release) { progress = it }
                            file = f
                            progress = 1f
                            installNow(f)
                        } catch (t: Throwable) {
                            progress = null
                            error = failed + (t.message?.let { ": $it" } ?: "")
                        }
                    }
                },
            ) { Text(stringResource(if (file != null) R.string.install else R.string.update_now)) }
        },
        dismissButton = {
            if (progress == null || file != null) {
                TextButton(onClick = {
                    // "Later" hides this version until the next one (or a manual check)
                    Core.setSetting("update_skip", release.version)
                    onDismiss()
                }) { Text(stringResource(R.string.later)) }
            }
        },
    )
}
