//! Chapter preparation: strip scripts/handlers, keep `dir`/`lang`/`style`/
//! `class` (unlike generic sanitizers, which silently drop `dir` and break
//! RTL books), set the document direction, and add the head marker.

use super::HEAD_MARKER;
use crate::rtl::TextDirection;
use regex::Regex;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid regex"))
}

static SCRIPT: OnceLock<Regex> = OnceLock::new();
static SCRIPT_SELF: OnceLock<Regex> = OnceLock::new();
static ON_ATTR: OnceLock<Regex> = OnceLock::new();
static JS_URL: OnceLock<Regex> = OnceLock::new();
static HTML_TAG: OnceLock<Regex> = OnceLock::new();
static HEAD_OPEN: OnceLock<Regex> = OnceLock::new();
static HEAD_CLOSE: OnceLock<Regex> = OnceLock::new();
static META_REFRESH: OnceLock<Regex> = OnceLock::new();
static XML_DECL: OnceLock<Regex> = OnceLock::new();
static CHARSET: OnceLock<Regex> = OnceLock::new();
static IFRAME: OnceLock<Regex> = OnceLock::new();

pub fn prepare(raw: &str, dir: TextDirection, lang: Option<&str>, path: &str) -> (String, bool) {
    let mut s = re(&SCRIPT, r"(?is)<script\b[^>]*>.*?</script\s*>").replace_all(raw, "").into_owned();
    s = re(&SCRIPT_SELF, r"(?is)<script\b[^>]*/>").replace_all(&s, "").into_owned();
    s = re(&IFRAME, r"(?is)<(iframe|object|embed)\b[^>]*>(.*?</(iframe|object|embed)\s*>)?").replace_all(&s, "").into_owned();
    s = re(&ON_ATTR, r#"(?i)\s+on[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#).replace_all(&s, "").into_owned();
    s = re(&JS_URL, r#"(?i)(href|src|xlink:href)\s*=\s*(["'])\s*javascript:[^"']*(["'])"#)
        .replace_all(&s, "$1=$2#$3")
        .into_owned();
    s = re(&META_REFRESH, r#"(?i)<meta\b[^>]*http-equiv\s*=\s*["']?refresh[^>]*>"#).replace_all(&s, "").into_owned();

    // <html dir=… lang=…>
    let html_re = re(&HTML_TAG, r"(?is)<html\b([^>]*)>");
    if let Some(m) = html_re.captures(&s) {
        let attrs = m.get(1).map(|a| a.as_str()).unwrap_or("");
        let mut extra = String::new();
        if !attrs.to_ascii_lowercase().contains("dir=") {
            extra.push_str(&format!(" dir=\"{}\"", dir.as_attr()));
        }
        if let Some(l) = lang {
            if !attrs.contains("lang=") {
                extra.push_str(&format!(" lang=\"{}\" xml:lang=\"{}\"", crate::util::escape_attr(l), crate::util::escape_attr(l)));
            }
        }
        let attrs = attrs.trim_end_matches('/');
        let rep = format!("<html{attrs}{extra}>");
        let r = m.get(0).unwrap().range();
        s.replace_range(r, &rep);
    } else {
        s = format!(
            "<html dir=\"{}\"{}><head></head><body>{}</body></html>",
            dir.as_attr(),
            lang.map(|l| format!(" lang=\"{}\"", crate::util::escape_attr(l))).unwrap_or_default(),
            s
        );
    }

    if let Some(m) = re(&HEAD_CLOSE, r"(?i)</head\s*>").find(&s) {
        s.insert_str(m.start(), HEAD_MARKER);
    } else if let Some(m) = re(&HEAD_OPEN, r"(?i)<head\b[^>]*/>").find(&s) {
        s.replace_range(m.range(), &format!("<head>{HEAD_MARKER}</head>"));
    } else if let Some(m) = html_re.find(&s) {
        s.insert_str(m.end(), &format!("<head>{HEAD_MARKER}</head>"));
    }

    let lower = path.to_ascii_lowercase();
    let wants_xml = lower.ends_with(".xhtml") || lower.ends_with(".xht") || lower.ends_with(".xml") || s.trim_start().starts_with("<?xml");
    let is_xml = wants_xml && well_formed(&s);
    if !is_xml {
        s = re(&XML_DECL, r"^\s*<\?xml[^>]*\?>").replace(&s, "").into_owned();
        if !s.trim_start().to_ascii_lowercase().starts_with("<!doctype") {
            s.insert_str(0, "<!DOCTYPE html>\n");
        }
    }
    (s, is_xml)
}

/// True if the document parses as XML (so it can be served as XHTML).
fn well_formed(s: &str) -> bool {
    let mut r = quick_xml::Reader::from_str(s);
    r.config_mut().check_end_names = true;
    let mut depth: i64 = 0;
    loop {
        match r.read_event() {
            Ok(quick_xml::events::Event::Eof) => return depth == 0,
            Ok(quick_xml::events::Event::Start(_)) => depth += 1,
            Ok(quick_xml::events::Event::End(_)) => depth -= 1,
            Ok(quick_xml::events::Event::GeneralRef(g)) => {
                // Undefined HTML entities (&nbsp;) are fatal in XHTML.
                let name: &str = g.as_ref();
                if !(name.starts_with('#') || matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")) {
                    return false;
                }
            }
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

pub fn declared_charset(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).into_owned();
    re(&CHARSET, r#"(?i)(?:encoding|charset)\s*=\s*["']?([A-Za-z0-9_\-]+)"#)
        .captures(&head)
        .map(|c| c[1].to_string())
}

/// Book CSS is passed through; only remote `@import`s are dropped so the
/// reader never touches the network.
pub fn css(data: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(data);
    if !s.contains("@import") {
        return data.to_vec();
    }
    s.lines()
        .filter(|l| !(l.contains("@import") && (l.contains("http://") || l.contains("https://"))))
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_dir_and_strips_scripts() {
        let raw = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>x</title></head><body><p dir="rtl" onclick="evil()">مرحبا</p><script>alert(1)</script></body></html>"#;
        let (out, xml) = prepare(raw, TextDirection::Rtl, Some("ar"), "a.xhtml");
        assert!(xml);
        assert!(out.contains(r#"<p dir="rtl">"#));
        assert!(!out.contains("alert"));
        assert!(out.contains(r#"dir="rtl" lang="ar""#));
        assert!(out.contains(HEAD_MARKER));
    }
    #[test]
    fn falls_back_to_html_on_nbsp() {
        let raw = "<html><head></head><body><p>a&nbsp;b</p></body></html>";
        let (out, xml) = prepare(raw, TextDirection::Ltr, None, "a.xhtml");
        assert!(!xml);
        assert!(out.starts_with("<!DOCTYPE html>"));
    }
}
