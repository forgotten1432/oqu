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
import app.sahifa.reader.core.Updater
import app.sahifa.reader.ui.UpdateDialog
import androidx.activity.result.contract.ActivityResultContracts
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
    private var update by mutableStateOf<Updater.Release?>(null)

    private val notifPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    /**
     * Looks for a newer release. Automatic checks run at most every 12 hours and
     * respect "Later"; a manual check (or tapping the notification) always shows it.
     */
    private fun checkForUpdate(manual: Boolean) {
        if (!manual) {
            if (!Updater.autoCheck) return
            val last = Core.setting("update_last_check", "0").toLongOrNull() ?: 0L
            if (System.currentTimeMillis() - last < 12 * 3600_000L) return
        }
        lifecycleScope.launch {
            val r = runCatching { Updater.check() }.getOrNull()
            when {
                r == null && manual -> Toast.makeText(this@MainActivity, getString(R.string.up_to_date, Updater.currentVersion), Toast.LENGTH_SHORT).show()
                r != null && (manual || Core.setting("update_skip", "") != r.version) -> update = r
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        appTheme = Core.setting("app_theme", "system")
        savedInstanceState?.getString("open_id")?.let { id -> current = Core.library.get(id) }
        handleIntent(intent)
        // Ask once (Android 13+) so update notifications can be shown.
        if (Build.VERSION.SDK_INT >= 33 && Core.setting("notif_asked", "0") != "1") {
            Core.setSetting("notif_asked", "1")
            notifPermission.launch(android.Manifest.permission.POST_NOTIFICATIONS)
        }
        checkForUpdate(manual = intent?.getBooleanExtra(Updater.EXTRA_SHOW_UPDATE, false) == true)
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
                        onCheckUpdate = { checkForUpdate(manual = true) },
                    )
                } else {
                    BackHandler { current = null }
                    ViewerScreen(entry = doc, onClose = { current = null })
                }
                update?.let { UpdateDialog(it) { update = null } }
            }
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        current?.let { outState.putString("open_id", it.id) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (intent.getBooleanExtra(Updater.EXTRA_SHOW_UPDATE, false)) checkForUpdate(manual = true)
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
