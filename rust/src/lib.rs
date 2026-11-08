//! reader_core — the Rust heart of the Sahifa reader.
//!
//! * `pdf`     – PDF via Chromium's PDFium, on one dedicated worker thread.
//! * `reflow`  – EPUB, DOCX, ODT, FB2, HTML, TXT, Markdown and CBZ converted to
//!               sanitized (X)HTML chapters that the Android WebView (Blink +
//!               HarfBuzz + ICU) lays out, giving first-class Arabic/Persian/
//!               Urdu/Hebrew shaping and bidi.
//! * `rtl`     – direction detection and RTL-aware search folding.
//! * `library` – recent files, reading progress, bookmarks and highlights.

uniffi::setup_scaffolding!();

pub mod library;
pub mod pdf;
mod pdf_layout;
pub mod reflow;
pub mod rtl;
mod util;

pub use rtl::TextDirection;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ReaderError {
    #[error("I/O error: {msg}")]
    Io { msg: String },
    #[error("Unsupported format: {msg}")]
    Unsupported { msg: String },
    #[error("Could not parse document: {msg}")]
    Parse { msg: String },
    #[error("PDF engine error: {msg}")]
    Pdf { msg: String },
    #[error("Password required or incorrect")]
    Password,
    #[error("Document is closed")]
    Closed,
}

impl From<std::io::Error> for ReaderError {
    fn from(e: std::io::Error) -> Self {
        ReaderError::Io { msg: e.to_string() }
    }
}

impl From<zip::result::ZipError> for ReaderError {
    fn from(e: zip::result::ZipError) -> Self {
        ReaderError::Parse { msg: format!("zip: {e}") }
    }
}

impl From<quick_xml::Error> for ReaderError {
    fn from(e: quick_xml::Error) -> Self {
        ReaderError::Parse { msg: format!("xml: {e}") }
    }
}

pub type Result<T> = std::result::Result<T, ReaderError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DocFormat {
    Pdf,
    Epub,
    Docx,
    Odt,
    Fb2,
    Html,
    Txt,
    Markdown,
    Cbz,
    Unknown,
}

impl DocFormat {
    pub fn is_reflowable(self) -> bool {
        !matches!(self, DocFormat::Pdf | DocFormat::Unknown)
    }
}

/// Detects the format from magic bytes first, then from the file name.
#[uniffi::export]
pub fn detect_format(path: String) -> DocFormat {
    use std::io::Read;
    let mut head = [0u8; 512];
    let n = std::fs::File::open(&path)
        .and_then(|mut f| f.read(&mut head))
        .unwrap_or(0);
    let head = &head[..n];
    let lower = path.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");

    if head.windows(5).take(1024).any(|w| w == b"%PDF-") {
        return DocFormat::Pdf;
    }
    if head.starts_with(b"PK\x03\x04") {
        let has = |needle: &[u8]| head.windows(needle.len()).any(|w| w == needle);
        if has(b"application/epub+zip") {
            return DocFormat::Epub;
        }
        if has(b"application/vnd.oasis.opendocument.text") {
            return DocFormat::Odt;
        }
        return match ext {
            "epub" => DocFormat::Epub,
            "docx" | "docm" | "dotx" => DocFormat::Docx,
            "odt" | "ott" => DocFormat::Odt,
            "cbz" => DocFormat::Cbz,
            _ => sniff_zip(&path),
        };
    }
    match ext {
        "pdf" => DocFormat::Pdf,
        "epub" => DocFormat::Epub,
        "docx" => DocFormat::Docx,
        "odt" => DocFormat::Odt,
        "fb2" => DocFormat::Fb2,
        "htm" | "html" | "xhtml" | "xht" | "mht" => DocFormat::Html,
        "md" | "markdown" | "mdown" => DocFormat::Markdown,
        "txt" | "text" | "log" | "csv" | "srt" => DocFormat::Txt,
        "cbz" => DocFormat::Cbz,
        _ => {
            let s = String::from_utf8_lossy(head).to_lowercase();
            if s.contains("<fictionbook") {
                DocFormat::Fb2
            } else if s.contains("<html") || s.contains("<!doctype html") {
                DocFormat::Html
            } else if !head.is_empty() && !head.iter().any(|b| *b == 0) {
                DocFormat::Txt
            } else {
                DocFormat::Unknown
            }
        }
    }
}

fn sniff_zip(path: &str) -> DocFormat {
    let Ok(f) = std::fs::File::open(path) else { return DocFormat::Unknown };
    let Ok(z) = zip::ZipArchive::new(std::io::BufReader::new(f)) else { return DocFormat::Unknown };
    let names: Vec<String> = z.file_names().filter_map(|n| n.ok().map(|c| c.into_owned())).collect();
    if names.iter().any(|n| *n == "META-INF/container.xml") {
        DocFormat::Epub
    } else if names.iter().any(|n| *n == "word/document.xml") {
        DocFormat::Docx
    } else if names.iter().any(|n| *n == "content.xml") {
        DocFormat::Odt
    } else if names.iter().all(|n| util::is_image_name(n) || n.ends_with('/')) {
        DocFormat::Cbz
    } else {
        DocFormat::Unknown
    }
}

/// Folds text for RTL-aware matching (exposed for the UI's quick filters).
#[uniffi::export]
pub fn fold_for_search(text: String) -> String {
    rtl::fold(&text)
}

#[uniffi::export]
pub fn core_version() -> String {
    format!("reader_core {} (PDFium chromium/8125)", env!("CARGO_PKG_VERSION"))
}
