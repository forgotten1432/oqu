//! Reflowable documents. Every format is turned into a `Book`: a list of
//! chapters, each served to the WebView at a virtual path under
//! `https://book.local/`, plus resources (images, CSS, fonts) at their own
//! paths so relative URLs inside chapters resolve naturally.

mod docx;
mod epub;
mod fb2;
mod odt;
mod sanitize;
mod simple;

use crate::rtl::{self, TextDirection};
use crate::util;
use crate::{DocFormat, ReaderError, Result};
use std::collections::HashMap;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

pub const HEAD_MARKER: &str = "<!--READER_HEAD-->";

#[derive(Debug, Clone, uniffi::Record)]
pub struct TocItem {
    pub title: String,
    pub chapter: u32,
    /// Fragment id inside the chapter, without '#'.
    pub anchor: Option<String>,
    pub level: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ChapterRef {
    /// Path relative to `https://book.local/`.
    pub path: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookInfo {
    pub title: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub direction: TextDirection,
    pub format: DocFormat,
    pub chapters: Vec<ChapterRef>,
    pub toc: Vec<TocItem>,
    /// Fixed-layout content (comics) should not be re-paginated.
    pub fixed_layout: bool,
    /// Text is Uyghur (or related): search keeps ى/ي, ې, ۇ/ۆ/ۈ distinct and
    /// accepts Uyghur Latin (ULY) queries.
    pub uyghur: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Resource {
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ReflowHit {
    pub chapter: u32,
    /// n-th match inside the chapter (0-based), so the UI can jump to it.
    pub occurrence: u32,
    pub snippet: String,
}

/// A match located inside a list of DOM text nodes (offsets in UTF-16 units,
/// ready for `document.createRange()`).
#[derive(Debug, Clone, uniffi::Record)]
pub struct NodeRange {
    pub start_node: u32,
    pub start_offset: u32,
    pub end_node: u32,
    pub end_offset: u32,
}

pub(crate) enum Store {
    Zip(Mutex<zip::ZipArchive<BufReader<std::fs::File>>>),
    Memory(HashMap<String, (String, Vec<u8>)>),
    Dir(std::path::PathBuf),
}

impl Store {
    fn open_zip(path: &str) -> Result<Store> {
        let f = std::fs::File::open(path)?;
        Ok(Store::Zip(Mutex::new(zip::ZipArchive::new(BufReader::new(f))?)))
    }

    fn get(&self, path: &str) -> Option<Vec<u8>> {
        match self {
            Store::Zip(z) => {
                let mut g = z.lock().ok()?;
                util::read_zip_entry(&mut g, path).ok()
            }
            Store::Memory(m) => m.get(path).map(|(_, d)| d.clone()),
            Store::Dir(root) => {
                let p = root.join(util::normalize(path));
                // never escape the document's directory
                if !p.starts_with(root) {
                    return None;
                }
                std::fs::read(p).ok()
            }
        }
    }

    fn mime(&self, path: &str) -> String {
        match self {
            Store::Memory(m) => m.get(path).map(|(t, _)| t.clone()).unwrap_or_else(|| util::mime_for(path).into()),
            _ => util::mime_for(path).into(),
        }
    }
}

pub(crate) struct Chapter {
    pub path: String,
    pub title: Option<String>,
    /// Pre-rendered HTML (converted formats); `None` = load from store.
    pub html: Option<String>,
}

pub(crate) struct Book {
    pub title: String,
    pub author: Option<String>,
    pub language: Option<String>,
    pub direction: Option<TextDirection>,
    pub chapters: Vec<Chapter>,
    pub toc: Vec<TocItem>,
    pub store: Store,
    pub cover: Option<String>,
    pub fixed_layout: bool,
}

#[derive(uniffi::Object)]
pub struct ReflowDoc {
    book: Book,
    format: DocFormat,
    direction: TextDirection,
    profile: rtl::FoldProfile,
    text_cache: Mutex<HashMap<(u32, bool), Arc<FoldedText>>>,
}

struct FoldedText {
    folded: Vec<char>,
    map: Vec<usize>,
    original: Vec<char>,
}

#[uniffi::export]
impl ReflowDoc {
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>> {
        let format = crate::detect_format(path.clone());
        let mut book = match format {
            DocFormat::Epub => epub::open(&path)?,
            DocFormat::Docx => docx::open(&path)?,
            DocFormat::Odt => odt::open(&path)?,
            DocFormat::Fb2 => fb2::open(&path)?,
            DocFormat::Html => simple::open_html(&path)?,
            DocFormat::Txt => simple::open_txt(&path)?,
            DocFormat::Markdown => simple::open_markdown(&path)?,
            DocFormat::Cbz => simple::open_cbz(&path)?,
            DocFormat::Pdf => return Err(ReaderError::Unsupported { msg: "use PdfDoc for PDF".into() }),
            DocFormat::Unknown => return Err(ReaderError::Unsupported { msg: path }),
        };
        if book.chapters.is_empty() {
            return Err(ReaderError::Parse { msg: "document has no readable content".into() });
        }
        if book.title.trim().is_empty() {
            book.title = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        // Sample the first chapters' text (skipping near-empty title pages).
        let mut sample = String::new();
        for i in 0..book.chapters.len().min(6) {
            if let Some(h) = raw_chapter(&book, i) {
                sample.push_str(&util::html_to_text(&h));
            }
            if sample.len() > 20_000 {
                break;
            }
        }
        // Untagged Uyghur text gets lang="ug" so HarfBuzz/ICU use Uyghur rules.
        let lang_is_generic = book.language.as_deref().map(|l| {
            let l = l.to_ascii_lowercase();
            l.is_empty() || l == "und" || l.starts_with("ar") || l.starts_with("en") || l.starts_with("fa")
        });
        if lang_is_generic.unwrap_or(true) && rtl::looks_uyghur(&sample) {
            book.language = Some("ug".into());
        }
        let profile = if book.language.as_deref().map(|l| l.to_ascii_lowercase().starts_with("ug")).unwrap_or(false) {
            rtl::FoldProfile::Uyghur
        } else {
            rtl::detect_profile(&sample)
        };
        let direction = match book.direction {
            Some(d) => d,
            None if book.language.as_deref().map(rtl::is_rtl_lang).unwrap_or(false) => TextDirection::Rtl,
            None => rtl::dominant(&sample),
        };
        Ok(Arc::new(ReflowDoc { book, format, direction, profile, text_cache: Mutex::new(HashMap::new()) }))
    }

    pub fn info(&self) -> BookInfo {
        BookInfo {
            title: self.book.title.clone(),
            author: self.book.author.clone(),
            language: self.book.language.clone(),
            direction: self.direction,
            format: self.format,
            chapters: self
                .book
                .chapters
                .iter()
                .map(|c| ChapterRef { path: c.path.clone(), title: c.title.clone() })
                .collect(),
            toc: self.book.toc.clone(),
            fixed_layout: self.book.fixed_layout,
            uyghur: self.profile == rtl::FoldProfile::Uyghur,
        }
    }

    /// Index of the chapter served at `path`, if any.
    pub fn chapter_index(&self, path: String) -> Option<u32> {
        let p = util::normalize(&util::percent_decode(path.split('#').next().unwrap_or("")));
        self.book.chapters.iter().position(|c| c.path == p).map(|i| i as u32)
    }

    /// A complete HTML document for chapter `index`, sanitized, with `dir`/`lang`
    /// set and a `<!--READER_HEAD-->` marker where the app injects its CSS/JS.
    /// Returns (mime, html).
    pub fn chapter(&self, index: u32) -> Result<Resource> {
        let i = index as usize;
        let raw = raw_chapter(&self.book, i).ok_or(ReaderError::Parse { msg: format!("chapter {index} unreadable") })?;
        let lang = self.book.language.as_deref();
        let (html, is_xml) = sanitize::prepare(&raw, self.direction, lang, &self.book.chapters[i].path);
        let mime = if is_xml { "application/xhtml+xml" } else { "text/html" };
        Ok(Resource { mime: mime.into(), data: html.into_bytes() })
    }

    /// Any non-chapter resource (image, css, font…) by its book path.
    pub fn resource(&self, path: String) -> Option<Resource> {
        let p = util::normalize(&util::percent_decode(&path));
        let data = self.book.store.get(&p)?;
        let mime = self.book.store.mime(&p);
        let data = if mime == "text/css" { sanitize::css(&data) } else { data };
        Some(Resource { mime, data })
    }

    pub fn cover(&self) -> Option<Resource> {
        let c = self.book.cover.clone()?;
        self.resource(c)
    }

    /// Searches all chapters. Diacritics and presentation forms are ignored;
    /// for Uyghur books every Uyghur letter stays distinct and Latin (ULY)
    /// queries are transliterated.
    pub fn search(&self, query: String, max_hits: u32) -> Vec<ReflowHit> {
        let p = self.profile_for_query(&query);
        let needles = rtl::needles(&query, p);
        let mut out = Vec::new();
        if needles.is_empty() {
            return out;
        }
        for ci in 0..self.book.chapters.len() {
            let ft = self.folded_text(ci as u32, p);
            for (occ, (start, len)) in rtl::find_any(&ft.folded, &needles).into_iter().enumerate() {
                // Snippet from the original text, so it keeps hamza and diacritics.
                let (s0, e0) = (ft.map[start], ft.map[start + len - 1]);
                let a = s0.saturating_sub(40);
                let b = (e0 + 41).min(ft.original.len());
                let snippet: String = ft.original[a..b].iter().collect();
                out.push(ReflowHit { chapter: ci as u32, occurrence: occ as u32, snippet: snippet.trim().replace('\n', " ") });
                if out.len() as u32 >= max_hits {
                    return out;
                }
            }
        }
        out
    }

    /// Plain text of a chapter (for text-to-speech / copy).
    pub fn chapter_text(&self, index: u32) -> String {
        raw_chapter(&self.book, index as usize).map(|h| util::html_to_text(&h)).unwrap_or_default()
    }
}

impl ReflowDoc {
    fn profile_for_query(&self, q: &str) -> rtl::FoldProfile {
        if self.profile == rtl::FoldProfile::Uyghur { rtl::FoldProfile::Uyghur } else { rtl::profile_for("", q) }
    }

    fn folded_text(&self, i: u32, p: rtl::FoldProfile) -> Arc<FoldedText> {
        let key = (i, p == rtl::FoldProfile::Uyghur);
        if let Some(t) = self.text_cache.lock().unwrap().get(&key) {
            return t.clone();
        }
        let txt = raw_chapter(&self.book, i as usize).map(|h| util::html_to_text(&h)).unwrap_or_default();
        let (folded, map) = rtl::fold_with_map(&txt, p);
        let original: Vec<char> = txt.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).collect();
        let t = Arc::new(FoldedText { folded, map, original });
        self.text_cache.lock().unwrap().insert(key, t.clone());
        t
    }
}

fn raw_chapter(book: &Book, i: usize) -> Option<String> {
    let ch = book.chapters.get(i)?;
    if let Some(h) = &ch.html {
        return Some(h.clone());
    }
    let bytes = book.store.get(&ch.path)?;
    Some(util::decode_text(&bytes, sanitize::declared_charset(&bytes).as_deref()))
}

/// Finds `query` inside the DOM text nodes the WebView reports, with the same
/// folding as [ReflowDoc::search] (pass `uyghur` from [BookInfo]); returns
/// UTF-16 ranges for JS.
#[uniffi::export]
pub fn locate_in_nodes(nodes: Vec<String>, query: String, uyghur: bool) -> Vec<NodeRange> {
    let p = if uyghur { rtl::FoldProfile::Uyghur } else { rtl::profile_for("", &query) };
    let needles = rtl::needles(&query, p);
    if needles.is_empty() {
        return vec![];
    }
    // Concatenate, remembering (node, utf16 offset) for every source char.
    let mut all = String::new();
    let mut pos: Vec<(u32, u32, u32)> = Vec::new(); // node, utf16 start, utf16 len
    for (ni, n) in nodes.iter().enumerate() {
        let mut off = 0u32;
        for c in n.chars() {
            let l = c.len_utf16() as u32;
            all.push(c);
            pos.push((ni as u32, off, l));
            off += l;
        }
    }
    let (folded, map) = rtl::fold_with_map(&all, p);
    let chars: Vec<char> = all.chars().collect();
    rtl::find_any(&folded, &needles)
        .into_iter()
        .map(|(s, len)| {
            let e = s + len - 1;
            let (sn, so, _) = pos[map[s]];
            // include combining marks (harakat, niqqud) that trail the last letter
            let mut last = map[e];
            while last + 1 < chars.len() && rtl::is_mark_to_strip(chars[last + 1]) && pos[last + 1].0 == pos[last].0 {
                last += 1;
            }
            let (en, eo, el) = pos[last];
            NodeRange { start_node: sn, start_offset: so, end_node: en, end_offset: eo + el }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locates_across_nodes_with_harakat() {
        let nodes = vec!["قال: ".to_string(), "الْكِتَ".to_string(), "ابُ جميل".to_string()];
        let r = locate_in_nodes(nodes, "الكتاب".into(), false);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].start_node, 1);
        assert_eq!(r[0].start_offset, 0);
        assert_eq!(r[0].end_node, 2);
        assert_eq!(r[0].end_offset, 3); // "ابُ" = 3 UTF-16 units incl. damma
    }

    #[test]
    fn locates_uyghur_by_latin_query() {
        let nodes = vec!["مەن ".to_string(), "ئۈرۈمچىدە ".to_string(), "ئوقۇيمەن".to_string()];
        let r = locate_in_nodes(nodes.clone(), "ürümchide".into(), true);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].start_node, 1);
        // tila (تىل) must not match تيل in a Uyghur book
        assert!(locate_in_nodes(vec!["تيل".into()], "تىل".into(), true).is_empty());
    }
}
