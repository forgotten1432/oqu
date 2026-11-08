//! PDF engine: Chromium's PDFium (via `pdfium-render`), owned by a single
//! worker thread. PDFium is not thread-safe, and `PdfDocument<'a>` borrows
//! from `Pdfium`; keeping both on one thread sidesteps both problems, and
//! every UniFFI call becomes a message to that thread.

use crate::rtl;
use crate::{ReaderError, Result};
use pdfium_render::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, sync_channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};

struct State {
    pdfium: &'static Pdfium,
    docs: HashMap<u64, PdfDocument<'static>>,
}

type Job = Box<dyn FnOnce(&mut State) + Send>;

static WORKER: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn pdf_err(e: PdfiumError) -> ReaderError {
    match e {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => ReaderError::Password,
        other => ReaderError::Pdf { msg: other.to_string() },
    }
}

/// Loads libpdfium.so and starts the worker thread. Safe to call repeatedly.
/// `library_path` is tried first (e.g. `nativeLibraryDir/libpdfium.so`),
/// then the bare soname (works for uncompressed libs inside the APK).
#[uniffi::export]
pub fn pdf_init(library_path: String) -> Result<()> {
    if WORKER.get().is_some() {
        return Ok(());
    }
    let (ready_tx, ready_rx) = sync_channel::<std::result::Result<(), String>>(1);
    let (tx, rx) = channel::<Job>();
    std::thread::Builder::new()
        .name("pdfium-worker".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let bindings = Pdfium::bind_to_library(&library_path)
                .or_else(|_| Pdfium::bind_to_library("libpdfium.so"))
                .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")));
            let bindings = match bindings {
                Ok(b) => b,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let pdfium: &'static Pdfium = Box::leak(Box::new(Pdfium::new(bindings)));
            let mut state = State { pdfium, docs: HashMap::new() };
            let _ = ready_tx.send(Ok(()));
            while let Ok(job) = rx.recv() {
                // A panic inside one job must not kill the engine.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut state)));
            }
        })?;
    match ready_rx.recv() {
        Ok(Ok(())) => {
            let _ = WORKER.set(Mutex::new(tx));
            Ok(())
        }
        Ok(Err(msg)) => Err(ReaderError::Pdf { msg: format!("cannot load libpdfium: {msg}") }),
        Err(_) => Err(ReaderError::Pdf { msg: "pdfium worker died".into() }),
    }
}

fn run<R: Send + 'static>(f: impl FnOnce(&mut State) -> Result<R> + Send + 'static) -> Result<R> {
    let worker = WORKER.get().ok_or(ReaderError::Pdf { msg: "pdf_init was not called".into() })?;
    let (tx, rx) = sync_channel(1);
    let job: Job = Box::new(move |s: &mut State| {
        let _ = tx.send(f(s));
    });
    worker
        .lock()
        .map_err(|_| ReaderError::Pdf { msg: "worker lock poisoned".into() })?
        .send(job)
        .map_err(|_| ReaderError::Pdf { msg: "pdfium worker stopped".into() })?;
    rx.recv().unwrap_or(Err(ReaderError::Pdf { msg: "job panicked".into() }))
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PageSize {
    /// Width/height in PDF points (1/72 inch).
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct OutlineItem {
    pub title: String,
    pub page: i32,
    pub level: u32,
}

/// A rectangle normalized to the page: 0..1, origin top-left.
#[derive(Debug, Clone, uniffi::Record)]
pub struct NormRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PdfSearchHit {
    pub page: i32,
    pub rects: Vec<NormRect>,
    /// Surrounding text, in logical order, for the results list.
    pub context: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PdfInfo {
    pub page_count: i32,
    pub title: Option<String>,
    pub author: Option<String>,
    pub sizes: Vec<PageSize>,
    /// Dominant script direction of the first pages' text (for reading UI defaults).
    pub direction: rtl::TextDirection,
    /// "ug" when the text is detected as Uyghur.
    pub language: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PdfLink {
    pub rect: NormRect,
    pub page: Option<i32>,
    pub uri: Option<String>,
}

#[derive(uniffi::Object)]
pub struct PdfDoc {
    id: u64,
    info: PdfInfo,
    // Text-geometry cache per page for repeated searches (page -> chars).
    text_cache: Mutex<lru::LruCache<i32, Arc<PageText>>>,
}

struct PageText {
    chars: Vec<char>,
    boxes: Vec<Option<NormRect>>,
}

#[uniffi::export]
impl PdfDoc {
    #[uniffi::constructor]
    pub fn open(path: String, password: Option<String>) -> Result<Arc<Self>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let info = run(move |s| {
            let doc = s.pdfium.load_pdf_from_file(&path, password.as_deref()).map_err(pdf_err)?;
            let pages = doc.pages();
            let sizes = pages
                .page_sizes()
                .map_err(pdf_err)?
                .into_iter()
                .map(|r| PageSize { width: r.width().value, height: r.height().value })
                .collect::<Vec<_>>();
            let meta = |t| doc.metadata().get(t).map(|m| m.value().to_string()).filter(|v| !v.trim().is_empty());
            let mut sample = String::new();
            for i in 0..pages.len().min(5) {
                if let Ok(p) = pages.get(i) {
                    if let Ok(t) = p.text() {
                        sample.push_str(&t.all());
                    }
                }
            }
            let info = PdfInfo {
                page_count: pages.len() as i32,
                title: meta(PdfDocumentMetadataTagType::Title),
                author: meta(PdfDocumentMetadataTagType::Author),
                sizes,
                direction: rtl::dominant(&sample),
                language: if rtl::looks_uyghur(&sample) { Some("ug".into()) } else { None },
            };
            s.docs.insert(id, doc);
            Ok(info)
        })?;
        Ok(Arc::new(PdfDoc {
            id,
            info,
            text_cache: Mutex::new(lru::LruCache::new(std::num::NonZeroUsize::new(64).unwrap())),
        }))
    }

    pub fn info(&self) -> PdfInfo {
        self.info.clone()
    }

    /// Renders a tile of a page laid out at `full_width`×`full_height` pixels.
    /// Returns tightly packed RGBA_8888 (Android `Bitmap.Config.ARGB_8888` byte
    /// order) of size `tile_w*tile_h*4`. Pass the full size as the tile to
    /// render the whole page.
    pub fn render_tile(
        &self,
        page: i32,
        full_width: i32,
        full_height: i32,
        tile_x: i32,
        tile_y: i32,
        tile_w: i32,
        tile_h: i32,
        with_annotations: bool,
    ) -> Result<Vec<u8>> {
        let id = self.id;
        run(move |s| {
            let doc = s.docs.get(&id).ok_or(ReaderError::Closed)?;
            let p = doc.pages().get(page).map_err(pdf_err)?;
            let mut bmp = PdfBitmap::empty(tile_w.max(1), tile_h.max(1), PdfBitmapFormat::BGRA).map_err(pdf_err)?;
            let whole = tile_x == 0 && tile_y == 0 && tile_w == full_width && tile_h == full_height;
            let mut cfg = PdfRenderConfig::new()
                .set_target_size(full_width, full_height)
                .set_reverse_byte_order(true) // BGRA -> RGBA
                .clear_before_rendering(true)
                .set_clear_color(PdfColor::WHITE)
                .render_annotations(with_annotations)
                .use_lcd_text_rendering(false)
                .set_text_smoothing(true)
                .set_image_smoothing(true)
                .set_path_smoothing(true);
            if whole {
                cfg = cfg.render_form_data(true);
            } else {
                cfg = cfg.set_origin(-tile_x, -tile_y);
            }
            p.render_into_bitmap_with_config(&mut bmp, &cfg).map_err(pdf_err)?;
            Ok(bmp.as_raw_bytes())
        })
    }

    pub fn outline(&self) -> Result<Vec<OutlineItem>> {
        let id = self.id;
        run(move |s| {
            let doc = s.docs.get(&id).ok_or(ReaderError::Closed)?;
            let mut out = Vec::new();
            fn walk(b: PdfBookmark, level: u32, out: &mut Vec<OutlineItem>) {
                if out.len() > 5000 {
                    return;
                }
                let page = b
                    .destination()
                    .and_then(|d| d.page_index().ok())
                    .map(|p| p as i32)
                    .unwrap_or(-1);
                out.push(OutlineItem { title: b.title().unwrap_or_default(), page, level });
                for c in b.iter_direct_children() {
                    walk(c, level + 1, out);
                }
            }
            if let Some(root) = doc.bookmarks().root() {
                for b in root.iter_siblings() {
                    walk(b, 0, &mut out);
                }
            }
            Ok(out)
        })
    }

    /// Plain text of a page in logical order (used by Reading Mode).
    pub fn page_text(&self, page: i32) -> Result<String> {
        let id = self.id;
        run(move |s| {
            let doc = s.docs.get(&id).ok_or(ReaderError::Closed)?;
            let p = doc.pages().get(page).map_err(pdf_err)?;
            let t = p.text().map_err(pdf_err)?.all();
            Ok(t)
        })
        .map(|raw| {
            // Prefer the geometry-rebuilt logical text when the page has RTL script.
            if crate::rtl::dominant(&raw) == rtl::TextDirection::Rtl || raw.chars().any(|c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFEFF)) {
                self.page_text_geometry(page).map(|pt| pt.chars.iter().collect()).unwrap_or(raw)
            } else {
                raw
            }
        })
    }

    pub fn links(&self, page: i32) -> Result<Vec<PdfLink>> {
        let id = self.id;
        run(move |s| {
            let doc = s.docs.get(&id).ok_or(ReaderError::Closed)?;
            let p = doc.pages().get(page).map_err(pdf_err)?;
            let (w, h) = (p.width().value, p.height().value);
            let mut out = Vec::new();
            for l in p.links().iter() {
                let Ok(r) = l.rect() else { continue };
                let target_page = l.destination().and_then(|d| d.page_index().ok()).map(|v| v as i32);
                let uri = l.action().and_then(|a| match a {
                    PdfAction::Uri(u) => u.uri().ok(),
                    _ => None,
                });
                if target_page.is_none() && uri.is_none() {
                    continue;
                }
                out.push(PdfLink { rect: norm(&r, w, h), page: target_page, uri });
            }
            Ok(out)
        })
    }

    /// RTL-aware search on one page. The UI calls this page by page so it can
    /// show results progressively and cancel.
    pub fn search_page(&self, page: i32, query: String) -> Result<Vec<PdfSearchHit>> {
        let pt = self.page_text_geometry(page)?;
        let src: String = pt.chars.iter().collect();
        let p = if self.info.language.as_deref() == Some("ug") { rtl::FoldProfile::Uyghur } else { rtl::profile_for(&src, &query) };
        let needles = rtl::needles(&query, p);
        if needles.is_empty() {
            return Ok(vec![]);
        }
        let (folded, map) = rtl::fold_with_map(&src, p);
        let mut hits = Vec::new();
        for (start, len) in rtl::find_any(&folded, &needles) {
            let end = start + len - 1;
            let (s_src, e_src) = (map[start], map[end]);
            let rects = merge_boxes(&pt.boxes[s_src..=e_src]);
            let ctx_from = s_src.saturating_sub(40);
            let ctx_to = (e_src + 41).min(pt.chars.len());
            let context: String = pt.chars[ctx_from..ctx_to]
                .iter()
                .map(|c| if c.is_control() { ' ' } else { *c })
                .collect();
            hits.push(PdfSearchHit { page, rects, context: context.trim().to_string() });
        }
        Ok(hits)
    }

    /// Text of the characters inside a normalized rect (text selection/copy).
    pub fn text_in_rect(&self, page: i32, rect: NormRect) -> Result<String> {
        let pt = self.page_text_geometry(page)?;
        let mut s = String::new();
        for (c, b) in pt.chars.iter().zip(pt.boxes.iter()) {
            match b {
                Some(b) if b.left < rect.right && b.right > rect.left && b.top < rect.bottom && b.bottom > rect.top => s.push(*c),
                None if !s.is_empty() && (*c == '\n' || *c == ' ' || *c == '\r') => s.push(*c),
                _ => {}
            }
        }
        Ok(s.trim().to_string())
    }

    pub fn release(&self) {
        let id = self.id;
        let _ = run(move |s| {
            s.docs.remove(&id);
            Ok(())
        });
    }
}

impl PdfDoc {
    fn page_text_geometry(&self, page: i32) -> Result<Arc<PageText>> {
        if let Some(pt) = self.text_cache.lock().unwrap().get(&page) {
            return Ok(pt.clone());
        }
        let id = self.id;
        let pt = run(move |s| {
            let doc = s.docs.get(&id).ok_or(ReaderError::Closed)?;
            let p = doc.pages().get(page).map_err(pdf_err)?;
            let (w, h) = (p.width().value, p.height().value);
            let text = p.text().map_err(pdf_err)?;
            let mut chars = Vec::new();
            let mut boxes = Vec::new();
            for ch in text.chars().iter() {
                let Some(c) = ch.unicode_char() else { continue };
                chars.push(c);
                boxes.push(ch.loose_bounds().ok().filter(|r| r.width().value > 0.0 || c.is_whitespace()).map(|r| norm(&r, w, h)));
            }
            let (chars, boxes) = crate::pdf_layout::reconstruct(&chars, &boxes);
            Ok(PageText { chars, boxes })
        })?;
        let pt = Arc::new(pt);
        self.text_cache.lock().unwrap().put(page, pt.clone());
        Ok(pt)
    }
}

impl Drop for PdfDoc {
    fn drop(&mut self) {
        self.release();
    }
}

fn norm(r: &PdfRect, w: f32, h: f32) -> NormRect {
    let w = if w <= 0.0 { 1.0 } else { w };
    let h = if h <= 0.0 { 1.0 } else { h };
    NormRect {
        left: (r.left().value / w).clamp(0.0, 1.0),
        right: (r.right().value / w).clamp(0.0, 1.0),
        top: (1.0 - r.top().value / h).clamp(0.0, 1.0),
        bottom: (1.0 - r.bottom().value / h).clamp(0.0, 1.0),
    }
}

/// Merges per-character boxes into one rect per visual line. Works for RTL
/// runs too, because we only union boxes, never assume left-to-right order.
fn merge_boxes(boxes: &[Option<NormRect>]) -> Vec<NormRect> {
    let mut lines: Vec<NormRect> = Vec::new();
    for b in boxes.iter().flatten() {
        let mid = (b.top + b.bottom) / 2.0;
        if let Some(l) = lines.iter_mut().find(|l| mid > l.top && mid < l.bottom) {
            l.left = l.left.min(b.left);
            l.right = l.right.max(b.right);
            l.top = l.top.min(b.top);
            l.bottom = l.bottom.max(b.bottom);
        } else {
            lines.push(b.clone());
        }
    }
    lines
}
