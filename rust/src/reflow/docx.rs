//! DOCX → HTML. Handles paragraph bidi (`w:bidi`), table `w:bidiVisual`,
//! headings (style names / outline levels, with `basedOn` inheritance),
//! numbered & bulleted lists, bold/italic/underline/strike/sub/sup, colors,
//! hyperlinks, bookmarks, images, tables (colspan) and footnotes.

use super::{Book, Chapter, Store, TocItem};
use crate::rtl::TextDirection;
use crate::util::{self, attr, attr_q, escape, escape_attr, local};
use crate::Result;
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;

#[derive(Default, Clone)]
struct PStyle {
    based_on: Option<String>,
    heading: Option<u32>,
    bidi: Option<bool>,
    jc: Option<String>,
}

#[derive(Default)]
struct Styles {
    p: HashMap<String, PStyle>,
    default_bidi: bool,
    default_pstyle: Option<String>,
}

impl Styles {
    fn resolve(&self, id: Option<&str>) -> PStyle {
        let mut out = PStyle::default();
        let mut cur = id.map(|s| s.to_string()).or_else(|| self.default_pstyle.clone());
        let mut guard = 0;
        while let (Some(c), true) = (cur.clone(), guard < 16) {
            guard += 1;
            let Some(s) = self.p.get(&c) else { break };
            if out.heading.is_none() {
                out.heading = s.heading;
            }
            if out.bidi.is_none() {
                out.bidi = s.bidi;
            }
            if out.jc.is_none() {
                out.jc = s.jc.clone();
            }
            cur = s.based_on.clone();
        }
        if out.bidi.is_none() {
            out.bidi = Some(self.default_bidi);
        }
        out
    }
}

struct Numbering {
    /// numId -> abstractNumId
    nums: HashMap<String, String>,
    /// (abstractNumId, ilvl) -> (numFmt, lvlText, start)
    levels: HashMap<(String, u32), (String, String, u32)>,
    counters: HashMap<String, Vec<u32>>,
}

impl Numbering {
    fn label(&mut self, num_id: &str, ilvl: u32) -> Option<String> {
        if num_id == "0" {
            return None;
        }
        let abs = self.nums.get(num_id)?.clone();
        let counters = self.counters.entry(num_id.to_string()).or_insert_with(|| vec![0; 9]);
        let lvl = (ilvl as usize).min(8);
        counters[lvl] += 1;
        for c in counters.iter_mut().skip(lvl + 1) {
            *c = 0;
        }
        let snapshot = counters.clone();
        let (fmt, text, _) = self.levels.get(&(abs.clone(), ilvl)).cloned().unwrap_or(("bullet".into(), "•".into(), 1));
        if fmt == "bullet" || fmt == "none" {
            let t = text.trim();
            // Symbol-font private-use bullets render as boxes; normalize them.
            let ok = !t.is_empty() && t.chars().all(|c| !(('\u{E000}'..='\u{F8FF}').contains(&c)));
            return Some(if ok { t.to_string() } else { "•".into() });
        }
        let mut label = text.clone();
        for l in 0..=lvl {
            let start = self.levels.get(&(abs.clone(), l as u32)).map(|v| v.2).unwrap_or(1);
            let n = snapshot[l] + start - 1;
            let f = self.levels.get(&(abs.clone(), l as u32)).map(|v| v.0.clone()).unwrap_or_default();
            label = label.replace(&format!("%{}", l + 1), &fmt_num(n, &f));
        }
        Some(label)
    }
}

fn fmt_num(n: u32, fmt: &str) -> String {
    match fmt {
        "lowerLetter" => letters(n, b'a'),
        "upperLetter" => letters(n, b'A'),
        "lowerRoman" => roman(n).to_lowercase(),
        "upperRoman" => roman(n),
        "arabicAbjad" | "arabicAlpha" => {
            const ABJAD: [char; 28] = ['أ', 'ب', 'ج', 'د', 'ه', 'و', 'ز', 'ح', 'ط', 'ي', 'ك', 'ل', 'م', 'ن', 'س', 'ع', 'ف', 'ص', 'ق', 'ر', 'ش', 'ت', 'ث', 'خ', 'ذ', 'ض', 'ظ', 'غ'];
            ABJAD[((n.max(1) - 1) % 28) as usize].to_string()
        }
        "hindiNumbers" | "arabicIndic" => n.to_string().chars().map(|c| char::from_u32(c as u32 - 0x30 + 0x0660).unwrap_or(c)).collect(),
        "hebrew1" | "hebrew2" => {
            const HE: [char; 22] = ['א', 'ב', 'ג', 'ד', 'ה', 'ו', 'ז', 'ח', 'ט', 'י', 'כ', 'ל', 'מ', 'נ', 'ס', 'ע', 'פ', 'צ', 'ק', 'ר', 'ש', 'ת'];
            HE[((n.max(1) - 1) % 22) as usize].to_string()
        }
        _ => n.to_string(),
    }
}

fn letters(mut n: u32, base: u8) -> String {
    let mut s = Vec::new();
    while n > 0 {
        n -= 1;
        s.push((base + (n % 26) as u8) as char);
        n /= 26;
    }
    s.iter().rev().collect()
}

fn roman(mut n: u32) -> String {
    let t = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut s = String::new();
    for (v, r) in t {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

pub fn open(path: &str) -> Result<Book> {
    let store = Store::open_zip(path)?;
    let Store::Zip(zm) = &store else { unreachable!() };
    let mut z = zm.lock().unwrap();

    let doc_xml = util::zip_entry_string(&mut z, "word/document.xml")?;
    let rels = util::zip_entry_string(&mut z, "word/_rels/document.xml.rels").map(|s| parse_rels(&s)).unwrap_or_default();
    let styles = util::zip_entry_string(&mut z, "word/styles.xml").map(|s| parse_styles(&s)).unwrap_or_default();
    let mut numbering = util::zip_entry_string(&mut z, "word/numbering.xml").map(|s| parse_numbering(&s)).unwrap_or(Numbering {
        nums: HashMap::new(),
        levels: HashMap::new(),
        counters: HashMap::new(),
    });
    let foot_rels = util::zip_entry_string(&mut z, "word/_rels/footnotes.xml.rels").map(|s| parse_rels(&s)).unwrap_or_default();
    let footnotes_xml = util::zip_entry_string(&mut z, "word/footnotes.xml").ok();
    let core = util::zip_entry_string(&mut z, "docProps/core.xml").ok();
    drop(z);

    let mut conv = Conv::new(&styles, &mut numbering);
    conv.run(&doc_xml, &rels, false);
    let (rtl_paras, ltr_paras) = (conv.rtl_paras, conv.ltr_paras);
    let mut body = std::mem::take(&mut conv.out);
    let toc = std::mem::take(&mut conv.toc);
    if let Some(f) = footnotes_xml {
        let mut fc = Conv::new(&styles, &mut numbering);
        fc.run(&f, &foot_rels, true);
        if !fc.out.trim().is_empty() {
            body.push_str("<hr class=\"fn-sep\"/><section class=\"footnotes\">");
            body.push_str(&fc.out);
            body.push_str("</section>");
        }
    }

    let (title, author, language) = core.map(|c| parse_core(&c)).unwrap_or_default();
    let direction = if rtl_paras > ltr_paras { Some(TextDirection::Rtl) } else { None };
    let html = format!(
        "<html><head><meta charset=\"utf-8\"/><title>{}</title><style>table.docx{{border-collapse:collapse;max-width:100%}}table.docx td{{border:1px solid #9995;padding:.3em .5em;vertical-align:top}}p.li .ln{{display:inline-block;min-width:1.6em}}sup.fn a{{text-decoration:none}}</style></head><body class=\"docx\">{}</body></html>",
        escape(&title),
        body
    );
    let chapters = vec![Chapter { path: "document.html".into(), title: Some(title.clone()), html: Some(html) }];
    Ok(Book { title, author, language, direction, chapters, toc, store, cover: None, fixed_layout: false })
}

fn parse_rels(s: &str) -> HashMap<String, (String, bool)> {
    let mut m = HashMap::new();
    let mut r = quick_xml::Reader::from_str(s);
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Empty(e) | Event::Start(e) if local(e.name().into_inner()) == "Relationship" => {
                if let (Some(id), Some(t)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    let ext = attr(&e, "TargetMode").as_deref() == Some("External");
                    m.insert(id, (t, ext));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    m
}

fn val(e: &BytesStart) -> Option<String> {
    attr(e, "val")
}

fn on(e: &BytesStart) -> bool {
    !matches!(val(e).as_deref(), Some("0") | Some("false") | Some("off") | Some("none"))
}

fn parse_styles(s: &str) -> Styles {
    let mut st = Styles::default();
    let mut r = quick_xml::Reader::from_str(s);
    let mut cur: Option<(String, PStyle)> = None;
    let mut in_defaults = false;
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Start(e) | Event::Empty(e) => {
                let n = local(e.name().into_inner());
                match n.as_str() {
                    "pPrDefault" => in_defaults = true,
                    "style" => {
                        if attr(&e, "type").as_deref() == Some("paragraph") {
                            let id = attr(&e, "styleId").unwrap_or_default();
                            if attr(&e, "default").as_deref() == Some("1") {
                                st.default_pstyle = Some(id.clone());
                            }
                            cur = Some((id, PStyle::default()));
                        } else {
                            cur = None;
                        }
                    }
                    "name" => {
                        if let (Some((_, p)), Some(v)) = (cur.as_mut(), val(&e)) {
                            let l = v.to_lowercase();
                            if l == "title" {
                                p.heading = Some(1);
                            } else if let Some(rest) = l.strip_prefix("heading ") {
                                p.heading = rest.trim().parse::<u32>().ok();
                            }
                        }
                    }
                    "basedOn" => {
                        if let Some((_, p)) = cur.as_mut() {
                            p.based_on = val(&e);
                        }
                    }
                    "outlineLvl" => {
                        if let (Some((_, p)), Some(v)) = (cur.as_mut(), val(&e).and_then(|v| v.parse::<u32>().ok())) {
                            if v < 9 && p.heading.is_none() {
                                p.heading = Some(v + 1);
                            }
                        }
                    }
                    "bidi" => {
                        if in_defaults {
                            st.default_bidi = on(&e);
                        } else if let Some((_, p)) = cur.as_mut() {
                            p.bidi = Some(on(&e));
                        }
                    }
                    "jc" => {
                        if let Some((_, p)) = cur.as_mut() {
                            p.jc = val(&e);
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => match local(e.name().into_inner()).as_str() {
                "style" => {
                    if let Some((id, p)) = cur.take() {
                        st.p.insert(id, p);
                    }
                }
                "pPrDefault" => in_defaults = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    st
}

fn parse_numbering(s: &str) -> Numbering {
    let mut n = Numbering { nums: HashMap::new(), levels: HashMap::new(), counters: HashMap::new() };
    let mut r = quick_xml::Reader::from_str(s);
    let mut abs: Option<String> = None;
    let mut lvl: Option<(u32, String, String, u32)> = None;
    let mut num: Option<String> = None;
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Start(e) | Event::Empty(e) => match local(e.name().into_inner()).as_str() {
                "abstractNum" => abs = attr(&e, "abstractNumId"),
                "lvl" => lvl = Some((attr(&e, "ilvl").and_then(|v| v.parse().ok()).unwrap_or(0), "decimal".into(), "%1.".into(), 1)),
                "numFmt" => {
                    if let (Some(l), Some(v)) = (lvl.as_mut(), val(&e)) {
                        l.1 = v;
                    }
                }
                "lvlText" => {
                    if let (Some(l), Some(v)) = (lvl.as_mut(), val(&e)) {
                        l.2 = v;
                    }
                }
                "start" => {
                    if let (Some(l), Some(v)) = (lvl.as_mut(), val(&e).and_then(|v| v.parse().ok())) {
                        l.3 = v;
                    }
                }
                "num" => num = attr(&e, "numId"),
                "abstractNumId" => {
                    if let (Some(id), Some(v)) = (num.as_ref(), val(&e)) {
                        n.nums.insert(id.clone(), v);
                    }
                }
                _ => {}
            },
            Event::End(e) => match local(e.name().into_inner()).as_str() {
                "lvl" => {
                    if let (Some(a), Some((i, f, t, s))) = (abs.as_ref(), lvl.take()) {
                        n.levels.insert((a.clone(), i), (f, t, s));
                    }
                }
                "abstractNum" => abs = None,
                "num" => num = None,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    n
}

fn parse_core(s: &str) -> (String, Option<String>, Option<String>) {
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
                    "creator" => a = Some(v),
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

#[derive(Default, Clone)]
struct RunFmt {
    b: bool,
    i: bool,
    u: bool,
    s: bool,
    sup: bool,
    sub: bool,
    color: Option<String>,
    highlight: Option<String>,
}

#[derive(Default)]
struct Para {
    style: Option<String>,
    bidi: Option<bool>,
    jc: Option<String>,
    num: Option<(String, u32)>,
    html: String,
    has_text: bool,
}

struct Conv<'a> {
    styles: &'a Styles,
    numbering: &'a mut Numbering,
    out: String,
    toc: Vec<TocItem>,
    paras: Vec<Para>,
    run: RunFmt,
    in_ppr: bool,
    in_rpr: bool,
    in_text: bool,
    in_instr: bool,
    link_open: Vec<bool>,
    table_pos: Vec<usize>,
    cell_pos: Option<usize>,
    heading_n: u32,
    rtl_paras: u32,
    ltr_paras: u32,
    footnote_mode: bool,
    skip_depth: u32,
}

impl<'a> Conv<'a> {
    fn new(styles: &'a Styles, numbering: &'a mut Numbering) -> Self {
        Conv {
            styles,
            numbering,
            out: String::new(),
            toc: Vec::new(),
            paras: Vec::new(),
            run: RunFmt::default(),
            in_ppr: false,
            in_rpr: false,
            in_text: false,
            in_instr: false,
            link_open: Vec::new(),
            table_pos: Vec::new(),
            cell_pos: None,
            heading_n: 0,
            rtl_paras: 0,
            ltr_paras: 0,
            footnote_mode: false,
            skip_depth: 0,
        }
    }

    fn sink(&mut self) -> &mut String {
        match self.paras.last_mut() {
            Some(p) => &mut p.html,
            None => &mut self.out,
        }
    }

    fn push_text(&mut self, t: &str) {
        if t.is_empty() {
            return;
        }
        let f = self.run.clone();
        let mut open = String::new();
        let mut close = String::new();
        let mut style = String::new();
        if let Some(c) = &f.color {
            if c != "auto" && c.len() == 6 {
                style.push_str(&format!("color:#{c};"));
            }
        }
        if let Some(h) = &f.highlight {
            if h != "none" {
                style.push_str(&format!("background:{h};"));
            }
        }
        if !style.is_empty() {
            open.push_str(&format!("<span style=\"{style}\">"));
            close.insert_str(0, "</span>");
        }
        for (flag, tag) in [(f.b, "b"), (f.i, "i"), (f.u, "u"), (f.s, "s"), (f.sup, "sup"), (f.sub, "sub")] {
            if flag {
                open.push_str(&format!("<{tag}>"));
                close.insert_str(0, &format!("</{tag}>"));
            }
        }
        let s = format!("{open}{}{close}", escape(t));
        if let Some(p) = self.paras.last_mut() {
            p.has_text = true;
        }
        self.sink().push_str(&s);
    }

    fn end_para(&mut self) {
        let Some(p) = self.paras.pop() else { return };
        let st = self.styles.resolve(p.style.as_deref());
        let bidi = p.bidi.or(st.bidi).unwrap_or(false);
        if p.has_text {
            if bidi {
                self.rtl_paras += 1;
            } else {
                self.ltr_paras += 1;
            }
        }
        let jc = p.jc.clone().or(st.jc.clone());
        let mut style = String::new();
        match jc.as_deref() {
            Some("center") => style.push_str("text-align:center;"),
            Some("both") | Some("distribute") | Some("lowKashida") | Some("mediumKashida") | Some("highKashida") => style.push_str("text-align:justify;"),
            _ => {}
        }
        let dir = if bidi { " dir=\"rtl\"" } else { "" };
        let heading = st.heading.filter(|h| *h >= 1 && !self.footnote_mode);
        let html = if let Some(h) = heading {
            let h = h.min(6);
            self.heading_n += 1;
            let id = format!("h{}", self.heading_n);
            let plain = util::html_to_text(&format!("<body>{}</body>", p.html));
            let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            if !plain.is_empty() {
                self.toc.push(TocItem { title: plain, chapter: 0, anchor: Some(id.clone()), level: h - 1 });
            }
            format!("<h{h} id=\"{id}\"{dir} style=\"{style}\">{}</h{h}>", p.html)
        } else if let Some((num_id, ilvl)) = &p.num {
            let label = self.numbering.label(num_id, *ilvl).unwrap_or_default();
            style.push_str(&format!("margin-inline-start:{:.1}em;", 1.5 * (*ilvl as f32 + 1.0)));
            format!("<p class=\"li\"{dir} style=\"{style}\"><span class=\"ln\">{}</span> {}</p>", escape(&label), p.html)
        } else if p.html.trim().is_empty() {
            "<p class=\"empty\">&#160;</p>".to_string()
        } else {
            format!("<p{dir} style=\"{style}\">{}</p>", p.html)
        };
        self.sink().push_str(&html);
    }

    fn open(&mut self, e: &BytesStart, empty: bool, rels: &HashMap<String, (String, bool)>) {
        let n = local(e.name().into_inner());
        if self.skip_depth > 0 {
            if !empty {
                self.skip_depth += 1;
            }
            return;
        }
        match n.as_str() {
            // deleted text, field codes, alternate content fallbacks
            "del" | "fldSimple_instr" | "Fallback" | "instrText" => {
                if n == "instrText" {
                    self.in_instr = !empty;
                } else if !empty {
                    self.skip_depth = 1;
                }
            }
            "footnote" if self.footnote_mode => {
                let id = attr(e, "id").unwrap_or_default();
                let t = attr(e, "type");
                if t.is_some() && t.as_deref() != Some("normal") {
                    if !empty {
                        self.skip_depth = 1;
                    }
                } else {
                    let s = format!("<div class=\"fn\" id=\"fn{}\">", escape_attr(&id));
                    self.out.push_str(&s);
                }
            }
            "p" => self.paras.push(Para::default()),
            "pPr" => self.in_ppr = !empty,
            "rPr" => self.in_rpr = !empty,
            "pStyle" if self.in_ppr => {
                if let Some(p) = self.paras.last_mut() {
                    p.style = val(e);
                }
            }
            "bidi" if self.in_ppr && !self.in_rpr => {
                let v = on(e);
                if let Some(p) = self.paras.last_mut() {
                    p.bidi = Some(v);
                }
            }
            "jc" if self.in_ppr && !self.in_rpr => {
                if let Some(p) = self.paras.last_mut() {
                    p.jc = val(e);
                }
            }
            "numId" if self.in_ppr => {
                if let (Some(p), Some(v)) = (self.paras.last_mut(), val(e)) {
                    let lvl = p.num.as_ref().map(|n| n.1).unwrap_or(0);
                    p.num = Some((v, lvl));
                }
            }
            "ilvl" if self.in_ppr => {
                if let Some(p) = self.paras.last_mut() {
                    let l = val(e).and_then(|v| v.parse().ok()).unwrap_or(0);
                    let id = p.num.as_ref().map(|n| n.0.clone()).unwrap_or_default();
                    p.num = Some((id, l));
                }
            }
            "r" => self.run = RunFmt::default(),
            "b" | "bCs" if self.in_rpr && !self.in_ppr => self.run.b = on(e),
            "i" | "iCs" if self.in_rpr && !self.in_ppr => self.run.i = on(e),
            "u" if self.in_rpr && !self.in_ppr => self.run.u = on(e),
            "strike" | "dstrike" if self.in_rpr && !self.in_ppr => self.run.s = on(e),
            "vertAlign" if self.in_rpr && !self.in_ppr => match val(e).as_deref() {
                Some("superscript") => self.run.sup = true,
                Some("subscript") => self.run.sub = true,
                _ => {}
            },
            "color" if self.in_rpr && !self.in_ppr => self.run.color = val(e),
            "highlight" if self.in_rpr && !self.in_ppr => self.run.highlight = val(e),
            "t" | "delText" => self.in_text = !empty && n == "t",
            "tab" if !self.in_ppr => self.sink().push_str("&#8195;"),
            "br" | "cr" => {
                if attr(e, "type").as_deref() == Some("page") {
                    self.sink().push_str("<br class=\"pb\"/>");
                } else {
                    self.sink().push_str("<br/>");
                }
            }
            "noBreakHyphen" => self.sink().push_str("&#8209;"),
            "sym" => {
                if let Some(c) = attr(e, "char").and_then(|c| u32::from_str_radix(&c, 16).ok()).and_then(char::from_u32) {
                    if !('\u{E000}'..='\u{F8FF}').contains(&c) {
                        self.push_text(&c.to_string());
                    }
                }
            }
            "hyperlink" => {
                let href = attr_q(e, "r:id")
                    .and_then(|id| rels.get(&id).map(|(t, _)| t.clone()))
                    .or_else(|| attr(e, "anchor").map(|a| format!("#{a}")));
                match href {
                    Some(h) if !empty => {
                        let s = format!("<a href=\"{}\">", escape_attr(&h));
                        self.sink().push_str(&s);
                        self.link_open.push(true);
                    }
                    _ if !empty => self.link_open.push(false),
                    _ => {}
                }
            }
            "bookmarkStart" => {
                if let Some(name) = attr(e, "name") {
                    if !name.starts_with("_GoBack") {
                        let s = format!("<a id=\"{}\"></a>", escape_attr(&name));
                        self.sink().push_str(&s);
                    }
                }
            }
            "blip" => {
                if let Some(id) = attr_q(e, "r:embed").or_else(|| attr_q(e, "r:link")) {
                    self.image(rels, &id);
                }
            }
            "imagedata" => {
                if let Some(id) = attr_q(e, "r:id") {
                    self.image(rels, &id);
                }
            }
            "footnoteReference" => {
                if let Some(id) = attr(e, "id") {
                    let s = format!("<sup class=\"fn\"><a href=\"#fn{0}\" id=\"fnref{0}\">{0}</a></sup>", escape_attr(&id));
                    self.sink().push_str(&s);
                }
            }
            "footnoteRef" if self.footnote_mode => {
                // the number at the start of a footnote's own text; handled by the div id
            }
            "tbl" => {
                self.end_open_text_para_if_any();
                let pos = self.sink().len();
                self.table_pos.push(pos);
                self.sink().push_str("<table class=\"docx\">");
            }
            "bidiVisual" => {
                if let Some(pos) = self.table_pos.last().copied() {
                    if on(e) {
                        let tgt = self.sink();
                        if tgt.len() >= pos + 6 {
                            tgt.insert_str(pos + 6, " dir=\"rtl\"");
                        }
                    }
                }
            }
            "tr" => self.sink().push_str("<tr>"),
            "tc" => {
                let pos = self.sink().len();
                self.cell_pos = Some(pos);
                self.sink().push_str("<td>");
            }
            "gridSpan" => {
                if let (Some(pos), Some(v)) = (self.cell_pos, val(e)) {
                    let tgt = self.sink();
                    if tgt.len() >= pos + 3 && tgt[pos..].starts_with("<td") {
                        tgt.insert_str(pos + 3, &format!(" colspan=\"{}\"", escape_attr(&v)));
                    }
                }
            }
            _ => {}
        }
    }

    fn end_open_text_para_if_any(&mut self) {}

    fn image(&mut self, rels: &HashMap<String, (String, bool)>, id: &str) {
        if let Some((t, ext)) = rels.get(id) {
            if *ext {
                return; // never fetch remote images
            }
            let p = util::resolve_path("word/document.xml", t);
            let s = format!("<img src=\"{}\" alt=\"\"/>", escape_attr(&p));
            self.sink().push_str(&s);
        }
    }

    fn close(&mut self, n: &str) {
        if self.skip_depth > 0 {
            self.skip_depth -= 1;
            return;
        }
        match n {
            "p" => self.end_para(),
            "pPr" => self.in_ppr = false,
            "rPr" => self.in_rpr = false,
            "t" => self.in_text = false,
            "instrText" => self.in_instr = false,
            "hyperlink" => {
                if self.link_open.pop() == Some(true) {
                    self.sink().push_str("</a>");
                }
            }
            "tbl" => {
                self.table_pos.pop();
                self.sink().push_str("</table>");
            }
            "tr" => self.sink().push_str("</tr>"),
            "tc" => {
                self.cell_pos = None;
                self.sink().push_str("</td>");
            }
            "footnote" if self.footnote_mode => self.out.push_str("</div>"),
            _ => {}
        }
    }

    fn run(&mut self, xml: &str, rels: &HashMap<String, (String, bool)>, footnote_mode: bool) {
        self.footnote_mode = footnote_mode;
        let mut r = quick_xml::Reader::from_str(xml);
        r.config_mut().check_end_names = false;
        loop {
            let ev = match r.read_event() {
                Ok(e) => e,
                Err(_) => break,
            };
            match ev {
                Event::Start(e) => self.open(&e, false, rels),
                Event::Empty(e) => {
                    self.open(&e, true, rels);
                    let n = local(e.name().into_inner());
                    if matches!(n.as_str(), "p" | "tc" | "tr" | "tbl") {
                        self.close(&n);
                    }
                }
                Event::End(e) => {
                    let n = local(e.name().into_inner());
                    self.close(&n);
                }
                Event::Text(t) => {
                    if self.in_text && self.skip_depth == 0 {
                        let s = util::unescape_xml(&AsRef::<str>::as_ref(&t));
                        self.push_text(&s);
                    }
                }
                Event::GeneralRef(g) => {
                    if self.in_text && self.skip_depth == 0 {
                        let s = util::unescape_xml(&format!("&{};", AsRef::<str>::as_ref(&g)));
                        self.push_text(&s);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        while !self.paras.is_empty() {
            self.end_para();
        }
    }
}
