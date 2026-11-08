//! Desktop smoke test: `cargo run --example smoke -- <pdfium lib> <files...>`
use reader_core::pdf::{pdf_init, PdfDoc};
use reader_core::reflow::ReflowDoc;
use reader_core::{detect_format, DocFormat};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = args.next().expect("pdfium library path");
    for path in args {
        let fmt = detect_format(path.clone());
        println!("\n=== {path} -> {fmt:?}");
        let t = Instant::now();
        if fmt == DocFormat::Pdf {
            pdf_init(lib.clone()).expect("pdf init");
            let d = PdfDoc::open(path.clone(), None).expect("open pdf");
            let info = d.info();
            println!("lang={:?} pages={} title={:?} dir={:?} size0={:?} open={:?}", info.language, info.page_count, info.title, info.direction, info.sizes[0], t.elapsed());
            let t = Instant::now();
            let w = 1080;
            let h = (w as f32 * info.sizes[0].height / info.sizes[0].width) as i32;
            let px = d.render_tile(0, w, h, 0, 0, w, h, true).expect("render");
            assert_eq!(px.len(), (w * h * 4) as usize);
            let dark = px.chunks(4).filter(|p| p[0] < 128).count();
            println!("render {w}x{h} in {:?}, dark px={dark}, first px RGBA={:?}", t.elapsed(), &px[..4]);
            let tile = d.render_tile(0, w * 3, h * 3, 512, 512, 256, 256, false).expect("tile");
            assert_eq!(tile.len(), 256 * 256 * 4);
            let text = d.page_text(0).unwrap();
            println!("text: {}", text.chars().take(120).collect::<String>().replace('\n', " | "));
            for q in ["الكتاب", "كتاب", "שלום", "English", "123", "كىتاب", "kitab", "ürümchide", "séning", "تيل", "تىل"] {
                let hits = d.search_page(0, q.into()).unwrap();
                println!("search {q:?}: {} hits {:?}", hits.len(), hits.first().map(|h| (&h.rects, &h.context)));
            }
            println!("outline: {:?}", d.outline().unwrap());
        } else {
            let d = match ReflowDoc::open(path.clone()) {
                Ok(d) => d,
                Err(e) => {
                    println!("ERROR {e}");
                    continue;
                }
            };
            let info = d.info();
            println!(
                "title={:?} author={:?} lang={:?} dir={:?} chapters={} open={:?}",
                info.title, info.author, info.language, info.direction, info.chapters.len(), t.elapsed()
            );
            for t in &info.toc {
                println!("  toc {}{} -> ch{} #{:?}", "  ".repeat(t.level as usize), t.title, t.chapter, t.anchor);
            }
            let ch = d.chapter(0).unwrap();
            let html = String::from_utf8_lossy(&ch.data);
            println!("chapter0 mime={} len={}", ch.mime, html.len());
            println!("{}", html.chars().take(700).collect::<String>());
            for q in ["الكتاب", "كتاب", "שלום", "عريض", "كىتاب", "kitab", "ürümchide", "séning", "تىل", "تيل"] {
                let hits = d.search(q.into(), 10);
                println!("search {q:?}: {:?}", hits.iter().map(|h| (h.chapter, h.occurrence, h.snippet.clone())).collect::<Vec<_>>());
            }
            if let Some(c) = d.cover() {
                println!("cover {} {} bytes", c.mime, c.data.len());
            }
        }
    }
}
