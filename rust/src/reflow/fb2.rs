//! FictionBook 2 (.fb2) → HTML, with embedded base64 images.

use super::{Book, Chapter, Store, TocItem};
use crate::util::{self, attr, attr_q, escape, escape_attr, local};
use crate::Result;
use quick_xml::events::Event;
use std::collections::HashMap;

pub fn open(path: &str) -> Result<Book> {
    let bytes = std::fs::read(path)?;
    let xml = util::decode_text(&bytes, super::sanitize::declared_charset(&bytes).as_deref());
    let mut r = quick_xml::Reader::from_str(&xml);
    r.config_mut().check_end_names = false;

    let mut out = String::new();
    let mut toc = Vec::new();
    let mut res: HashMap<String, (String, Vec<u8>)> = HashMap::new();
    let mut title = String::new();
    let (mut first, mut last) = (String::new(), String::new());
    let mut lang: Option<String> = None;
    let mut path_stack: Vec<String> = Vec::new();
    let mut close_stack: Vec<&'static str> = Vec::new();
    let mut section_depth = 0u32;
    let mut sec_n = 0u32;
    let mut title_start: Option<(usize, String, u32)> = None;
    let mut binary: Option<(String, String)> = None;
    let mut bin_data = String::new();
    let mut cover_href: Option<String> = None;
    let mut body_n = 0;

    loop {
        let ev = match r.read_event() {
            Ok(e) => e,
            Err(_) => break,
        };
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                let n = local(e.name().into_inner());
                let in_body = path_stack.iter().any(|p| p == "body");
                let in_desc = path_stack.iter().any(|p| p == "title-info");
                if n == "binary" && !empty {
                    let id = attr(e, "id").unwrap_or_default();
                    let ct = attr(e, "content-type").unwrap_or_else(|| "image/jpeg".into());
                    binary = Some((id, ct));
                    bin_data.clear();
                }
                if in_desc && n == "image" && path_stack.last().map(|s| s == "coverpage").unwrap_or(false) {
                    cover_href = attr_q(e, "l:href").or_else(|| attr(e, "href")).map(|h| h.trim_start_matches('#').to_string());
                }
                let mut close: &'static str = "";
                if in_body || n == "body" {
                    match n.as_str() {
                        "body" => {
                            body_n += 1;
                            if body_n > 1 {
                                out.push_str("<hr/><div class=\"notes\">");
                                close = "</div>";
                            }
                        }
                        "section" => {
                            section_depth += 1;
                            sec_n += 1;
                            let id = attr(e, "id").unwrap_or_else(|| format!("s{sec_n}"));
                            out.push_str(&format!("<section id=\"{}\">", escape_attr(&id)));
                            close = "</section>";
                        }
                        "title" => {
                            let lvl = section_depth.clamp(1, 6);
                            let id = format!("t{}", toc.len() + 1);
                            out.push_str(&format!("<h{lvl} id=\"{id}\">"));
                            title_start = Some((out.len(), id, lvl));
                            close = ["", "</h1>", "</h2>", "</h3>", "</h4>", "</h5>", "</h6>"][lvl as usize];
                        }
                        "subtitle" => {
                            out.push_str("<h4 class=\"subtitle\">");
                            close = "</h4>";
                        }
                        "p" => {
                            out.push_str("<p>");
                            close = "</p>";
                        }
                        "epigraph" => {
                            out.push_str("<blockquote class=\"epigraph\">");
                            close = "</blockquote>";
                        }
                        "cite" => {
                            out.push_str("<blockquote>");
                            close = "</blockquote>";
                        }
                        "poem" | "stanza" => {
                            out.push_str("<div class=\"poem\">");
                            close = "</div>";
                        }
                        "v" => {
                            out.push_str("<p class=\"verse\">");
                            close = "</p>";
                        }
                        "text-author" => {
                            out.push_str("<p class=\"author\">");
                            close = "</p>";
                        }
                        "emphasis" => {
                            out.push_str("<em>");
                            close = "</em>";
                        }
                        "strong" => {
                            out.push_str("<strong>");
                            close = "</strong>";
                        }
                        "strikethrough" => {
                            out.push_str("<s>");
                            close = "</s>";
                        }
                        "sup" => {
                            out.push_str("<sup>");
                            close = "</sup>";
                        }
                        "sub" => {
                            out.push_str("<sub>");
                            close = "</sub>";
                        }
                        "code" => {
                            out.push_str("<code>");
                            close = "</code>";
                        }
                        "empty-line" => out.push_str("<br/>"),
                        "a" => {
                            let h = attr_q(e, "l:href").or_else(|| attr(e, "href")).unwrap_or_default();
                            out.push_str(&format!("<a href=\"{}\">", escape_attr(&h)));
                            close = "</a>";
                        }
                        "image" => {
                            if let Some(h) = attr_q(e, "l:href").or_else(|| attr(e, "href")) {
                                let id = h.trim_start_matches('#');
                                out.push_str(&format!("<img src=\"bin/{}\" alt=\"\"/>", escape_attr(id)));
                            }
                        }
                        "table" => {
                            out.push_str("<table>");
                            close = "</table>";
                        }
                        "tr" => {
                            out.push_str("<tr>");
                            close = "</tr>";
                        }
                        "td" | "th" => {
                            out.push_str("<td>");
                            close = "</td>";
                        }
                        _ => {}
                    }
                }
                if !empty {
                    path_stack.push(n);
                    close_stack.push(close);
                } else if n == "section" {
                    section_depth -= 1;
                    out.push_str(close);
                }
            }
            Event::End(e) => {
                let n = local(e.name().into_inner());
                if n == "binary" {
                    if let Some((id, ct)) = binary.take() {
                        res.insert(format!("bin/{id}"), (ct, util::base64_decode(&bin_data)));
                    }
                }
                if n == "section" {
                    section_depth = section_depth.saturating_sub(1);
                }
                if n == "title" {
                    if let Some((start, id, lvl)) = title_start.take() {
                        let t = util::html_to_text(&format!("<body>{}</body>", &out[start..]));
                        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !t.is_empty() && body_n <= 1 {
                            toc.push(TocItem { title: t, chapter: 0, anchor: Some(id), level: lvl - 1 });
                        }
                    }
                }
                path_stack.pop();
                if let Some(c) = close_stack.pop() {
                    out.push_str(c);
                }
            }
            Event::Text(t) => {
                let s = util::unescape_xml(&AsRef::<str>::as_ref(&t));
                if binary.is_some() {
                    bin_data.push_str(&s);
                    continue;
                }
                let cur = path_stack.last().map(|s| s.as_str()).unwrap_or("");
                let in_ti = path_stack.iter().any(|p| p == "title-info");
                if in_ti {
                    match cur {
                        "book-title" => title.push_str(s.trim()),
                        "first-name" if first.is_empty() => first = s.trim().into(),
                        "last-name" if last.is_empty() => last = s.trim().into(),
                        "lang" => lang = Some(s.trim().into()),
                        _ => {}
                    }
                } else if path_stack.iter().any(|p| p == "body") {
                    out.push_str(&escape(&s));
                }
            }
            Event::GeneralRef(g) => {
                if path_stack.iter().any(|p| p == "body") && binary.is_none() {
                    out.push_str(&escape(&util::unescape_xml(&format!("&{};", AsRef::<str>::as_ref(&g)))));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let author = {
        let a = format!("{first} {last}").trim().to_string();
        if a.is_empty() { None } else { Some(a) }
    };
    let html = format!(
        "<html><head><meta charset=\"utf-8\"/><title>{}</title><style>.poem{{margin:1em 2em}}.verse{{margin:0}}.epigraph{{font-style:italic;margin-inline-start:30%}}.author{{text-align:end;font-style:italic}}</style></head><body class=\"fb2\">{}</body></html>",
        escape(&title),
        out
    );
    Ok(Book {
        title: title.clone(),
        author,
        language: lang,
        direction: None,
        chapters: vec![Chapter { path: "book.html".into(), title: Some(title), html: Some(html) }],
        toc,
        store: Store::Memory(res),
        cover: cover_href.map(|c| format!("bin/{c}")),
        fixed_layout: false,
    })
}
