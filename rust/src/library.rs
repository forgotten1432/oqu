//! Recent files, reading positions, bookmarks and highlights, persisted as
//! one JSON file (atomic write via temp file + rename).

use crate::{DocFormat, ReaderError, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct LibraryEntry {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub format: String,
    pub size_bytes: u64,
    pub added_ms: i64,
    pub last_opened_ms: i64,
    /// 0..1 reading progress.
    pub progress: f32,
    /// Opaque locator: "page:12:0.35" for PDF, "ch:3:0.42" for reflow.
    pub position: Option<String>,
    pub page_count: i32,
    pub starred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AnnotationKind {
    Bookmark,
    Highlight,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct Annotation {
    pub id: String,
    pub doc_id: String,
    pub kind: AnnotationKind,
    pub position: String,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: u32,
    pub created_ms: i64,
}

#[derive(Default, Serialize, Deserialize)]
struct Data {
    entries: Vec<LibraryEntry>,
    annotations: Vec<Annotation>,
    #[serde(default)]
    settings: std::collections::BTreeMap<String, String>,
}

#[derive(uniffi::Object)]
pub struct DocLibrary {
    path: String,
    data: Mutex<Data>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Stable id for a file path (FNV-1a, hex).
pub fn doc_id_for(path: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in path.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn format_name(f: DocFormat) -> &'static str {
    match f {
        DocFormat::Pdf => "PDF",
        DocFormat::Epub => "EPUB",
        DocFormat::Docx => "DOCX",
        DocFormat::Odt => "ODT",
        DocFormat::Fb2 => "FB2",
        DocFormat::Html => "HTML",
        DocFormat::Txt => "TXT",
        DocFormat::Markdown => "MD",
        DocFormat::Cbz => "CBZ",
        DocFormat::Unknown => "?",
    }
}

#[uniffi::export]
impl DocLibrary {
    #[uniffi::constructor]
    pub fn open(path: String) -> Arc<Self> {
        let data = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Data>(&b).ok()).unwrap_or_default();
        Arc::new(DocLibrary { path, data: Mutex::new(data) })
    }

    /// Adds or refreshes a file and marks it as just opened.
    pub fn touch(&self, path: String, display_name: String) -> LibraryEntry {
        let id = doc_id_for(&path);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let fmt = format_name(crate::detect_format(path.clone())).to_string();
        let mut d = self.data.lock().unwrap();
        let now = now_ms();
        let e = if let Some(e) = d.entries.iter_mut().find(|e| e.id == id) {
            e.last_opened_ms = now;
            e.size_bytes = size;
            e.clone()
        } else {
            let e = LibraryEntry {
                id: id.clone(),
                path,
                display_name,
                title: None,
                author: None,
                format: fmt,
                size_bytes: size,
                added_ms: now,
                last_opened_ms: now,
                progress: 0.0,
                position: None,
                page_count: 0,
                starred: false,
            };
            d.entries.push(e.clone());
            e
        };
        drop(d);
        let _ = self.save();
        e
    }

    pub fn set_meta(&self, id: String, title: Option<String>, author: Option<String>, page_count: i32) {
        self.update(&id, |e| {
            if title.as_deref().map(|t| !t.trim().is_empty()).unwrap_or(false) {
                e.title = title;
            }
            if author.is_some() {
                e.author = author;
            }
            if page_count > 0 {
                e.page_count = page_count;
            }
        });
    }

    pub fn set_progress(&self, id: String, position: String, progress: f32) {
        self.update(&id, |e| {
            e.position = Some(position);
            e.progress = progress.clamp(0.0, 1.0);
        });
    }

    pub fn set_starred(&self, id: String, starred: bool) {
        self.update(&id, |e| e.starred = starred);
    }

    pub fn remove(&self, id: String) {
        let mut d = self.data.lock().unwrap();
        d.entries.retain(|e| e.id != id);
        d.annotations.retain(|a| a.doc_id != id);
        drop(d);
        let _ = self.save();
    }

    pub fn get(&self, id: String) -> Option<LibraryEntry> {
        self.data.lock().unwrap().entries.iter().find(|e| e.id == id).cloned()
    }

    /// Most recently opened first. `filter` is matched RTL-aware (folded).
    pub fn recent(&self, filter: Option<String>) -> Vec<LibraryEntry> {
        let d = self.data.lock().unwrap();
        let f = filter.map(|f| crate::rtl::fold(&f)).filter(|f| !f.trim().is_empty());
        let mut v: Vec<LibraryEntry> = d
            .entries
            .iter()
            .filter(|e| match &f {
                None => true,
                Some(f) => {
                    let hay = crate::rtl::fold(&format!("{} {} {}", e.display_name, e.title.clone().unwrap_or_default(), e.author.clone().unwrap_or_default()));
                    hay.contains(f.trim())
                }
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| b.last_opened_ms.cmp(&a.last_opened_ms));
        v
    }

    pub fn add_annotation(&self, doc_id: String, kind: AnnotationKind, position: String, text: Option<String>, note: Option<String>, color: u32) -> Annotation {
        let a = Annotation {
            id: format!("{:x}", now_ms() as u64 ^ (doc_id_for(&position).len() as u64) << 48 ^ rand_bits()),
            doc_id,
            kind,
            position,
            text,
            note,
            color,
            created_ms: now_ms(),
        };
        self.data.lock().unwrap().annotations.push(a.clone());
        let _ = self.save();
        a
    }

    pub fn remove_annotation(&self, id: String) {
        self.data.lock().unwrap().annotations.retain(|a| a.id != id);
        let _ = self.save();
    }

    pub fn annotations(&self, doc_id: String) -> Vec<Annotation> {
        let mut v: Vec<Annotation> = self.data.lock().unwrap().annotations.iter().filter(|a| a.doc_id == doc_id).cloned().collect();
        v.sort_by(|a, b| a.created_ms.cmp(&b.created_ms));
        v
    }

    pub fn setting(&self, key: String) -> Option<String> {
        self.data.lock().unwrap().settings.get(&key).cloned()
    }

    pub fn set_setting(&self, key: String, value: String) {
        self.data.lock().unwrap().settings.insert(key, value);
        let _ = self.save();
    }
}

impl DocLibrary {
    fn update(&self, id: &str, f: impl FnOnce(&mut LibraryEntry)) {
        let mut d = self.data.lock().unwrap();
        if let Some(e) = d.entries.iter_mut().find(|e| e.id == id) {
            f(e);
        }
        drop(d);
        let _ = self.save();
    }

    fn save(&self) -> Result<()> {
        let bytes = {
            let d = self.data.lock().unwrap();
            serde_json::to_vec(&*d).map_err(|e| ReaderError::Io { msg: e.to_string() })?
        };
        let tmp = format!("{}.tmp", self.path);
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn rand_bits() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_i64(now_ms());
    h.finish() & 0xFFFF_FFFF
}

#[uniffi::export]
pub fn doc_id(path: String) -> String {
    doc_id_for(&path)
}
