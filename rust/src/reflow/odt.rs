//! ODT (OpenDocument Text) → HTML, with `style:writing-mode="rl-tb"` RTL
//! paragraphs, headings, lists, tables, links, images and basic emphasis.

use super::{Book, Chapter, Store, TocItem};
use crate::rtl::TextDirection;
use crate::util::{self, attr, escape, escape_attr, local};
use crate::Result;
use quick_xml::events::Event;
use std::collections::HashMap;

#[derive(Default, Clone)]
struct OStyle {
    parent: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    rtl: Option<bool>,
    align: Option<String>,
}

fn parse_styles(xml: &str, out: &mut HashMap<String, OStyle>) {
    let mut r = quick_xml::Reader::from_str(xml);
    r.config_mut().check_end_names = false;
    let mut cur: Option<(String, OStyle)> = None;
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Start(e) | Event::Empty(e) => {
                let n = local(e.name().into_inner());
                match n.as_str() {
                    "style" => {
                        if let Some((k, v)) = cur.take() {
                            out.insert(k, v);
                        }
                        if let Some(name) = attr(&e, "name") {
                            cur = Some((name, OStyle { parent: attr(&e, "parent-style-name"), ..Default::default() }));
                        }
                    }
                    "text-properties" => {
                        if let Some((_, s)) = cur.as_mut() {
                            if let Some(w) = attr(&e, "font-weight") {
                                s.bold = Some(w == "bold" || w.parse::<u32>().map(|v| v >= 600).unwrap_or(false));
                            }
                            if let Some(st) = attr(&e, "font-style") {
                                s.italic = Some(st == "italic" || st == "oblique");
                            }
                            if let Some(u) = attr(&e, "text-underline-style") {
                                s.underline = Some(u != "none");
                            }
                            if let Some(u) = attr(&e, "text-line-through-style") {
                                s.strike = Some(u != "none");
                            }
                        }
                    }
                    "paragraph-properties" => {
                        if let Some((_, s)) = cur.as_mut() {
                            if let Some(w) = attr(&e, "writing-mode") {
                                s.rtl = Some(w.starts_with("rl"));
                            }
                            if let Some(a) = attr(&e, "text-align") {
                                s.align = Some(a);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                if local(e.name().into_inner()) == "style" {
                    if let Some((k, v)) = cur.take() {
                        out.insert(k, v);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
}

fn resolve(styles: &HashMap<String, OStyle>, name: Option<&str>) -> OStyle {
    let mut out = OStyle::default();
    let mut cur = name.map(|s| s.to_string());
    let mut guard = 0;
    while let (Some(c), true) = (cur.clone(), guard < 16) {
        guard += 1;
        let Some(s) = styles.get(&c) else { break };
        out.bold = out.bold.or(s.bold);
        out.italic = out.italic.or(s.italic);
        out.underline = out.underline.or(s.underline);
        out.strike = out.strike.or(s.strike);
        out.rtl = out.rtl.or(s.rtl);
        out.align = out.align.clone().or(s.align.clone());
        cur = s.parent.clone();
    }
    out
}

pub fn open(path: &str) -> Result<Book> {
    let store = Store::open_zip(path)?;
    let Store::Zip(zm) = &store else { unreachable!() };
    let mut z = zm.lock().unwrap();
    let content = util::zip_entry_string(&mut z, "content.xml")?;
    let styles_xml = util::zip_entry_string(&mut z, "styles.xml").unwrap_or_default();
    let meta = util::zip_entry_string(&mut z, "meta.xml").unwrap_or_default();
    drop(z);

    let mut styles = HashMap::new();
    parse_styles(&styles_xml, &mut styles);
    parse_styles(&content, &mut styles);

    let mut out = String::new();
    let mut toc = Vec::new();
    let mut stack: Vec<String> = Vec::new(); // closing tags
    let mut in_body = false;
    let mut heading_n = 0;
    let mut heading_buf: Option<(u32, String, usize)> = None;
    let (mut rtl_n, mut ltr_n) = (0u32, 0u32);
    let mut skip = 0u32;

    let mut r = quick_xml::Reader::from_str(&content);
    r.config_mut().check_end_names = false;
    loop {
        let ev = match r.read_event() {
            Ok(e) => e,
            Err(_) => break,
        };
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                let n = local(e.name().into_inner());
                if n == "text" && !in_body {
                    in_body = true;
                    continue;
                }
                if !in_body {
                    continue;
                }
                if skip > 0 {
                    if !empty {
                        skip += 1;
                    }
                    continue;
                }
                let mut close = String::new();
                match n.as_str() {
                    "p" | "h" => {
                        let st = resolve(&styles, attr(e, "style-name").as_deref());
                        let mut css = String::new();
                        match st.align.as_deref() {
                            Some("center") => css.push_str("text-align:center;"),
                            Some("justify") => css.push_str("text-align:justify;"),
                            _ => {}
                        }
                        let dir = if st.rtl == Some(true) {
                            rtl_n += 1;
                            " dir=\"rtl\""
                        } else {
                            ltr_n += 1;
                            ""
                        };
                        let emph = emphasis_open(&st);
                        if n == "h" {
                            let lvl: u32 = attr(e, "outline-level").and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 6);
                            heading_n += 1;
                            let id = format!("h{heading_n}");
                            out.push_str(&format!("<h{lvl} id=\"{id}\"{dir} style=\"{css}\">{}", emph.0));
                            heading_buf = Some((lvl, id, out.len()));
                            close = format!("{}</h{lvl}>", emph.1);
                        } else {
                            out.push_str(&format!("<p{dir} style=\"{css}\">{}", emph.0));
                            close = format!("{}</p>", emph.1);
                        }
                        if empty {
                            out.push_str(&close);
                            if n == "h" {
                                heading_buf = None;
                            }
                            continue;
                        }
                    }
                    "span" => {
                        let st = resolve(&styles, attr(e, "style-name").as_deref());
                        let (o, c) = emphasis_open(&st);
                        out.push_str(&o);
                        close = c;
                    }
                    "a" => {
                        let href = attr(e, "href").unwrap_or_default();
                        if href.trim_start().to_lowercase().starts_with("javascript:") {
                            out.push_str("<a>");
                        } else {
                            out.push_str(&format!("<a href=\"{}\">", escape_attr(&href)));
                        }
                        close = "</a>".into();
                    }
                    "list" => {
                        out.push_str("<ul>");
                        close = "</ul>".into();
                    }
                    "list-item" | "list-header" => {
                        out.push_str("<li>");
                        close = "</li>".into();
                    }
                    "table" => {
                        out.push_str("<table class=\"docx\">");
                        close = "</table>".into();
                    }
                    "table-row" => {
                        out.push_str("<tr>");
                        close = "</tr>".into();
                    }
                    "table-cell" => {
                        let span = attr(e, "number-columns-spanned").map(|v| format!(" colspan=\"{}\"", escape_attr(&v))).unwrap_or_default();
                        out.push_str(&format!("<td{span}>"));
                        close = "</td>".into();
                    }
                    "line-break" => out.push_str("<br/>"),
                    "tab" => out.push_str("&#8195;"),
                    "s" => {
                        let c: usize = attr(e, "c").and_then(|v| v.parse().ok()).unwrap_or(1);
                        out.push_str(&"&#160;".repeat(c.min(64)));
                    }
                    "image" => {
                        if let Some(h) = attr(e, "href") {
                            if !h.contains("://") {
                                out.push_str(&format!("<img src=\"{}\" alt=\"\"/>", escape_attr(&util::normalize(&h))));
                            }
                        }
                    }
                    "bookmark" | "bookmark-start" => {
                        if let Some(nm) = attr(e, "name") {
                            out.push_str(&format!("<a id=\"{}\"></a>", escape_attr(&nm)));
                        }
                    }
                    "note-citation" => {
                        out.push_str("<sup>");
                        close = "</sup>".into();
                    }
                    "note-body" => {
                        out.push_str("<span class=\"note\"> (");
                        close = ")</span>".into();
                    }
                    "tracked-changes" | "sequence-decls" | "variable-decls" | "user-field-decls" | "table-columns" | "soft-page-break" | "annotation" => {
                        if !empty {
                            skip = 1;
                        }
                        continue;
                    }
                    _ => {}
                }
                if !empty {
                    stack.push(close);
                }
            }
            Event::End(e) => {
                if !in_body {
                    continue;
                }
                let n = local(e.name().into_inner());
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                if n == "text" {
                    in_body = false;
                    continue;
                }
                if let Some(c) = stack.pop() {
                    if n == "h" {
                        if let Some((lvl, id, start)) = heading_buf.take() {
                            let txt = util::html_to_text(&format!("<body>{}</body>", &out[start..]));
                            let txt = txt.split_whitespace().collect::<Vec<_>>().join(" ");
                            if !txt.is_empty() {
                                toc.push(TocItem { title: txt, chapter: 0, anchor: Some(id), level: lvl - 1 });
                            }
                        }
                    }
                    out.push_str(&c);
                }
            }
            Event::Text(t) if in_body && skip == 0 => {
                out.push_str(&escape(&util::unescape_xml(&AsRef::<str>::as_ref(&t))));
            }
            Event::GeneralRef(g) if in_body && skip == 0 => {
                out.push_str(&escape(&util::unescape_xml(&format!("&{};", AsRef::<str>::as_ref(&g)))));
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let (title, author, language) = parse_meta(&meta);
    let html = format!(
        "<html><head><meta charset=\"utf-8\"/><title>{}</title><style>table.docx{{border-collapse:collapse;max-width:100%}}table.docx td{{border:1px solid #9995;padding:.3em .5em;vertical-align:top}}</style></head><body class=\"odt\">{}</body></html>",
        escape(&title),
        out
    );
    let direction = if rtl_n > ltr_n { Some(TextDirection::Rtl) } else { None };
    Ok(Book {
        title: title.clone(),
        author,
        language,
        direction,
        chapters: vec![Chapter { path: "content.html".into(), title: Some(title), html: Some(html) }],
        toc,
        store,
        cover: None,
        fixed_layout: false,
    })
}

fn emphasis_open(st: &OStyle) -> (String, String) {
    let mut o = String::new();
    let mut c = String::new();
    for (flag, tag) in [(st.bold, "b"), (st.italic, "i"), (st.underline, "u"), (st.strike, "s")] {
        if flag == Some(true) {
            o.push_str(&format!("<{tag}>"));
            c.insert_str(0, &format!("</{tag}>"));
        }
    }
    (o, c)
}

fn parse_meta(s: &str) -> (String, Option<String>, Option<String>) {
    let mut r = quick_xml::Reader::from_str(s);
    let (mut t, mut a, mut l) = (String::new(), None, None);
    let mut cur = String::new();
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Start(e) => cur = local(e.name().into_inner()),
            Event::Text(x) => {
                let v = util::unescape_xml(&AsRef::<str>::as_ref(&x)).trim().to_string();
                if v.is_empty() {
                    continue;
                }
                match cur.as_str() {
                    "title" => t = v,
                    "creator" | "initial-creator" if a.is_none() => a = Some(v),
                    "language" => l = Some(v),
                    _ => {}
                }
            }
            Event::End(_) => cur.clear(),
            Event::Eof => break,
            _ => {}
        }
    }
    (t, a, l)
}
