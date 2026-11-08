package app.sahifa.reader

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.lifecycleScope
import app.sahifa.reader.core.Core
import app.sahifa.reader.ui.HomeScreen
import app.sahifa.reader.ui.SahifaTheme
import app.sahifa.reader.ui.ViewerScreen
import kotlinx.coroutines.launch
import uniffi.reader_core.LibraryEntry

class MainActivity : ComponentActivity() {
    /** Applies the in-app language. Layout direction follows it automatically:
     *  Uyghur/Arabic -> right-to-left, Turkish/English -> left-to-right. */
    override fun attachBaseContext(base: android.content.Context) {
        super.attachBaseContext(base)
        val tag = runCatching { Core.setting("app_lang", "") }.getOrDefault("")
        if (tag.isBlank()) return
        val locale = java.util.Locale.forLanguageTag(tag)
        java.util.Locale.setDefault(locale)
        // Override only the locale; the Activity keeps its own context, which
        // system services such as printing require.
        applyOverrideConfiguration(android.content.res.Configuration().apply {
            setLocale(locale)
            setLayoutDirection(locale)
        })
    }

    private var current by mutableStateOf<LibraryEntry?>(null)
    private var importing by mutableStateOf(false)
    private var appTheme by mutableStateOf("system")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        appTheme = Core.setting("app_theme", "system")
        savedInstanceState?.getString("open_id")?.let { id -> current = Core.library.get(id) }
        handleIntent(intent)
        setContent {
            SahifaTheme(appTheme) {
                val doc = current
                if (doc == null) {
                    HomeScreen(
                        importing = importing,
                        onOpenUri = { openUri(it) },
                        onOpenEntry = { current = Core.library.touch(it.path, it.displayName) },
                        appTheme = appTheme,
                        onAppTheme = { appTheme = it; Core.setSetting("app_theme", it) },
                        onLanguage = { tag -> Core.setSetting("app_lang", tag); recreate() },
                    )
                } else {
                    BackHandler { current = null }
                    ViewerScreen(entry = doc, onClose = { current = null })
                }
            }
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        current?.let { outState.putString("open_id", it.id) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        val uri: Uri? = when (intent.action) {
            Intent.ACTION_VIEW -> intent.data
            Intent.ACTION_SEND -> if (Build.VERSION.SDK_INT >= 33) {
                intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM)
            }
            else -> null
        }
        uri?.let { openUri(it) }
    }

    private fun openUri(uri: Uri) {
        importing = true
        lifecycleScope.launch {
            try {
                current = Core.import(uri)
            } catch (t: Throwable) {
                Toast.makeText(this@MainActivity, getString(R.string.error_open) + ": " + t.message, Toast.LENGTH_LONG).show()
            } finally {
                importing = false
            }
        }
    }
}
