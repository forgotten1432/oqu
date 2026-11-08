package app.sahifa.reader.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

val Brand = Color(0xFFC4282F)
val BrandDark = Color(0xFFFF6B6F)

/** File-type accent colors used by list icons (PDF red, EPUB green, Word blue…). */
fun formatColor(format: String): Color = when (format) {
    "PDF" -> Color(0xFFD32F2F)
    "EPUB" -> Color(0xFF2E7D32)
    "DOCX" -> Color(0xFF1565C0)
    "ODT" -> Color(0xFF00838F)
    "FB2" -> Color(0xFF6A1B9A)
    "HTML" -> Color(0xFFEF6C00)
    "MD" -> Color(0xFF37474F)
    "TXT" -> Color(0xFF546E7A)
    "CBZ" -> Color(0xFFAD1457)
    else -> Color(0xFF757575)
}

@Composable
fun SahifaTheme(mode: String, content: @Composable () -> Unit) {
    val dark = when (mode) {
        "dark" -> true
        "light" -> false
        else -> isSystemInDarkTheme()
    }
    val colors = if (dark) darkColorScheme(
        primary = BrandDark,
        onPrimary = Color(0xFF3B0004),
        primaryContainer = Color(0xFF7A1A1F),
        onPrimaryContainer = Color(0xFFFFDAD8),
        secondary = Color(0xFFE7BDBA),
        background = Color(0xFF121214),
        surface = Color(0xFF1A1A1D),
        surfaceVariant = Color(0xFF2A2A2E),
        surfaceContainer = Color(0xFF202024),
        surfaceContainerHigh = Color(0xFF26262A),
    ) else lightColorScheme(
        primary = Brand,
        onPrimary = Color.White,
        primaryContainer = Color(0xFFFFDAD8),
        onPrimaryContainer = Color(0xFF410005),
        secondary = Color(0xFF775654),
        background = Color(0xFFF7F7F8),
        surface = Color.White,
        surfaceVariant = Color(0xFFF0EEEE),
        surfaceContainer = Color(0xFFF3F2F2),
        surfaceContainerHigh = Color(0xFFECEAEA),
    )
    MaterialTheme(colorScheme = colors, content = content)
}
