use crate::{ReaderError, Result};
use std::io::Read;

pub fn is_image_name(n: &str) -> bool {
    let l = n.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".avif", ".svg"]
        .iter()
        .any(|e| l.ends_with(e))
}

pub fn mime_for(path: &str) -> &'static str {
    let l = path.to_ascii_lowercase();
    let ext = l.rsplit('.').next().unwrap_or("");
    match ext {
        "xhtml" | "xht" => "application/xhtml+xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "xml" | "opf" | "ncx" => "application/xml",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// Decodes bytes of unknown encoding: BOM → declared charset → chardetng guess.
/// Legacy Arabic (Windows-1256), Hebrew (Windows-1255), Turkish (1254) and
/// Persian files are handled by the detector.
pub fn decode_text(bytes: &[u8], declared: Option<&str>) -> String {
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc.decode_without_bom_handling(&bytes[bom_len..]).0.into_owned();
    }
    if let Some(label) = declared {
        if let Some(enc) = encoding_rs::Encoding::for_label(label.trim().as_bytes()) {
            return enc.decode_without_bom_handling(bytes).0.into_owned();
        }
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    let mut det = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    det.feed(bytes, true);
    let enc = det.guess(None, chardetng::Utf8Detection::Allow);
    enc.decode_without_bom_handling(bytes).0.into_owned()
}

pub fn escape(s: &str) -> String {
    html_escape::encode_text(s).into_owned()
}

pub fn escape_attr(s: &str) -> String {
    html_escape::encode_double_quoted_attribute(s).into_owned()
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && s.is_char_boundary(i + 3) {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolves `rel` against the directory of `base` (both zip-internal paths).
pub fn resolve_path(base: &str, rel: &str) -> String {
    let rel = rel.split('#').next().unwrap_or("");
    let rel = percent_decode(rel);
    if rel.starts_with('/') {
        return normalize(&rel[1..]);
    }
    let dir = match base.rfind('/') {
        Some(i) => &base[..=i],
        None => "",
    };
    normalize(&format!("{dir}{rel}"))
}

pub fn normalize(p: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub fn read_zip_entry<R: std::io::Read + std::io::Seek>(
    z: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>> {
    let real = if z.index_for_name(name).is_some() {
        name.to_owned()
    } else {
        // case-insensitive fallback (some EPUBs have inconsistent casing)
        let lname = name.to_lowercase();
        z.file_names()
            .filter_map(|n| n.ok().map(|c| c.into_owned()))
            .find(|n| n.to_lowercase() == lname)
            .ok_or_else(|| ReaderError::Parse { msg: format!("missing entry {name}") })?
    };
    let mut f = z.by_name(&real)?;
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn zip_entry_string<R: std::io::Read + std::io::Seek>(
    z: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<String> {
    let b = read_zip_entry(z, name)?;
    Ok(decode_text(&b, None))
}

/// Local (namespace-stripped) name of an XML tag as an owned string.
pub fn local<T: AsRef<[u8]> + ?Sized>(name: &T) -> String {
    let name = name.as_ref();
    let s = match name.iter().rposition(|b| *b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    };
    String::from_utf8_lossy(s).into_owned()
}

pub fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        if local(AsRef::<str>::as_ref(&a.key)) == key {
            let raw = AsRef::<str>::as_ref(&a.value).to_owned();
            return Some(unescape_xml(&raw));
        }
    }
    None
}

/// Like [attr] but also matches the full qualified name (e.g. `xlink:href`, `r:id`).
pub fn attr_q(e: &quick_xml::events::BytesStart, qname: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        if AsRef::<str>::as_ref(&a.key) == qname {
            let raw = AsRef::<str>::as_ref(&a.value).to_owned();
            return Some(unescape_xml(&raw));
        }
    }
    None
}

pub fn unescape_xml(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    html_escape::decode_html_entities(s).into_owned()
}

pub fn base64_decode(input: &str) -> Vec<u8> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        if let Some(v) = val(c) {
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
                acc &= (1 << bits) - 1;
            }
        }
    }
    out
}

/// Strips tags/entities to get searchable plain text.
pub fn html_to_text(html: &str) -> String {
    let body = match html.find("<body") {
        Some(i) => &html[i..],
        None => html,
    };
    let mut out = String::with_capacity(body.len() / 2);
    let mut in_tag = false;
    let mut tag = String::new();
    let mut skip_until: Option<&str> = None;
    for c in body.chars() {
        if in_tag {
            if c == '>' {
                in_tag = false;
                let t = tag.to_ascii_lowercase();
                if let Some(end) = skip_until {
                    if t.starts_with(end) {
                        skip_until = None;
                    }
                } else if t.starts_with("script") {
                    skip_until = Some("/script");
                } else if t.starts_with("style") {
                    skip_until = Some("/style");
                } else if t.starts_with("/p") || t.starts_with("br") || t.starts_with("/h") || t.starts_with("/li") || t.starts_with("/div") || t.starts_with("/tr") {
                    out.push('\n');
                }
                tag.clear();
            } else {
                tag.push(c);
            }
        } else if c == '<' {
            in_tag = true;
        } else if skip_until.is_none() {
            out.push(c);
        }
    }
    html_escape::decode_html_entities(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths() {
        assert_eq!(resolve_path("OEBPS/Text/c1.xhtml", "../Images/a%20b.jpg#x"), "OEBPS/Images/a b.jpg");
        assert_eq!(resolve_path("c1.xhtml", "img.png"), "img.png");
    }
    #[test]
    fn b64() {
        assert_eq!(base64_decode("aGVsbG8="), b"hello");
        assert_eq!(base64_decode("aGVs\nbG8h"), b"hello!");
    }
    #[test]
    fn decodes_cp1256() {
        // "سلام" in windows-1256
        let bytes = [0xD3u8, 0xE1, 0xC7, 0xE3, 0x20, 0xD3, 0xE1, 0xC7, 0xE3, 0x20, 0xC8, 0xD1, 0xC7, 0xED];
        let s = decode_text(&bytes, Some("windows-1256"));
        assert!(s.starts_with("سلام"));
    }
}
