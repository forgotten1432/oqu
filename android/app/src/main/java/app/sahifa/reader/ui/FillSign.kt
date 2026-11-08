package app.sahifa.reader.ui

import android.graphics.Bitmap
import android.graphics.Canvas as AndroidCanvas
import android.graphics.Paint
import android.graphics.Path as AndroidPath
import android.graphics.Typeface
import android.text.Layout
import android.text.StaticLayout
import android.text.TextDirectionHeuristics
import android.text.TextPaint
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.sahifa.reader.R
import app.sahifa.reader.core.Core
import uniffi.reader_core.NormRect
import java.io.File
import java.nio.ByteBuffer
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** Something placed on a page in Fill & Sign mode, not yet written into the PDF. */
class Stamp(
    val id: Long,
    val page: Int,
    rect: NormRect,
    val bitmap: Bitmap?, // null = white-out box
) {
    var rect by mutableStateOf(rect)
}

/** What a tap or drag on a page does while Fill & Sign is open. */
enum class SignTool { None, Place, WhiteOut }

class EditState {
    val stamps = mutableStateListOf<Stamp>()
    var tool by mutableStateOf(SignTool.None)
    /** Bitmap waiting to be placed with the next tap, and its width in PDF points. */
    var pending by mutableStateOf<Bitmap?>(null)
    var pendingWidthPt by mutableFloatStateOf(120f)
    var selected by mutableStateOf<Long?>(null)
    private var nextId = 1L
    fun newId() = nextId++
    val dirty get() = stamps.isNotEmpty()

    fun place(page: Int, at: Offset, pageWidthPt: Float, pageAspect: Float) {
        val bmp = pending ?: return
        val w = (pendingWidthPt / pageWidthPt).coerceIn(0.02f, 0.95f)
        val h = w * bmp.height / bmp.width / pageAspect
        val left = (at.x - w / 2).coerceIn(0f, 1f - w)
        val top = (at.y - h / 2).coerceIn(0f, 1f - h)
        val s = Stamp(newId(), page, NormRect(left, top, left + w, top + h), bmp)
        stamps += s
        selected = s.id
    }
}

private val UYGHUR_LETTERS = Regex("[\\u0600-\\u06FF]")

private fun typefaceFor(text: String): Typeface =
    if (UYGHUR_LETTERS.containsMatchIn(text)) {
        runCatching { Typeface.createFromAsset(Core.app.assets, "fonts/UKIJTuz.ttf") }.getOrDefault(Typeface.DEFAULT)
    } else Typeface.SANS_SERIF

/**
 * Renders text to a transparent bitmap using Android's own shaping (HarfBuzz,
 * bidi), so Uyghur/Arabic letters join and run right-to-left correctly.
 */
fun renderTextBitmap(text: String, color: Int, sizePx: Float = 128f): Bitmap {
    val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        this.color = color
        textSize = sizePx
        typeface = typefaceFor(text)
    }
    val maxW = text.lines().maxOf { paint.measureText(it) }.roundToInt().coerceAtLeast(1) + 8
    val layout = StaticLayout.Builder.obtain(text, 0, text.length, paint, maxW)
        .setAlignment(Layout.Alignment.ALIGN_NORMAL)
        .setTextDirection(TextDirectionHeuristics.FIRSTSTRONG_LTR)
        .setIncludePad(true)
        .build()
    val bmp = Bitmap.createBitmap(maxW, layout.height.coerceAtLeast(1), Bitmap.Config.ARGB_8888)
    layout.draw(AndroidCanvas(bmp))
    return bmp
}

fun Bitmap.toRgba(): ByteArray {
    val b = if (config == Bitmap.Config.ARGB_8888) this else copy(Bitmap.Config.ARGB_8888, false)
    val buf = ByteBuffer.allocate(b.byteCount)
    b.copyPixelsToBuffer(buf)
    return buf.array()
}

private val signatureFile get() = File(Core.app.filesDir, "signature.png")

fun savedSignature(): Bitmap? =
    signatureFile.takeIf { it.exists() }?.let { android.graphics.BitmapFactory.decodeFile(it.path) }

/** Finger-drawn signature; the strokes are cropped and returned as a bitmap. */
@Composable
fun SignaturePadDialog(onDone: (Bitmap) -> Unit, onDismiss: () -> Unit) {
    val saved = remember { savedSignature() }
    val strokes = remember { mutableStateListOf<MutableList<Offset>>() }
    var tick by remember { mutableIntStateOf(0) }
    var size by remember { mutableStateOf(androidx.compose.ui.geometry.Size.Zero) }
    val ink = Color(0xFF0B2A8A)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.signature)) },
        text = {
            Column {
                if (saved != null) {
                    OutlinedCard(onClick = { onDone(saved) }, modifier = Modifier.fillMaxWidth()) {
                        Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Image(saved.asImageBitmap(), null, Modifier.height(48.dp).weight(1f), contentScale = ContentScale.Fit)
                            Text(stringResource(R.string.use_saved_signature), style = MaterialTheme.typography.labelLarge)
                        }
                    }
                    Spacer(Modifier.height(10.dp))
                }
                Text(stringResource(R.string.draw_signature), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(6.dp))
                Canvas(
                    Modifier.fillMaxWidth().height(180.dp).clip(RoundedCornerShape(8.dp)).background(Color.White)
                        .border(1.dp, Color(0x33000000), RoundedCornerShape(8.dp))
                        .pointerInput(Unit) {
                            detectDragGestures(
                                onDragStart = { strokes += mutableListOf(it); tick++ },
                                onDrag = { ch, _ -> strokes.lastOrNull()?.add(ch.position); tick++; ch.consume() },
                            )
                        },
                ) {
                    size = this.size
                    tick.let { }
                    // baseline guide
                    drawLine(Color(0x22000000), Offset(16f, this.size.height * 0.75f), Offset(this.size.width - 16f, this.size.height * 0.75f), 2f)
                    for (s in strokes) {
                        if (s.size < 2) continue
                        val p = Path().apply { moveTo(s[0].x, s[0].y); for (pt in s.drop(1)) lineTo(pt.x, pt.y) }
                        drawPath(p, ink, style = Stroke(width = 5f, cap = StrokeCap.Round, join = StrokeJoin.Round))
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = strokes.isNotEmpty(), onClick = {
                renderStrokes(strokes, ink.toArgb())?.let { bmp ->
                    runCatching { signatureFile.outputStream().use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) } }
                    onDone(bmp)
                }
            }) { Text(stringResource(R.string.save)) }
        },
        dismissButton = {
            Row {
                TextButton(onClick = { strokes.clear(); tick++ }) { Text(stringResource(R.string.clear)) }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
            }
        },
    )
}

private fun renderStrokes(strokes: List<List<Offset>>, color: Int): Bitmap? {
    val pts = strokes.flatten()
    if (pts.isEmpty()) return null
    val pad = 8f
    val minX = pts.minOf { it.x } - pad
    val minY = pts.minOf { it.y } - pad
    val w = (pts.maxOf { it.x } + pad - minX).roundToInt().coerceAtLeast(4)
    val h = (pts.maxOf { it.y } + pad - minY).roundToInt().coerceAtLeast(4)
    val scale = 2f // render at 2x for crisp printing
    val bmp = Bitmap.createBitmap((w * scale).roundToInt(), (h * scale).roundToInt(), Bitmap.Config.ARGB_8888)
    val c = AndroidCanvas(bmp)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        this.color = color; style = Paint.Style.STROKE; strokeWidth = 5f * scale
        strokeCap = Paint.Cap.ROUND; strokeJoin = Paint.Join.ROUND
    }
    for (s in strokes) {
        if (s.size < 2) continue
        val p = AndroidPath()
        p.moveTo((s[0].x - minX) * scale, (s[0].y - minY) * scale)
        for (pt in s.drop(1)) p.lineTo((pt.x - minX) * scale, (pt.y - minY) * scale)
        c.drawPath(p, paint)
    }
    return bmp
}

@Composable
fun TextStampDialog(onDone: (String, Int) -> Unit, onDismiss: () -> Unit) {
    var v by remember { mutableStateOf("") }
    var color by remember { mutableIntStateOf(0xFF111111.toInt()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.add_text)) },
        text = {
            Column {
                OutlinedTextField(
                    value = v, onValueChange = { v = it }, minLines = 2, modifier = Modifier.fillMaxWidth(),
                    textStyle = MaterialTheme.typography.bodyLarge.copy(textDirection = androidx.compose.ui.text.style.TextDirection.Content),
                )
                Spacer(Modifier.height(10.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    listOf(0xFF111111.toInt(), 0xFF0B2A8A.toInt(), 0xFFC62828.toInt(), 0xFF2E7D32.toInt()).forEach { c ->
                        Box(
                            Modifier.size(30.dp).clip(CircleShape).background(Color(c))
                                .border(if (c == color) 3.dp else 0.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                .clickable { color = c },
                        )
                    }
                }
            }
        },
        confirmButton = { TextButton(enabled = v.isNotBlank(), onClick = { onDone(v.trim(), color) }) { Text(stringResource(R.string.ok)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Draggable, resizable stamps drawn over one page. */
@Composable
fun BoxScope.StampLayer(edit: EditState, page: Int) {
    val items = edit.stamps.filter { it.page == page }
    if (items.isEmpty()) return
    // Page coordinates are physical (left = left edge of the paper), so the
    // layer must not be mirrored when the app UI is right-to-left.
    CompositionLocalProvider(androidx.compose.ui.platform.LocalLayoutDirection provides androidx.compose.ui.unit.LayoutDirection.Ltr) {
    BoxWithConstraints(Modifier.matchParentSize()) {
        val pw = constraints.maxWidth.toFloat()
        val ph = constraints.maxHeight.toFloat()
        val density = LocalDensity.current
        for (s in items) {
            key(s.id) {
                val r = s.rect
                val selected = edit.selected == s.id
                val wDp = with(density) { ((r.right - r.left) * pw).toDp() }
                val hDp = with(density) { ((r.bottom - r.top) * ph).toDp() }
                Box(
                    Modifier.absoluteOffset { IntOffset((r.left * pw).roundToInt(), (r.top * ph).roundToInt()) }
                        .size(wDp, hDp)
                        .then(if (s.bitmap == null) Modifier.background(Color.White) else Modifier)
                        .border(if (selected) 1.5.dp else if (s.bitmap == null) 0.5.dp else 0.dp, if (selected) MaterialTheme.colorScheme.primary else Color(0x55000000))
                        .pointerInput(s.id) {
                            detectTapGestures(onTap = { edit.selected = s.id })
                        }
                        .pointerInput(s.id, pw, ph) {
                            detectDragGestures(onDragStart = { edit.selected = s.id }) { ch, d ->
                                ch.consume()
                                val cur = s.rect
                                val w = cur.right - cur.left
                                val h = cur.bottom - cur.top
                                val nl = (cur.left + d.x / pw).coerceIn(0f, 1f - w)
                                val nt = (cur.top + d.y / ph).coerceIn(0f, 1f - h)
                                s.rect = NormRect(nl, nt, nl + w, nt + h)
                            }
                        },
                ) {
                    s.bitmap?.let { Image(it.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds) }
                    if (selected) {
                        // delete
                        Box(
                            Modifier.align(Alignment.TopStart).offset((-12).dp, (-12).dp).size(24.dp).clip(CircleShape)
                                .background(MaterialTheme.colorScheme.error).clickable { edit.stamps.remove(s); edit.selected = null },
                            contentAlignment = Alignment.Center,
                        ) { Icon(Icons.Filled.Close, stringResource(R.string.delete), tint = Color.White, modifier = Modifier.size(16.dp)) }
                        // resize (keeps aspect ratio for images)
                        Box(
                            Modifier.align(Alignment.BottomEnd).offset(10.dp, 10.dp).size(22.dp).clip(CircleShape)
                                .background(MaterialTheme.colorScheme.primary)
                                .pointerInput(s.id, pw, ph) {
                                    detectDragGestures { ch, d ->
                                        ch.consume()
                                        val cur = s.rect
                                        var w = max(0.02f, cur.right - cur.left + d.x / pw)
                                        var h = if (s.bitmap != null) {
                                            w * s.bitmap.height / s.bitmap.width * pw / ph
                                        } else max(0.01f, cur.bottom - cur.top + d.y / ph)
                                        w = min(w, 1f - cur.left)
                                        h = min(h, 1f - cur.top)
                                        s.rect = NormRect(cur.left, cur.top, cur.left + w, cur.top + h)
                                    }
                                },
                        )
                    }
                }
            }
        }
    }
}
}
