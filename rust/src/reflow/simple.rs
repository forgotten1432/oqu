//! HTML, plain text, Markdown and CBZ.

use super::{Book, Chapter, Store, TocItem};
use crate::util::{self, escape, escape_attr};
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

fn stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn open_html(path: &str) -> Result<Book> {
    let bytes = std::fs::read(path)?;
    let html = util::decode_text(&bytes, super::sanitize::declared_charset(&bytes).as_deref());
    let title = regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .ok()
        .and_then(|r| r.captures(&html).map(|c| html_escape::decode_html_entities(c[1].trim()).into_owned()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| stem(path));
    let lang = regex::Regex::new(r#"(?is)<html[^>]*\blang\s*=\s*["']([^"']+)["']"#)
        .ok()
        .and_then(|r| r.captures(&html).map(|c| c[1].to_string()));
    let dir = Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let name = Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "index.html".into());
    let toc = headings_toc(&html);
    Ok(Book {
        title: title.clone(),
        author: None,
        language: lang,
        direction: None,
        chapters: vec![Chapter { path: name, title: Some(title), html: Some(add_heading_ids(&html)) }],
        toc,
        store: Store::Dir(dir),
        cover: None,
        fixed_layout: false,
    })
}

/// Splits large text files into ~150 KB chapters so the WebView stays fast.
pub fn open_txt(path: &str) -> Result<Book> {
    let bytes = std::fs::read(path)?;
    let text = util::decode_text(&bytes, None).replace("\r\n", "\n").replace('\r', "\n");
    let blank_paragraphs = text.matches("\n\n").count();
    let lines = text.lines().count().max(1);
    // If the file has few blank lines, treat every line as a paragraph.
    let paras: Vec<&str> = if blank_paragraphs * 4 >= lines / 4 {
        text.split("\n\n").collect()
    } else {
        text.lines().collect()
    };
    let title = stem(path);
    let mut chapters = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, chapters: &mut Vec<Chapter>| {
        if buf.is_empty() {
            return;
        }
        let i = chapters.len();
        chapters.push(Chapter {
            path: format!("text/{i}.html"),
            title: Some(format!("{}", i + 1)),
            html: Some(format!("<html><head><meta charset=\"utf-8\"/></head><body class=\"txt\">{buf}</body></html>")),
        });
        buf.clear();
    };
    for p in paras {
        let t = p.trim();
        if t.is_empty() {
            continue;
        }
        buf.push_str("<p>");
        buf.push_str(&escape(t).replace('\n', "<br/>"));
        buf.push_str("</p>\n");
        if buf.len() > 150_000 {
            flush(&mut buf, &mut chapters);
        }
    }
    flush(&mut buf, &mut chapters);
    let toc = if chapters.len() > 1 {
        chapters.iter().enumerate().map(|(i, _)| TocItem { title: format!("{} / {}", i + 1, chapters.len()), chapter: i as u32, anchor: None, level: 0 }).collect()
    } else {
        vec![]
    };
    let dir = Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    Ok(Book { title, author: None, language: None, direction: None, chapters, toc, store: Store::Dir(dir), cover: None, fixed_layout: false })
}

pub fn open_markdown(path: &str) -> Result<Book> {
    let bytes = std::fs::read(path)?;
    let md = util::decode_text(&bytes, None);
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    opts.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    let parser = pulldown_cmark::Parser::new_ext(&md, opts);
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, parser);
    let title = md
        .lines()
        .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
        .unwrap_or_else(|| stem(path));
    let html = add_heading_ids(&format!("<html><head><meta charset=\"utf-8\"/><title>{}</title></head><body class=\"md\">{}</body></html>", escape(&title), body));
    let toc = headings_toc(&html);
    let dir = Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    Ok(Book {
        title: title.clone(),
        author: None,
        language: None,
        direction: None,
        chapters: vec![Chapter { path: "document.html".into(), title: Some(title), html: Some(html) }],
        toc,
        store: Store::Dir(dir),
        cover: None,
        fixed_layout: false,
    })
}

pub fn open_cbz(path: &str) -> Result<Book> {
    let store = Store::open_zip(path)?;
    let Store::Zip(z) = &store else { unreachable!() };
    let mut names: Vec<String> = z.lock().unwrap().file_names().filter_map(|n| n.ok().map(|c| c.into_owned())).filter(|n| util::is_image_name(n)).collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    let chapters = names
        .iter()
        .enumerate()
        .map(|(i, n)| Chapter {
            path: format!("page/{i}.html"),
            title: Some(format!("{}", i + 1)),
            html: Some(format!(
                "<html><head><meta charset=\"utf-8\"/><style>html,body{{margin:0;padding:0;background:#000;height:100%}}img{{display:block;margin:auto;max-width:100%;max-height:100vh;object-fit:contain}}</style></head><body class=\"cbz\"><img src=\"../{}\" alt=\"\"/></body></html>",
                escape_attr(n)
            )),
        })
        .collect();
    let cover = names.first().cloned();
    Ok(Book { title: stem(path), author: None, language: None, direction: None, chapters, toc: vec![], store, cover, fixed_layout: true })
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let mut parts: Vec<(String, u64)> = Vec::new();
        let mut num = String::new();
        let mut txt = String::new();
        for c in s.to_lowercase().chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                if !num.is_empty() {
                    parts.push((std::mem::take(&mut txt), num.parse().unwrap_or(0)));
                    num.clear();
                }
                txt.push(c);
            }
        }
        parts.push((txt, num.parse().unwrap_or(0)));
        parts
    };
    key(a).cmp(&key(b))
}

/// Adds `id="hN"` to headings lacking an id so the TOC can target them.
fn add_heading_ids(html: &str) -> String {
    let re = regex::Regex::new(r"(?i)<h([1-6])(\s[^>]*)?>").unwrap();
    let mut n = 0;
    re.replace_all(html, |c: &regex::Captures| {
        let attrs = c.get(2).map(|m| m.as_str()).unwrap_or("");
        if attrs.contains("id=") {
            c[0].to_string()
        } else {
            n += 1;
            format!("<h{} id=\"hd{}\"{}>", &c[1], n, attrs)
        }
    })
    .into_owned()
}

fn headings_toc(html: &str) -> Vec<TocItem> {
    let html = add_heading_ids(html);
    let re = regex::Regex::new(r#"(?is)<h([1-6])[^>]*\bid\s*=\s*["']([^"']+)["'][^>]*>(.*?)</h[1-6]>"#).unwrap();
    let mut seen: HashMap<String, ()> = HashMap::new();
    re.captures_iter(&html)
        .filter_map(|c| {
            let title = util::html_to_text(&format!("<body>{}</body>", &c[3])).split_whitespace().collect::<Vec<_>>().join(" ");
            if title.is_empty() || seen.insert(c[2].to_string(), ()).is_some() {
                return None;
            }
            Some(TocItem { title, chapter: 0, anchor: Some(c[2].to_string()), level: c[1].parse::<u32>().unwrap_or(1) - 1 })
        })
        .collect()
}
