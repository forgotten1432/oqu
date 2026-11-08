//! EPUB 2/3: container.xml → OPF (metadata, manifest, spine with
//! `page-progression-direction`) → nav.xhtml or NCX for the TOC.

use super::{Book, Chapter, Store, TocItem};
use crate::rtl::TextDirection;
use crate::util::{self, attr, local};
use crate::{ReaderError, Result};
use quick_xml::events::Event;
use std::collections::HashMap;

struct Item {
    href: String,
    media: String,
    props: String,
}

pub fn open(path: &str) -> Result<Book> {
    let store = Store::open_zip(path)?;
    let Store::Zip(z) = &store else { unreachable!() };
    let mut z = z.lock().unwrap();

    let container = util::zip_entry_string(&mut z, "META-INF/container.xml")?;
    let opf_path = {
        let mut r = quick_xml::Reader::from_str(&container);
        let mut found = None;
        loop {
            match r.read_event()? {
                Event::Start(e) | Event::Empty(e) if local(e.name().into_inner()) == "rootfile" => {
                    let mt = attr(&e, "media-type").unwrap_or_default();
                    if found.is_none() || mt == "application/oebps-package+xml" {
                        found = attr(&e, "full-path");
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        found.ok_or(ReaderError::Parse { msg: "no rootfile".into() })?
    };
    let opf = util::zip_entry_string(&mut z, &opf_path)?;

    let mut title = String::new();
    let mut author: Option<String> = None;
    let mut language: Option<String> = None;
    let mut manifest: HashMap<String, Item> = HashMap::new();
    let mut spine: Vec<String> = Vec::new();
    let mut ppd: Option<String> = None;
    let mut ncx_id: Option<String> = None;
    let mut cover_id: Option<String> = None;
    let mut fixed_layout = false;
    {
        let mut r = quick_xml::Reader::from_str(&opf);
        let mut cur: Option<String> = None;
        let mut text = String::new();
        loop {
            match r.read_event()? {
                Event::Start(e) => {
                    let n = local(e.name().into_inner());
                    match n.as_str() {
                        "title" | "creator" | "language" => {
                            cur = Some(n);
                            text.clear();
                        }
                        "spine" => {
                            ppd = attr(&e, "page-progression-direction");
                            ncx_id = attr(&e, "toc");
                        }
                        "meta" => meta(&e, &mut cover_id, &mut fixed_layout, &mut cur, &mut text),
                        _ => {}
                    }
                }
                Event::Empty(e) => {
                    let n = local(e.name().into_inner());
                    match n.as_str() {
                        "item" => {
                            if let (Some(id), Some(href)) = (attr(&e, "id"), attr(&e, "href")) {
                                let item = Item {
                                    href: util::resolve_path(&opf_path, &href),
                                    media: attr(&e, "media-type").unwrap_or_default(),
                                    props: attr(&e, "properties").unwrap_or_default(),
                                };
                                manifest.insert(id, item);
                            }
                        }
                        "itemref" => {
                            if attr(&e, "linear").as_deref() != Some("no") || spine.is_empty() {
                                if let Some(id) = attr(&e, "idref") {
                                    spine.push(id);
                                }
                            }
                        }
                        "spine" => {
                            ppd = attr(&e, "page-progression-direction");
                            ncx_id = attr(&e, "toc");
                        }
                        "meta" => {
                            let mut dummy = None;
                            let mut t = String::new();
                            meta(&e, &mut cover_id, &mut fixed_layout, &mut dummy, &mut t)
                        }
                        _ => {}
                    }
                }
                Event::Text(t) => {
                    if cur.is_some() {
                        text.push_str(&util::unescape_xml(&AsRef::<str>::as_ref(&t)));
                    }
                }
                Event::GeneralRef(g) => {
                    if cur.is_some() {
                        text.push_str(&util::unescape_xml(&format!("&{};", AsRef::<str>::as_ref(&g))));
                    }
                }
                Event::CData(t) => {
                    if cur.is_some() {
                        text.push_str(&AsRef::<str>::as_ref(&t));
                    }
                }
                Event::End(e) => {
                    let n = local(e.name().into_inner());
                    if cur.as_deref() == Some(n.as_str()) {
                        let v = text.trim().to_string();
                        match n.as_str() {
                            "title" if title.is_empty() => title = v,
                            "creator" if author.is_none() && !v.is_empty() => author = Some(v),
                            "language" if language.is_none() && !v.is_empty() => language = Some(v),
                            "meta" if v == "pre-paginated" => fixed_layout = true,
                            _ => {}
                        }
                        cur = None;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }

    let chapters: Vec<Chapter> = spine
        .iter()
        .filter_map(|id| manifest.get(id))
        .filter(|it| it.media.contains("html") || it.media.contains("xml") || it.media.is_empty() || it.media == "image/svg+xml")
        .map(|it| Chapter { path: it.href.clone(), title: None, html: None })
        .collect();
    let index: HashMap<&str, u32> = chapters.iter().enumerate().map(|(i, c)| (c.path.as_str(), i as u32)).collect();

    let mut toc = Vec::new();
    if let Some(nav) = manifest.values().find(|i| i.props.split_whitespace().any(|p| p == "nav")) {
        if let Ok(s) = util::zip_entry_string(&mut z, &nav.href) {
            parse_nav(&s, &nav.href, &index, &mut toc);
        }
    }
    if toc.is_empty() {
        let ncx = ncx_id
            .and_then(|id| manifest.get(&id))
            .or_else(|| manifest.values().find(|i| i.media == "application/x-dtbncx+xml"));
        if let Some(ncx) = ncx {
            if let Ok(s) = util::zip_entry_string(&mut z, &ncx.href) {
                parse_ncx(&s, &ncx.href, &index, &mut toc);
            }
        }
    }

    let cover = manifest
        .values()
        .find(|i| i.props.split_whitespace().any(|p| p == "cover-image"))
        .or_else(|| cover_id.as_ref().and_then(|id| manifest.get(id)))
        .or_else(|| manifest.iter().find(|(id, i)| id.to_lowercase().contains("cover") && i.media.starts_with("image/")).map(|(_, i)| i))
        .map(|i| i.href.clone());

    let direction = match ppd.as_deref() {
        Some("rtl") => Some(TextDirection::Rtl),
        Some("ltr") => Some(TextDirection::Ltr),
        _ => None,
    };

    drop(z);
    let mut chapters = chapters;
    for t in &toc {
        {
            if let Some(c) = chapters.get_mut(t.chapter as usize) {
                if c.title.is_none() {
                    c.title = Some(t.title.clone());
                }
            }
        }
    }
    Ok(Book { title, author, language, direction, chapters, toc, store, cover, fixed_layout })
}

fn meta(
    e: &quick_xml::events::BytesStart,
    cover_id: &mut Option<String>,
    fixed: &mut bool,
    cur: &mut Option<String>,
    text: &mut String,
) {
    if attr(e, "name").as_deref() == Some("cover") {
        *cover_id = attr(e, "content");
    }
    if attr(e, "property").as_deref() == Some("rendition:layout") {
        *cur = Some("meta".into());
        text.clear();
    }
    if attr(e, "name").as_deref() == Some("fixed-layout") && attr(e, "content").as_deref() == Some("true") {
        *fixed = true;
    }
}

fn split_href(base: &str, href: &str) -> (String, Option<String>) {
    let path = util::resolve_path(base, href);
    let anchor = href.split_once('#').map(|(_, a)| a.to_string()).filter(|a| !a.is_empty());
    (path, anchor)
}

fn parse_nav(s: &str, base: &str, index: &HashMap<&str, u32>, toc: &mut Vec<TocItem>) {
    let mut r = quick_xml::Reader::from_str(s);
    r.config_mut().check_end_names = false;
    let mut in_toc = false;
    let mut nav_depth = 0i32;
    let mut ol_depth = 0u32;
    let mut cur_href: Option<String> = None;
    let mut text = String::new();
    let mut in_link = false;
    loop {
        let ev = match r.read_event() {
            Ok(e) => e,
            Err(_) => break,
        };
        match ev {
            Event::Start(e) => {
                let n = local(e.name().into_inner());
                if n == "nav" {
                    if in_toc {
                        nav_depth += 1;
                    } else if attr(&e, "type").map(|t| t.contains("toc")).unwrap_or(false) {
                        in_toc = true;
                        nav_depth = 1;
                    }
                } else if in_toc && n == "ol" {
                    ol_depth += 1;
                } else if in_toc && (n == "a" || n == "span") && !in_link {
                    in_link = true;
                    cur_href = attr(&e, "href");
                    text.clear();
                }
            }
            Event::Text(t) if in_link => text.push_str(&util::unescape_xml(&AsRef::<str>::as_ref(&t))),
            Event::GeneralRef(g) if in_link => text.push_str(&util::unescape_xml(&format!("&{};", AsRef::<str>::as_ref(&g)))),
            Event::End(e) => {
                let n = local(e.name().into_inner());
                if in_toc && n == "nav" {
                    nav_depth -= 1;
                    if nav_depth == 0 {
                        break;
                    }
                } else if in_toc && n == "ol" {
                    ol_depth = ol_depth.saturating_sub(1);
                } else if in_link && (n == "a" || n == "span") {
                    in_link = false;
                    if let Some(h) = cur_href.take() {
                        let (p, anchor) = split_href(base, &h);
                        if let Some(&ch) = index.get(p.as_str()) {
                            toc.push(TocItem { title: clean(&text), chapter: ch, anchor, level: ol_depth.saturating_sub(1) });
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
}

fn parse_ncx(s: &str, base: &str, index: &HashMap<&str, u32>, toc: &mut Vec<TocItem>) {
    let mut r = quick_xml::Reader::from_str(s);
    r.config_mut().check_end_names = false;
    let mut depth = 0u32;
    let mut label = String::new();
    let mut in_text = false;
    loop {
        let ev = match r.read_event() {
            Ok(e) => e,
            Err(_) => break,
        };
        match ev {
            Event::Start(e) => match local(e.name().into_inner()).as_str() {
                "navPoint" => {
                    depth += 1;
                    label.clear();
                }
                "text" => in_text = true,
                _ => {}
            },
            Event::Empty(e) if local(e.name().into_inner()) == "content" => {
                if let Some(src) = attr(&e, "src") {
                    let (p, anchor) = split_href(base, &src);
                    if let Some(&ch) = index.get(p.as_str()) {
                        toc.push(TocItem { title: clean(&label), chapter: ch, anchor, level: depth.saturating_sub(1) });
                    }
                }
            }
            Event::Text(t) if in_text => label.push_str(&util::unescape_xml(&AsRef::<str>::as_ref(&t))),
            Event::GeneralRef(g) if in_text => label.push_str(&util::unescape_xml(&format!("&{};", AsRef::<str>::as_ref(&g)))),
            Event::End(e) => match local(e.name().into_inner()).as_str() {
                "navPoint" => depth = depth.saturating_sub(1),
                "text" => in_text = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
