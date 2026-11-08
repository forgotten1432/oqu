use reader_core::pdf::{pdf_init, NormRect, PdfDoc};
use reader_core::pdf_edit::{pdf_save_edits, PageOp, PdfEdit, PdfEditKind};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    pdf_init(a[1].clone()).unwrap();
    let (src, out) = (a[2].clone(), a[3].clone());
    // a 40x20 opaque blue "signature"
    let (w, h) = (40u32, 20u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for px in rgba.chunks_mut(4) { px.copy_from_slice(&[20, 40, 200, 255]); }
    let r = |l: f32, t: f32, rr: f32, b: f32| NormRect { left: l, top: t, right: rr, bottom: b };
    let before = PdfDoc::open(src.clone(), None).unwrap();
    let n0 = before.info().page_count;
    let px0 = before.render_tile(0, 300, 388, 0, 0, 300, 388, true).unwrap();
    pdf_save_edits(src, None, out.clone(), vec![
        PdfEdit { page: 0, rect: r(0.6, 0.8, 0.9, 0.9), kind: PdfEditKind::Image { rgba, width: w, height: h } },
        PdfEdit { page: 0, rect: r(0.05, 0.05, 0.4, 0.1), kind: PdfEditKind::WhiteOut },
        PdfEdit { page: 0, rect: r(0.1, 0.3, 0.5, 0.33), kind: PdfEditKind::Highlight { color: 0xFFFFD54F } },
        PdfEdit { page: 0, rect: r(0.1, 0.5, 0.12, 0.52), kind: PdfEditKind::Note { text: "ئىزاھات".into(), color: 0xFFFFC107 } },
    ], vec![PageOp::Rotate { page: 0, quarter_turns: 1 }]).unwrap();
    let after = PdfDoc::open(out, None).unwrap();
    let i = after.info();
    println!("pages {n0} -> {} ; size0 {:?}", i.page_count, i.sizes[0]);
    let px1 = after.render_tile(0, 300, 232, 0, 0, 300, 232, true).unwrap();
    let blue = px1.chunks(4).filter(|p| p[2] > 150 && p[0] < 60).count();
    std::fs::write("/tmp/edited.rgba", &px1).unwrap();
    // second pass on the now-rotated page: a wide red bar at the display top-left
    let (w2, h2) = (60u32, 10u32);
    let mut red = vec![0u8; (w2 * h2 * 4) as usize];
    for (i, px) in red.chunks_mut(4).enumerate() { let x = i as u32 % w2; px.copy_from_slice(if x < 10 { &[0, 160, 0, 255] } else { &[220, 20, 20, 255] }); }
    let out2 = "/tmp/edited2.pdf".to_string();
    pdf_save_edits(a[3].clone(), None, out2.clone(), vec![PdfEdit { page: 0, rect: r(0.05, 0.05, 0.45, 0.12), kind: PdfEditKind::Image { rgba: red, width: w2, height: h2 } }], vec![]).unwrap();
    let d2 = PdfDoc::open(out2, None).unwrap();
    let px2 = d2.render_tile(0, 300, 232, 0, 0, 300, 232, true).unwrap();
    let at = |x: usize, y: usize| { let i = (y * 300 + x) * 4; [px2[i], px2[i+1], px2[i+2]] };
    // expect green (left end) near display x=0.06, red near x=0.4, at y ~ 0.085
    println!("left end {:?}  right part {:?}", at(20, 20), at(110, 20));
    println!("blue px after = {blue} (before rendering size {})", px0.len());
}
#[allow(dead_code)]
fn unused() {}
