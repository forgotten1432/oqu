//! Splits oversized chapters into parts of ~[TARGET] bytes.
//!
//! Many EPUBs (and every converted DOCX/ODT/FB2/HTML) put a whole book into
//! one HTML file. The WebView has to parse and lay out such a file in one go
//! before anything shows; with a few MB of text that takes tens of seconds on
//! a phone or gets the renderer killed. Parts are cut only between block
//! elements; elements still open at a cut (a wrapper `<div>`, a `<section>`)
//! are closed at the end of one part and reopened (without `id`) at the start
//! of the next, so every part stays well-formed.
//!
//! Parts are served at virtual paths next to the original file
//! (`Text/book.xhtml` → `Text/book.part1.xhtml`, `Text/book.part2.xhtml`, …),
//! so relative URLs keep resolving. Links to the original file and in-page
//! `#id` links are routed to the part holding that id.

use super::{Book, Chapter, Store};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Target part size in bytes of HTML (same size the TXT reader uses).
pub const TARGET: usize = 150_000;
/// Chapters smaller than this are left alone.
const MIN_SPLIT: usize = TARGET * 3 / 2;

/// Original chapter path → (chapter index, ids defined in that part) per part.
pub(crate) type PartMap = HashMap<String, Vec<(u32, HashSet<String>)>>;

const BLOCKS: &[&str] = &[
    "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "section", "article", "table", "ul", "ol", "dl", "pre",
    "figure", "aside", "header", "footer", "nav", "center", "main", "address", "hr", "br",
];
/// Elements a cut may pass through (closed, then reopened in the next part).
const CONTAINERS: &[&str] = &["div", "section", "article", "main", "aside", "header", "footer", "center", "blockquote", "font"];
/// Also allowed when a part has grown far past the target with no clean cut.
const SOFT: &[&str] = &["p", "pre", "ul", "ol", "li", "dl", "dd", "dt", "figure", "span", "b", "i", "em", "strong"];
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr",
];
const RAW_TEXT: &[&str] = &["script", "style", "textarea", "title", "xmp"];

pub(crate) struct Part {
    pub html: String,
    pub ids: HashSet<String>,
}

/// Splits every large chapter of `book` in place and remaps the TOC.
/// `raw` returns a chapter's undecorated HTML.
pub(crate) fn split_book(book: &mut Book, raw: impl Fn(&Store, &Chapter) -> Option<String>) -> PartMap {
    let mut map = PartMap::new();
    if book.fixed_layout {
        return map;
    }
    let old = std::mem::take(&mut book.chapters);
    let mut chapters = Vec::with_capacity(old.len());
    // per old chapter: (new index of its first part, original path if split)
    let mut moved: Vec<(u32, Option<String>)> = Vec::with_capacity(old.len());
    for ch in old {
        let big = match &ch.html {
            Some(h) => h.len() >= MIN_SPLIT,
            None => book.store.size(&ch.path).map(|s| s >= MIN_SPLIT as u64).unwrap_or(true),
        };
        let parts = if big { raw(&book.store, &ch).and_then(|h| split_html(&h, &ch.path)) } else { None };
        match parts {
            Some(parts) if parts.len() > 1 => {
                moved.push((chapters.len() as u32, Some(ch.path.clone())));
                let mut entry = Vec::with_capacity(parts.len());
                for (k, p) in parts.into_iter().enumerate() {
                    entry.push((chapters.len() as u32, p.ids));
                    chapters.push(Chapter {
                        path: part_path(&ch.path, k + 1),
                        title: if k == 0 { ch.title.clone() } else { None },
                        html: Some(p.html),
                    });
                }
                map.insert(ch.path, entry);
            }
            _ => {
                moved.push((chapters.len() as u32, None));
                chapters.push(ch);
            }
        }
    }
    book.chapters = chapters;
    for t in &mut book.toc {
        let Some((first, orig)) = moved.get(t.chapter as usize) else { continue };
        t.chapter = *first;
        if let (Some(orig), Some(a)) = (orig, &t.anchor) {
            if let Some(idx) = find_part(&map, orig, a) {
                t.chapter = idx;
            }
        }
    }
    map
}

/// Chapter index of the part of `orig` that defines `id`.
pub(crate) fn find_part(map: &PartMap, orig: &str, id: &str) -> Option<u32> {
    map.get(orig)?.iter().find(|(_, ids)| ids.contains(id)).map(|(i, _)| *i)
}

/// `OEBPS/Text/ch.xhtml`, 2 → `OEBPS/Text/ch.part2.xhtml`
pub(crate) fn part_path(path: &str, n: usize) -> String {
    let (dir, file) = match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    };
    match file.rfind('.') {
        Some(d) if d > 0 => format!("{dir}{}.part{n}{}", &file[..d], &file[d..]),
        _ => format!("{dir}{file}.part{n}"),
    }
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid regex"))
}
static BODY_OPEN: OnceLock<Regex> = OnceLock::new();
static BODY_CLOSE: OnceLock<Regex> = OnceLock::new();
static ID_ATTR: OnceLock<Regex> = OnceLock::new();
static IDS: OnceLock<Regex> = OnceLock::new();
static SELF_HREF: OnceLock<Regex> = OnceLock::new();

struct Open {
    name: String,
    tag: String,
}

/// Splits one HTML/XHTML document; `None` if it has no body or no safe cut.
pub(crate) fn split_html(html: &str, path: &str) -> Option<Vec<Part>> {
    let open = re(&BODY_OPEN, r"(?is)<body\b[^>]*>").find(html)?;
    let close = re(&BODY_CLOSE, r"(?is)</body\s*>").find_iter(html).last().map(|m| m.start()).unwrap_or(html.len());
    if close < open.end() {
        return None;
    }
    let prefix = &html[..open.end()];
    let suffix = if close < html.len() { &html[close..] } else { "</body></html>" };
    let body = &html[open.end()..close];
    let cuts = cut_points(body);
    if cuts.is_empty() {
        return None;
    }
    // In-page links (#id) point at the original file, which routes them to the
    // part that has the id.
    let file = path.rsplit('/').next().unwrap_or(path);
    let self_href = format!("${{1}}{}#", encode_segment(file));
    let mut parts = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0usize;
    let mut reopen = String::new();
    for (end, stack) in cuts.iter().map(|(e, s)| (*e, Some(s))).chain(std::iter::once((body.len(), None))) {
        let seg = &body[start..end];
        let mut h = String::with_capacity(prefix.len() + reopen.len() + seg.len() + suffix.len() + 64);
        h.push_str(prefix);
        h.push_str(&reopen);
        h.push_str(seg);
        if let Some(stack) = stack {
            for o in stack.iter().rev() {
                h.push_str("</");
                h.push_str(&o.name);
                h.push('>');
            }
            reopen = stack.iter().map(|o| re(&ID_ATTR, r#"(?i)\s+id\s*=\s*("[^"]*"|'[^']*'|[^\s>/]+)"#).replace_all(&o.tag, "").into_owned()).collect();
        }
        h.push_str(suffix);
        let h = re(&SELF_HREF, r#"(?i)(\bhref\s*=\s*["'])#"#).replace_all(&h, self_href.as_str()).into_owned();
        parts.push(Part { ids: ids_in(seg), html: h });
        start = end;
    }
    Some(parts)
}

fn ids_in(s: &str) -> HashSet<String> {
    re(&IDS, r#"(?i)\s(?:id|name)\s*=\s*["']([^"']+)["']"#)
        .captures_iter(s)
        .map(|c| crate::util::unescape_xml(&c[1]))
        .collect()
}

fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&()*+,;=:@".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn lower_name(s: &[u8]) -> String {
    let n: Vec<u8> = s.iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b':' || **c == b'-' || **c == b'_').copied().collect();
    let n = String::from_utf8_lossy(&n).to_ascii_lowercase();
    match n.rfind(':') {
        Some(i) => n[i + 1..].to_string(),
        None => n,
    }
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    if from >= b.len() {
        return None;
    }
    b[from..].windows(pat.len()).position(|w| w == pat).map(|p| p + from)
}

fn find_ci(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    if from >= b.len() {
        return None;
    }
    b[from..].windows(pat.len()).position(|w| w.eq_ignore_ascii_case(pat)).map(|p| p + from)
}

/// End of a start tag beginning at `i` (index after '>'), honouring quotes.
fn tag_end(b: &[u8], mut i: usize) -> usize {
    let mut q = 0u8;
    while i < b.len() {
        let c = b[i];
        if q != 0 {
            if c == q {
                q = 0;
            }
        } else if c == b'"' || c == b'\'' {
            q = c;
        } else if c == b'>' {
            return i + 1;
        }
        i += 1;
    }
    b.len()
}

/// Byte offsets in `body` after which a part may end, with the elements open there.
fn cut_points(body: &str) -> Vec<(usize, Vec<Open>)> {
    let b = body.as_bytes();
    let mut cuts: Vec<(usize, Vec<Open>)> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut last_cut = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        let Some(lt) = b[i..].iter().position(|&c| c == b'<').map(|p| p + i) else { break };
        i = lt;
        let rest = &b[i..];
        let mut closed: Option<String> = None;
        if rest.starts_with(b"<!--") {
            i = find(b, i + 4, b"-->").map(|p| p + 3).unwrap_or(b.len());
            continue;
        } else if rest.starts_with(b"<![CDATA[") {
            i = find(b, i + 9, b"]]>").map(|p| p + 3).unwrap_or(b.len());
            continue;
        } else if rest.starts_with(b"<!") || rest.starts_with(b"<?") {
            i = find(b, i, b">").map(|p| p + 1).unwrap_or(b.len());
            continue;
        } else if rest.starts_with(b"</") {
            let name = lower_name(&rest[2..]);
            i = find(b, i, b">").map(|p| p + 1).unwrap_or(b.len());
            if let Some(pos) = stack.iter().rposition(|o| o.name == name) {
                stack.truncate(pos);
            }
            closed = Some(name);
        } else if rest.len() > 1 && rest[1].is_ascii_alphabetic() {
            let name = lower_name(&rest[1..]);
            let end = tag_end(b, i + 1);
            let self_closing = end >= 2 && b[end - 2] == b'/';
            let tag = &body[i..end];
            i = end;
            if RAW_TEXT.contains(&name.as_str()) && !self_closing {
                let close = format!("</{name}");
                i = find_ci(b, i, close.as_bytes()).map(|p| tag_end(b, p)).unwrap_or(b.len());
            } else if VOID.contains(&name.as_str()) || self_closing {
                if name == "hr" || name == "br" {
                    closed = Some(name);
                }
            } else {
                stack.push(Open { name, tag: tag.to_string() });
            }
        } else {
            i += 1;
            continue;
        }
        let Some(name) = closed else { continue };
        let size = i - last_cut;
        if size < TARGET || b.len() - i < TARGET / 3 {
            continue;
        }
        let in_containers = stack.iter().all(|o| CONTAINERS.contains(&o.name.as_str()));
        let clean = BLOCKS.contains(&name.as_str()) && name != "br" && in_containers;
        let forced = size >= TARGET * 2
            && stack.iter().all(|o| CONTAINERS.contains(&o.name.as_str()) || SOFT.contains(&o.name.as_str()));
        if clean || forced {
            cuts.push((i, stack.iter().map(|o| Open { name: o.name.clone(), tag: o.tag.clone() }).collect()));
            last_cut = i;
        }
    }
    cuts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(body: &str) -> String {
        format!(r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>t</title></head><body class="b">{body}</body></html>"#)
    }

    fn paras(n: usize, from: usize) -> String {
        (from..from + n).map(|i| format!("<p id=\"p{i}\">paragraph {i} مەن ئۈرۈمچىدە ئوقۇيمەن lorem ipsum dolor sit amet.</p>\n")).collect()
    }

    fn well_formed(s: &str) -> bool {
        let mut r = quick_xml::Reader::from_str(s);
        let mut d = 0i64;
        loop {
            match r.read_event() {
                Ok(quick_xml::events::Event::Eof) => return d == 0,
                Ok(quick_xml::events::Event::Start(_)) => d += 1,
                Ok(quick_xml::events::Event::End(_)) => d -= 1,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
    }

    #[test]
    fn small_documents_are_not_split() {
        assert!(split_html(&doc(&paras(100, 0)), "a.xhtml").is_none());
    }

    #[test]
    fn splits_long_flat_body_into_well_formed_parts() {
        let h = doc(&paras(20_000, 0));
        let parts = split_html(&h, "Text/a.xhtml").unwrap();
        assert!(parts.len() > 10, "{}", parts.len());
        let text: usize = parts.iter().map(|p| p.html.matches("<p id=").count()).sum();
        assert_eq!(text, 20_000);
        for p in &parts {
            assert!(p.html.len() < TARGET * 2, "part too big: {}", p.html.len());
            assert!(well_formed(&p.html), "not well-formed");
            assert!(p.html.contains(r#"<body class="b">"#));
            assert!(p.html.contains("<title>t</title>"));
        }
        assert!(parts[3].ids.contains(&format!("p{}", parts[0].ids.len() * 3 + 1)) || !parts[3].ids.is_empty());
    }

    #[test]
    fn reopens_wrapper_without_its_id() {
        let h = doc(&format!(r#"<div id="w" class="main"><section>{}</section></div>"#, paras(20_000, 0)));
        let parts = split_html(&h, "a.xhtml").unwrap();
        assert!(parts.len() > 10);
        assert!(parts[1].html.contains(r#"<body class="b"><div class="main"><section>"#));
        assert!(parts[0].html.contains("</section></div></body>"));
        assert!(parts.iter().all(|p| well_formed(&p.html)));
        assert_eq!(parts.iter().filter(|p| p.ids.contains("w")).count(), 1);
    }

    #[test]
    fn never_cuts_inside_a_paragraph_or_table_when_avoidable() {
        let table = format!("<table>{}</table>", (0..200).map(|i| format!("<tr><td>{i} {}</td></tr>", "x".repeat(800))).collect::<String>());
        let h = doc(&format!("{}{}{}", paras(3000, 0), table, paras(3000, 3000)));
        let parts = split_html(&h, "a.xhtml").unwrap();
        assert_eq!(parts.iter().filter(|p| p.html.contains("<table>")).count(), 1);
        assert!(parts.iter().all(|p| well_formed(&p.html)));
    }

    #[test]
    fn forced_cut_for_one_giant_paragraph_with_line_breaks() {
        let lines: String = (0..40_000).map(|i| format!("line {i} lorem ipsum dolor sit amet<br/>")).collect();
        let h = doc(&format!("<div><p>{lines}</p></div>"));
        let parts = split_html(&h, "a.xhtml").unwrap();
        assert!(parts.len() > 3);
        assert!(parts[1].html.contains(r#"<body class="b"><div><p>"#));
        assert!(parts.iter().all(|p| well_formed(&p.html)));
    }

    #[test]
    fn self_links_point_at_original_file() {
        let h = doc(&format!(r##"<p><a href="#p19999">end</a></p>{}"##, paras(20_000, 0)));
        let parts = split_html(&h, "Text/my book.xhtml").unwrap();
        assert!(parts[0].html.contains(r##"href="my%20book.xhtml#p19999""##));
        assert!(parts.last().unwrap().ids.contains("p19999"));
    }

    #[test]
    fn skips_comments_and_scripts() {
        let h = doc(&format!("<!-- <div> --><script>var a='<div>';</script>{}", paras(20_000, 0)));
        let parts = split_html(&h, "a.xhtml").unwrap();
        assert!(parts.len() > 1);
        // the commented/scripted <div> must not be treated as an open element
        assert!(parts[1].html.contains("<body class=\"b\">\n<p id="), "{}", &parts[1].html[..300]);
        assert!(!parts[0].html.contains("</div></body>"));
    }

    #[test]
    fn part_paths() {
        assert_eq!(part_path("OEBPS/Text/c.xhtml", 2), "OEBPS/Text/c.part2.xhtml");
        assert_eq!(part_path("document.html", 1), "document.part1.html");
        assert_eq!(part_path("noext", 3), "noext.part3");
    }
}
