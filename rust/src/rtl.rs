//! RTL helpers: paragraph direction detection and search folding for
//! Arabic, Persian, Urdu, Hebrew, Syriac and Thaana text.

use unicode_bidi::{bidi_class, BidiClass};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TextDirection {
    Ltr,
    Rtl,
}

impl TextDirection {
    pub fn as_attr(self) -> &'static str {
        match self {
            TextDirection::Ltr => "ltr",
            TextDirection::Rtl => "rtl",
        }
    }
}

/// Direction of the first strong character (Unicode rule P2/P3).
pub fn first_strong(text: &str) -> Option<TextDirection> {
    for c in text.chars() {
        match bidi_class(c) {
            BidiClass::L => return Some(TextDirection::Ltr),
            BidiClass::R | BidiClass::AL => return Some(TextDirection::Rtl),
            _ => {}
        }
    }
    None
}

/// Dominant direction of a whole text by counting strong characters.
/// Used for documents that carry no explicit direction metadata.
pub fn dominant(text: &str) -> TextDirection {
    let (mut l, mut r) = (0usize, 0usize);
    for c in text.chars().take(20_000) {
        match bidi_class(c) {
            BidiClass::L => l += 1,
            BidiClass::R | BidiClass::AL => r += 1,
            _ => {}
        }
    }
    if r > l / 2 && r > 0 {
        TextDirection::Rtl
    } else {
        TextDirection::Ltr
    }
}

pub fn is_rtl_lang(lang: &str) -> bool {
    let primary = lang
        .split(|c| c == '-' || c == '_')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        primary.as_str(),
        "ar" | "fa" | "he" | "iw" | "ur" | "ps" | "sd" | "ug" | "yi" | "ckb" | "dv" | "syr" | "ku" | "prs" | "ks"
    )
}

pub(crate) fn is_mark_to_strip(c: char) -> bool {
    matches!(c as u32,
        0x0610..=0x061A   // Arabic signs
        | 0x064B..=0x065F // harakat / tashkeel
        | 0x0670          // superscript alef
        | 0x06D6..=0x06ED // Quranic annotation marks
        | 0x0640          // tatweel
        | 0x0591..=0x05BD // Hebrew cantillation + niqqud
        | 0x05BF | 0x05C1 | 0x05C2 | 0x05C4 | 0x05C5 | 0x05C7
        | 0x200B..=0x200F // ZW space, ZWNJ, ZWJ, LRM, RLM
        | 0x202A..=0x202E // bidi embeddings
        | 0x2066..=0x2069 // bidi isolates
        | 0x00AD          // soft hyphen
        | 0xFEFF)
}

/// How aggressively letters are merged when matching.
///
/// `Uyghur` keeps every letter the Uyghur alphabet distinguishes
/// (ى i ≠ ي y, ې é, ە e, ۇ u, ۆ ö, ۈ ü, ۋ w, ڭ ng, ھ h) and only drops the
/// syllable-onset hamza ئ, which is predictable from spelling and often typed
/// inconsistently. `Generic` is tuned for Arabic/Persian/Urdu/Hebrew, where
/// merging alef/ya variants helps users.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldProfile {
    Generic,
    Uyghur,
}

/// Letters that occur in Uyghur Ereb Yéziqi but not in Arabic/Persian.
fn is_uyghur_letter(c: char) -> bool {
    matches!(c, 'ې' | 'ە' | 'ۇ' | 'ۆ' | 'ۈ' | 'ۋ' | 'ڭ')
}

/// Uyghur (or a related Turkic/Kurdish Arabic-script text) if its distinctive
/// letters are a noticeable share of the Arabic-script letters.
pub fn detect_profile(text: &str) -> FoldProfile {
    let (mut arabic, mut uy) = (0usize, 0usize);
    for c in text.chars().take(60_000) {
        let c = std::iter::once(c).nfkc().next().unwrap_or(c);
        if ('\u{0600}'..='\u{06FF}').contains(&c) {
            arabic += 1;
            if is_uyghur_letter(c) {
                uy += 1;
            }
        }
    }
    if uy >= 2 && uy * 25 >= arabic {
        FoldProfile::Uyghur
    } else {
        FoldProfile::Generic
    }
}

/// True when the text looks like Uyghur specifically (é + ng/ü/ö present),
/// used to tag untagged documents with `lang="ug"` for correct shaping.
pub fn looks_uyghur(text: &str) -> bool {
    let mut e = 0;
    let mut other = 0;
    for c in text.chars().take(60_000) {
        let c = std::iter::once(c).nfkc().next().unwrap_or(c);
        match c {
            'ې' => e += 1,
            'ڭ' | 'ۈ' | 'ۆ' => other += 1,
            _ => {}
        }
    }
    e >= 2 && other >= 1 && detect_profile(text) == FoldProfile::Uyghur
}

fn fold_char(c: char, p: FoldProfile) -> Option<char> {
    if p == FoldProfile::Uyghur {
        return Some(match c {
            'ئ' => return None, // onset hamza: ignore on both sides
            'ه' | 'ة' | 'ۀ' => 'ە', // legacy texts type ە as ه
            'ہ' => 'ھ',
            'ی' | 'ۍ' => 'ي',  // Persian-keyboard ye
            'ک' | 'ڪ' => 'ك',
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            '٠'..='٩' => char::from_u32(c as u32 - 0x0660 + 0x30).unwrap_or(c),
            '۰'..='۹' => char::from_u32(c as u32 - 0x06F0 + 0x30).unwrap_or(c),
            _ => c,
        });
    }
    Some(match c {
        // Alef variants -> bare alef
        'أ' | 'إ' | 'آ' | 'ٱ' | 'ٲ' | 'ٳ' => 'ا',
        // Ya / alef maqsura / Persian ye -> Arabic ya
        'ى' | 'ی' | 'ې' | 'ۍ' | 'ے' => 'ي',
        // Persian keheh -> kaf
        'ک' | 'ڪ' => 'ك',
        // Ta marbuta -> ha (common user query habit)
        'ة' => 'ه',
        'ۀ' | 'ہ' | 'ھ' => 'ه',
        'ؤ' => 'و',
        'ئ' => 'ي',
        // Arabic-Indic / Persian digits -> ASCII
        '٠'..='٩' => char::from_u32(c as u32 - 0x0660 + 0x30).unwrap_or(c),
        '۰'..='۹' => char::from_u32(c as u32 - 0x06F0 + 0x30).unwrap_or(c),
        // Hebrew final forms -> regular
        'ך' => 'כ',
        'ם' => 'מ',
        'ן' => 'נ',
        'ף' => 'פ',
        'ץ' => 'צ',
        _ => c,
    })
}

/// Folds a string for matching (generic profile). NFKC turns Arabic
/// presentation forms (U+FB50–U+FDFF, U+FE70–U+FEFF, incl. Uyghur ligatures
/// U+FBEA–U+FBFB, as PDFs often store them) into base letters.
pub fn fold(text: &str) -> String {
    fold_p(text, FoldProfile::Generic)
}

pub fn fold_p(text: &str, p: FoldProfile) -> String {
    let mut out = String::with_capacity(text.len());
    fold_into(text, p, &mut out, |_| {});
    out
}

/// Folds `text` and calls `on_char(source_index)` once for every char pushed,
/// so callers can map folded positions back to source characters.
pub fn fold_into(text: &str, p: FoldProfile, out: &mut String, mut on_char: impl FnMut(usize)) {
    for (i, src) in text.chars().enumerate() {
        for c in std::iter::once(src).nfkc() {
            if is_mark_to_strip(c) {
                continue;
            }
            for lc in c.to_lowercase() {
                let Some(f) = fold_char(lc, p) else { continue };
                let f = if f.is_whitespace() { ' ' } else { f };
                // collapse runs of whitespace
                if f == ' ' && out.ends_with(' ') {
                    continue;
                }
                out.push(f);
                on_char(i);
            }
        }
    }
}

/// Like [fold_p] but returns a per-char map back to the char index of `text`.
pub fn fold_with_map(text: &str, p: FoldProfile) -> (Vec<char>, Vec<usize>) {
    let mut s = String::new();
    let mut map = Vec::new();
    fold_into(text, p, &mut s, |i| map.push(i));
    (s.chars().collect(), map)
}

/// Converts Uyghur Latin script (ULY) to Uyghur Arabic script (UEY), so users
/// can search Arabic-script books by typing Latin. Apostrophes separate
/// ambiguous digraphs (e.g. "nan'gah"). Onset ئ is not emitted: matching
/// ignores it anyway.
pub fn uly_to_uey(text: &str) -> String {
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let mut out = String::with_capacity(text.len() * 2);
    let mut i = 0;
    while i < lower.len() {
        let c = lower[i];
        let next = lower.get(i + 1).copied();
        let two = |a: char, b: char| c == a && next == Some(b);
        let (s, adv) = if two('c', 'h') {
            ("چ", 2)
        } else if two('g', 'h') {
            ("غ", 2)
        } else if two('n', 'g') {
            ("ڭ", 2)
        } else if two('s', 'h') {
            ("ش", 2)
        } else if two('z', 'h') {
            ("ژ", 2)
        } else {
            let s = match c {
                'a' => "ا",
                'e' | 'ə' => "ە",
                'é' | 'ë' | 'ê' => "ې",
                'i' | 'ı' => "ى",
                'o' => "و",
                'ö' | 'ő' => "ۆ",
                'u' => "ۇ",
                'ü' | 'ű' => "ۈ",
                'b' => "ب",
                'p' => "پ",
                't' => "ت",
                'j' => "ج",
                'x' => "خ",
                'd' => "د",
                'r' => "ر",
                'z' => "ز",
                's' => "س",
                'f' => "ف",
                'q' => "ق",
                'k' => "ك",
                'g' => "گ",
                'l' => "ل",
                'm' => "م",
                'n' => "ن",
                'h' => "ھ",
                'w' | 'v' => "ۋ",
                'y' => "ي",
                'c' => "چ",
                '\'' | '’' | 'ʼ' => "",
                '?' => "؟",
                ',' => "،",
                ';' => "؛",
                _ => {
                    out.push(c);
                    i += 1;
                    continue;
                }
            };
            (s, 1)
        };
        out.push_str(s);
        i += adv;
    }
    out
}

fn is_latin_query(q: &str) -> bool {
    q.chars().any(|c| c.is_alphabetic()) && q.chars().all(|c| !c.is_alphabetic() || c.is_ascii_alphabetic() || matches!(c, 'é' | 'ë' | 'ö' | 'ü' | 'É' | 'Ö' | 'Ü' | 'ı' | 'ə'))
}

/// Folded needles to look for: the query itself, plus its Uyghur-Arabic
/// transliteration when the document is Uyghur and the query is Latin.
pub fn needles(query: &str, p: FoldProfile) -> Vec<Vec<char>> {
    let q = query.trim();
    let mut v: Vec<Vec<char>> = Vec::new();
    let a: Vec<char> = fold_p(q, p).trim().chars().collect();
    if !a.is_empty() {
        v.push(a);
    }
    if p == FoldProfile::Uyghur && is_latin_query(q) {
        let b: Vec<char> = fold_p(&uly_to_uey(q), p).trim().chars().collect();
        if !b.is_empty() && !v.contains(&b) {
            v.push(b);
        }
    }
    v
}

/// Profile for searching `hay` with `query` (query letters can force Uyghur).
pub fn profile_for(hay: &str, query: &str) -> FoldProfile {
    if query.chars().any(is_uyghur_letter) {
        FoldProfile::Uyghur
    } else {
        detect_profile(hay)
    }
}

/// All matches of any needle as (start, len) in folded-char space, sorted.
pub fn find_any(hay: &[char], needles: &[Vec<char>]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = needles.iter().flat_map(|n| find_all(hay, n).into_iter().map(move |s| (s, n.len()))).collect();
    out.sort();
    out.dedup_by_key(|x| x.0);
    out
}

/// Returns char-index ranges (in `hay`'s char space) of all matches of `needle`.
pub fn find_all(hay_folded: &[char], needle_folded: &[char]) -> Vec<usize> {
    let n = needle_folded.len();
    if n == 0 || hay_folded.len() < n {
        return vec![];
    }
    let mut hits = Vec::new();
    let mut i = 0;
    while i + n <= hay_folded.len() {
        if hay_folded[i..i + n] == *needle_folded {
            hits.push(i);
            i += n;
        } else {
            i += 1;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_direction() {
        assert_eq!(first_strong("123 مرحبا"), Some(TextDirection::Rtl));
        assert_eq!(first_strong("  Hello עולם"), Some(TextDirection::Ltr));
        assert_eq!(dominant("שלום עולם hello"), TextDirection::Rtl);
        assert!(is_rtl_lang("fa-IR"));
        assert!(!is_rtl_lang("tr"));
    }

    #[test]
    fn folds_arabic_presentation_forms_and_tashkeel() {
        // "ﻣﺮﺣﺒﺎ" written in presentation forms equals "مرحبا"
        assert_eq!(fold("ﻣﺮﺣﺒﺎ"), fold("مرحبا"));
        assert_eq!(fold("الْعَرَبِيَّة"), fold("العربيه"));
        assert_eq!(fold("أحمد"), fold("احمد"));
        assert_eq!(fold("کتاب"), fold("كتاب"));
        assert_eq!(fold("שָׁלוֹם"), fold("שלום"));
        assert_eq!(fold("١٢٣"), "123");
    }

    #[test]
    fn maps_back_to_source() {
        let (f, map) = fold_with_map("الْكِتَاب", FoldProfile::Generic);
        let needle: Vec<char> = fold("كتاب").chars().collect();
        let hits = find_all(&f, &needle);
        assert_eq!(hits.len(), 1);
        let start_src = map[hits[0]];
        assert_eq!(start_src, 3); // 'ك' after "الْ"
    }

    const UY: &str = "ئۇيغۇر تىلى — بۇ كىتاب ناھايىتى قىزىقارلىق. مەن ئۈرۈمچىدە ئوقۇيمەن، سېنىڭ ئىسمىڭ نېمە؟";

    #[test]
    fn detects_uyghur() {
        assert_eq!(detect_profile(UY), FoldProfile::Uyghur);
        assert!(looks_uyghur(UY));
        assert_eq!(detect_profile("هذا كتاب عربي جميل جدا"), FoldProfile::Generic);
        assert!(!looks_uyghur("این یک متن فارسی است"));
    }

    #[test]
    fn uyghur_keeps_distinct_letters() {
        let p = FoldProfile::Uyghur;
        // ى (i) and ي (y) are different Uyghur letters
        assert_ne!(fold_p("تىل", p), fold_p("تيل", p));
        // ې (é) is not ي
        assert_ne!(fold_p("سېنى", p), fold_p("سينى", p));
        // ۇ ۆ ۈ are three vowels
        assert_ne!(fold_p("ۇ", p), fold_p("ۈ", p));
        // onset hamza is ignored; legacy ه == ە
        assert_eq!(fold_p("ئۈرۈمچى", p), fold_p("ۈرۈمچى", p));
        assert_eq!(fold_p("مەن", p), fold_p("مهن", p));
        // Uyghur presentation-form ligature (U+FBEA..) folds to letters
        assert_eq!(fold_p("\u{FBEB}", p), fold_p("ئا", p));
    }

    #[test]
    fn latin_query_finds_arabic_script() {
        assert_eq!(uly_to_uey("Uyghur"), "ۇيغۇر");
        assert_eq!(uly_to_uey("kitab"), "كىتاب");
        assert_eq!(uly_to_uey("sëning"), "سېنىڭ");
        let p = detect_profile(UY);
        let hay: Vec<char> = fold_p(UY, p).chars().collect();
        for q in ["kitab", "Ürümchide", "séning", "qiziqarliq", "ئۇيغۇر"] {
            let hits = find_any(&hay, &needles(q, p));
            assert_eq!(hits.len(), 1, "query {q}");
        }
        // "مەن" (I) also occurs inside "ئوقۇيمەن"
        assert_eq!(find_any(&hay, &needles("مەن", p)).len(), 2);
        // "تىل" (til) must not match "تيل"
        assert!(find_any(&hay, &needles("تيل", p)).is_empty());
    }
}
