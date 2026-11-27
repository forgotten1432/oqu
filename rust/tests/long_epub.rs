use std::io::Write;
use std::time::Instant;

fn make_epub(path: &std::path::Path, chapters: usize, paras: usize) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let st: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let df: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("mimetype", st).unwrap();
    z.write_all(b"application/epub+zip").unwrap();
    z.start_file("META-INF/container.xml", df).unwrap();
    z.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    let mut man = String::new();
    let mut spine = String::new();
    for i in 0..chapters {
        man.push_str(&format!(r#"<item id="c{i}" href="c{i}.xhtml" media-type="application/xhtml+xml"/>"#));
        spine.push_str(&format!(r#"<itemref idref="c{i}"/>"#));
        z.start_file(format!("OEBPS/c{i}.xhtml"), df).unwrap();
        let mut s = String::from(r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>t</title></head><body>"#);
        for p in 0..paras {
            s.push_str(&format!("<p id=\"p{p}\">مەن ئۈرۈمچىدە ئوقۇيمەن paragraph {p} lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>\n"));
        }
        s.push_str("</body></html>");
        z.write_all(s.as_bytes()).unwrap();
    }
    z.start_file("OEBPS/nav.xhtml", df).unwrap();
    let last = paras.saturating_sub(1);
    z.write_all(format!(r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>n</title></head><body><nav epub:type="toc"><ol><li><a href="c0.xhtml">Start</a></li><li><a href="c0.xhtml#p{last}">End</a></li></ol></nav></body></html>"#).as_bytes()).unwrap();
    man.push_str(r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#);
    z.start_file("OEBPS/content.opf", df).unwrap();
    z.write_all(format!(r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Long</dc:title></metadata><manifest>{man}</manifest><spine>{spine}</spine></package>"#).as_bytes()).unwrap();
    z.finish().unwrap();
}

#[test]
fn long_epub_opens() {
    let dir = std::env::temp_dir();
    for (ch, paras) in [(1usize, 60_000usize), (800, 200)] {
        let p = dir.join(format!("long_{ch}_{paras}.epub"));
        make_epub(&p, ch, paras);
        let t = Instant::now();
        let d = reader_core::reflow::ReflowDoc::open(p.to_string_lossy().into()).expect("open");
        eprintln!("{ch}x{paras}: open {:?} size {}", t.elapsed(), std::fs::metadata(&p).unwrap().len());
        let info = d.info();
        eprintln!("  chapters {} toc {:?}", info.chapters.len(), info.toc.iter().map(|t| (t.title.clone(), t.chapter)).collect::<Vec<_>>());
        if ch == 1 {
            let n = info.chapters.len() as u32;
            assert!(n > 20, "long chapter must be split, got {n}");
            assert_eq!(info.toc[0].chapter, 0);
            assert_eq!(info.toc[1].chapter, n - 1, "anchor in last part");
            assert_eq!(d.chapter_index(format!("OEBPS/c0.xhtml#p{}", paras - 1)), Some(n - 1));
            assert_eq!(d.chapter_index("OEBPS/c0.xhtml".into()), Some(0));
            assert_eq!(d.chapter_index("OEBPS/c0.part2.xhtml".into()), Some(1));
            for i in 0..n {
                let c = d.chapter(i).unwrap();
                assert_eq!(c.mime, "application/xhtml+xml", "part {i} must stay well-formed XHTML");
                assert!(c.data.len() < 400_000, "part {i}: {}", c.data.len());
            }
        } else {
            assert_eq!(info.chapters.len(), ch, "small chapters untouched");
        }
        let t = Instant::now();
        let c = d.chapter(0).unwrap();
        eprintln!("  chapter0 {:?} {} bytes mime {}", t.elapsed(), c.data.len(), c.mime);
        let t = Instant::now();
        let h = d.search("ürümchide".into(), 1000);
        eprintln!("  search {:?} hits {}", t.elapsed(), h.len());
    }
}

