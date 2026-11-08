//! Fill & Sign / light editing: stamps app-rendered images (signatures, typed
//! text, dates, check marks), white-out boxes, real PDF highlight and note
//! annotations, and page rotation/deletion, then saves a new PDF.
//!
//! Typed text arrives as an image rendered by Android (HarfBuzz + ICU), because
//! PDFium's own text objects cannot shape Uyghur/Arabic script.

use crate::pdf::{pdf_err, run, NormRect, PageGeom};
use crate::{ReaderError, Result};
use pdfium_render::prelude::*;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PdfEditKind {
    /// Straight (non-premultiplied) RGBA pixels, `width*height*4` bytes.
    Image { rgba: Vec<u8>, width: u32, height: u32 },
    /// Opaque white rectangle covering existing content.
    WhiteOut,
    /// Highlight annotation; color is 0xAARRGGBB.
    Highlight { color: u32 },
    /// Sticky-note annotation at the rect's top-left corner.
    Note { text: String, color: u32 },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PdfEdit {
    pub page: i32,
    /// Normalized to the page, origin top-left (same space as search hits).
    pub rect: NormRect,
    pub kind: PdfEditKind,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PageOp {
    /// Clockwise quarter turns added to the page's rotation.
    Rotate { page: i32, quarter_turns: i32 },
    Delete { page: i32 },
}

fn color(argb: u32) -> PdfColor {
    PdfColor::new((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}

fn rotation_from_quarters(q: i32) -> PdfPageRenderRotation {
    match q.rem_euclid(4) {
        1 => PdfPageRenderRotation::Degrees90,
        2 => PdfPageRenderRotation::Degrees180,
        3 => PdfPageRenderRotation::Degrees270,
        _ => PdfPageRenderRotation::None,
    }
}

fn quarters(r: PdfPageRenderRotation) -> i32 {
    match r {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 1,
        PdfPageRenderRotation::Degrees180 => 2,
        PdfPageRenderRotation::Degrees270 => 3,
    }
}

/// Applies `edits` and `ops` to the PDF at `src_path` and writes `out_path`.
/// The source file is not modified (`out_path` must differ from it).
#[uniffi::export]
pub fn pdf_save_edits(
    src_path: String,
    password: Option<String>,
    out_path: String,
    edits: Vec<PdfEdit>,
    ops: Vec<PageOp>,
) -> Result<()> {
    if src_path == out_path {
        return Err(ReaderError::Io { msg: "output must differ from input".into() });
    }
    run(move |s| {
        let doc = s.pdfium.load_pdf_from_file(&src_path, password.as_deref()).map_err(pdf_err)?;
        for e in &edits {
            let mut page = doc.pages().get(e.page).map_err(pdf_err)?;
            let g = PageGeom::of(&page);
            let n = NormRect {
                left: e.rect.left.min(e.rect.right),
                right: e.rect.left.max(e.rect.right),
                top: e.rect.top.min(e.rect.bottom),
                bottom: e.rect.top.max(e.rect.bottom),
            };
            let rect = g.content_rect(&n);
            let (left, right, top, bottom) = (rect.left().value, rect.right().value, rect.top().value, rect.bottom().value);
            match &e.kind {
                PdfEditKind::Image { rgba, width, height } => {
                    let (iw, ih) = (*width as i32, *height as i32);
                    if rgba.len() < (iw * ih * 4) as usize || iw <= 0 || ih <= 0 {
                        return Err(ReaderError::Parse { msg: "bad image size".into() });
                    }
                    // RGBA -> BGRA, as PDFium expects
                    let stride = (iw * 4) as usize;
                    let mut bgra = vec![0u8; stride * ih as usize];
                    for (src, dst) in rgba.chunks_exact(4).zip(bgra.chunks_exact_mut(4)) {
                        let a = src[3] as u32;
                        // Android bitmaps are premultiplied; PDF images are not.
                        let un = |c: u8| if a == 0 || a == 255 { c } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
                        dst[0] = un(src[2]);
                        dst[1] = un(src[1]);
                        dst[2] = un(src[0]);
                        dst[3] = src[3];
                    }
                    let bitmap = PdfBitmap::from_bytes(iw, ih, PdfBitmapFormat::BGRA, &mut bgra).map_err(pdf_err)?;
                    let mut obj = PdfPageImageObject::new(&doc).map_err(pdf_err)?;
                    obj.set_bitmap(&bitmap).map_err(pdf_err)?;
                    // Map the image's unit square so it appears upright on the
                    // displayed (possibly rotated) page.
                    let bl = g.to_content(n.left, n.bottom);
                    let br = g.to_content(n.right, n.bottom);
                    let tl = g.to_content(n.left, n.top);
                    obj.transform(br.0 - bl.0, br.1 - bl.1, tl.0 - bl.0, tl.1 - bl.1, bl.0, bl.1).map_err(pdf_err)?;
                    page.objects_mut().add_image_object(obj).map_err(pdf_err)?;
                }
                PdfEditKind::WhiteOut => {
                    page.objects_mut()
                        .create_path_object_rect(rect, None, None, Some(PdfColor::WHITE))
                        .map_err(pdf_err)?;
                }
                PdfEditKind::Highlight { color: c } => {
                    let mut a = page.annotations_mut().create_highlight_annotation().map_err(pdf_err)?;
                    a.set_bounds(rect).map_err(pdf_err)?;
                    a.attachment_points_mut()
                        .create_attachment_point_at_end(PdfQuadPoints::new(
                            // Acrobat order: upper-left, upper-right, lower-left, lower-right
                            PdfPoints::new(left), PdfPoints::new(top),
                            PdfPoints::new(right), PdfPoints::new(top),
                            PdfPoints::new(left), PdfPoints::new(bottom),
                            PdfPoints::new(right), PdfPoints::new(bottom),
                        ))
                        .map_err(pdf_err)?;
                    a.set_stroke_color(color(*c)).map_err(pdf_err)?;
                    a.set_fill_color(color(*c)).map_err(pdf_err)?;
                }
                PdfEditKind::Note { text, color: c } => {
                    let mut a = page.annotations_mut().create_text_annotation(text).map_err(pdf_err)?;
                    let (nx, ny) = g.to_content(n.left, n.top);
                    let r = PdfRect::new_from_values(ny - 20.0, nx, ny, nx + 20.0);
                    a.set_bounds(r).map_err(pdf_err)?;
                    a.set_stroke_color(color(*c)).map_err(pdf_err)?;
                }
            }
        }
        let mut deletes = Vec::new();
        for op in &ops {
            match op {
                PageOp::Rotate { page, quarter_turns } => {
                    let mut p = doc.pages().get(*page).map_err(pdf_err)?;
                    let cur = p.rotation().map(quarters).unwrap_or(0);
                    p.set_rotation(rotation_from_quarters(cur + quarter_turns));
                }
                PageOp::Delete { page } => deletes.push(*page),
            }
        }
        deletes.sort_unstable();
        deletes.dedup();
        if deletes.len() as i32 >= doc.pages().len() as i32 {
            return Err(ReaderError::Parse { msg: "cannot delete every page".into() });
        }
        for p in deletes.into_iter().rev() {
            doc.pages().get(p).map_err(pdf_err)?.delete().map_err(pdf_err)?;
        }
        doc.save_to_file(&out_path).map_err(pdf_err)?;
        Ok(())
    })
}
