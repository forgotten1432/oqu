//! Rebuilds logical-order text from PDF glyph geometry.
//!
//! Many PDF producers (browsers, Word "Save as PDF", scanners' OCR layers)
//! emit Arabic/Hebrew as positioned glyphs whose extraction order is visual,
//! partially reversed, or scrambled by separately placed diacritics. Instead of
//! trusting the content-stream order, we:
//!   1. group chars that share one glyph box (ligatures such as lam-alef),
//!   2. cluster glyphs into visual lines,
//!   3. sort each line by x and apply the bidi rule per line (RTL lines are
//!      read right-to-left, embedded LTR/number runs keep their order),
//!   4. re-attach combining marks (harakat, niqqud) to their base glyph,
//!   5. synthesize spaces from horizontal gaps.
//! Every output char keeps its box, so search hits map back to rectangles.

use crate::pdf::NormRect;
use unicode_bidi::{bidi_class, BidiClass};

#[derive(Clone)]
struct Cluster {
    chars: Vec<char>,
    b: NormRect,
    marks: Vec<char>,
}

impl Cluster {
    fn cx(&self) -> f32 {
        (self.b.left + self.b.right) / 2.0
    }
    fn cy(&self) -> f32 {
        (self.b.top + self.b.bottom) / 2.0
    }
    fn h(&self) -> f32 {
        (self.b.bottom - self.b.top).max(1e-4)
    }
    fn class(&self) -> BidiClass {
        self.chars.first().map(|c| bidi_class(*c)).unwrap_or(BidiClass::ON)
    }
}

fn is_mark(c: char) -> bool {
    bidi_class(c) == BidiClass::NSM
}

fn same_box(a: &NormRect, b: &NormRect) -> bool {
    (a.left - b.left).abs() < 1e-5 && (a.right - b.right).abs() < 1e-5 && (a.top - b.top).abs() < 1e-5 && (a.bottom - b.bottom).abs() < 1e-5
}

pub fn reconstruct(chars: &[char], boxes: &[Option<NormRect>]) -> (Vec<char>, Vec<Option<NormRect>>) {
    // 1. clusters + loose marks
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut marks: Vec<(char, NormRect)> = Vec::new();
    let mut prev_box: Option<NormRect> = None;
    for (c, b) in chars.iter().zip(boxes.iter()) {
        let Some(b) = b else {
            prev_box = None;
            continue;
        };
        if c.is_control() {
            prev_box = None;
            continue;
        }
        if c.is_whitespace() {
            clusters.push(Cluster { chars: vec![' '], b: b.clone(), marks: vec![] });
            prev_box = None;
            continue;
        }
        if is_mark(*c) {
            marks.push((*c, b.clone()));
            continue;
        }
        match (&prev_box, clusters.last_mut()) {
            (Some(p), Some(last)) if same_box(p, b) => last.chars.push(*c),
            _ => clusters.push(Cluster { chars: vec![*c], b: b.clone(), marks: vec![] }),
        }
        prev_box = Some(b.clone());
    }
    // lam-alef ligature: always lam first
    for cl in clusters.iter_mut() {
        if cl.chars.len() == 2 && is_alef(cl.chars[0]) && cl.chars[1] == 'ل' {
            cl.chars.swap(0, 1);
        }
    }
    if clusters.is_empty() {
        return (vec![], vec![]);
    }

    // 2. lines, top to bottom. Build bands from normal-height glyphs first;
    //    undersized boxes (glyphs whose box collapsed onto a diacritic) join
    //    the nearest band afterwards.
    let mut hs: Vec<f32> = clusters.iter().filter(|c| c.chars[0] != ' ').map(|c| c.h()).collect();
    if hs.is_empty() {
        return (vec![], vec![]);
    }
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_h = hs[hs.len() / 2];
    let mut order: Vec<usize> = (0..clusters.len()).collect();
    order.sort_by(|&a, &b| clusters[a].cy().partial_cmp(&clusters[b].cy()).unwrap_or(std::cmp::Ordering::Equal));
    let mut lines: Vec<(f32, f32, Vec<usize>)> = Vec::new(); // top, bottom, members
    let mut minor = Vec::new();
    for i in order {
        let c = &clusters[i];
        if c.h() < 0.6 * median_h {
            minor.push(i);
            continue;
        }
        let cy = c.cy();
        if let Some(line) = lines.iter_mut().rev().take(3).find(|(t, b, _)| cy > *t && cy < *b) {
            line.0 = line.0.min(c.b.top);
            line.1 = line.1.max(c.b.bottom);
            line.2.push(i);
        } else {
            lines.push((c.b.top, c.b.bottom, vec![i]));
        }
    }
    for i in minor {
        let cy = clusters[i].cy();
        let best = lines
            .iter_mut()
            .map(|l| {
                let d = if cy < l.0 { l.0 - cy } else if cy > l.1 { cy - l.1 } else { 0.0 };
                (d, l)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        match best {
            Some((d, l)) if d < median_h => l.2.push(i),
            _ => {
                let c = &clusters[i];
                lines.push((c.b.top, c.b.bottom, vec![i]));
            }
        }
    }
    lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // 4. attach marks to the base glyph with the best horizontal overlap
    for (m, mb) in marks {
        let mcx = (mb.left + mb.right) / 2.0;
        let mcy = (mb.top + mb.bottom) / 2.0;
        let above = matches!(m as u32, 0x064B | 0x064C | 0x064E | 0x064F | 0x0651 | 0x0652 | 0x0653 | 0x0654 | 0x0670 | 0x05B9 | 0x05C1 | 0x05C2);
        let below = matches!(m as u32, 0x064D | 0x0650 | 0x0655 | 0x05B0..=0x05B8 | 0x05BB);
        let mut best: Option<(usize, f32)> = None;
        for (t, b, members) in &lines {
            let h = b - t;
            if above && *b < mcy - 0.1 * h {
                continue; // line is entirely above the mark: an above-mark belongs to a line below it
            }
            if below && *t > mcy + 0.1 * h {
                continue;
            }
            let dv = if mcy < *t { t - mcy } else if mcy > *b { mcy - b } else { 0.0 };
            if dv > h {
                continue;
            }
            for &i in members {
                let c = &clusters[i];
                if c.chars[0] == ' ' {
                    continue;
                }
                let dh = if mcx >= c.b.left && mcx <= c.b.right { 0.0 } else { (mcx - c.cx()).abs() };
                let d = dv + dh;
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((i, d));
                }
            }
        }
        if let Some((i, _)) = best {
            clusters[i].marks.push(m);
        }
    }

    // 3 + 5. per-line visual sort, bidi, gaps
    let mut out_c = Vec::new();
    let mut out_b = Vec::new();
    for (li, (_, _, members)) in lines.iter().enumerate() {
        let mut row: Vec<usize> = members.clone();
        row.sort_by(|&a, &b| clusters[a].cx().partial_cmp(&clusters[b].cx()).unwrap_or(std::cmp::Ordering::Equal));
        let (mut r, mut l) = (0, 0);
        for &i in &row {
            match clusters[i].class() {
                BidiClass::R | BidiClass::AL => r += 1,
                BidiClass::L => l += 1,
                _ => {}
            }
        }
        let rtl_line = r > l;
        // visual items with synthesized spaces (None = space)
        let mut items: Vec<Option<usize>> = Vec::new();
        for (k, &i) in row.iter().enumerate() {
            if k > 0 {
                let p = &clusters[row[k - 1]];
                let c = &clusters[i];
                let gap = c.b.left - p.b.right;
                let has_space = p.chars[0] == ' ' || c.chars[0] == ' ';
                if !has_space && gap > 0.45 * median_h {
                    items.push(None);
                }
            }
            items.push(Some(i));
        }
        if rtl_line {
            items.reverse();
            fix_runs(&mut items, &clusters, |cl| matches!(cl, BidiClass::L | BidiClass::EN | BidiClass::AN));
        } else {
            fix_runs(&mut items, &clusters, |cl| matches!(cl, BidiClass::R | BidiClass::AL));
        }
        if li > 0 {
            out_c.push('\n');
            out_b.push(None);
        }
        for it in items {
            match it {
                None => {
                    if out_c.last().map(|c| *c != ' ').unwrap_or(false) {
                        out_c.push(' ');
                        out_b.push(None);
                    }
                }
                Some(i) if clusters[i].chars[0] == ' ' => {
                    if out_c.last().map(|c| *c != ' ' && *c != '\n').unwrap_or(false) {
                        out_c.push(' ');
                        out_b.push(None);
                    }
                }
                Some(i) => {
                    let c = &clusters[i];
                    for ch in c.chars.iter().chain(c.marks.iter()) {
                        out_c.push(*ch);
                        out_b.push(Some(c.b.clone()));
                    }
                }
            }
        }
    }
    (out_c, out_b)
}

fn is_alef(c: char) -> bool {
    matches!(c, 'ا' | 'أ' | 'إ' | 'آ')
}

/// Reverses maximal runs of "opposite direction" items (allowing spaces and
/// number separators inside a run) so that they read in their own order.
fn fix_runs(items: &mut [Option<usize>], clusters: &[Cluster], opposite: impl Fn(BidiClass) -> bool) {
    let is_opp = |it: &Option<usize>| it.map(|i| opposite(clusters[i].class())).unwrap_or(false);
    let is_joiner = |it: &Option<usize>| match it {
        None => true,
        Some(i) => matches!(clusters[*i].class(), BidiClass::CS | BidiClass::ES | BidiClass::ET | BidiClass::WS),
    };
    let n = items.len();
    let mut i = 0;
    while i < n {
        if !is_opp(&items[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i; // last opposite index
        let mut j = i + 1;
        while j < n {
            if is_opp(&items[j]) {
                end = j;
                j += 1;
            } else if is_joiner(&items[j]) {
                j += 1;
            } else {
                break;
            }
        }
        items[start..=end].reverse();
        i = end + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bx(x: f32, y: f32) -> Option<NormRect> {
        Some(NormRect { left: x, right: x + 0.01, top: y, bottom: y + 0.02 })
    }
    #[test]
    fn visual_rtl_line_becomes_logical() {
        // "نص 12" drawn visually: "12" at left, then space, then ص, ن at right.
        let chars = ['1', '2', 'ص', 'ن'];
        let boxes = [bx(0.10, 0.1), bx(0.11, 0.1), bx(0.20, 0.1), bx(0.21, 0.1)];
        let (c, _) = reconstruct(&chars, &boxes);
        let s: String = c.iter().collect();
        assert_eq!(s, "نص 12");
    }
    #[test]
    fn mark_attaches_to_base() {
        // ب with fatha above, laid out as one letter.
        let chars = ['َ', 'ب'];
        let boxes = [Some(NormRect { left: 0.5, right: 0.505, top: 0.08, bottom: 0.09 }), bx(0.497, 0.1)];
        let (c, _) = reconstruct(&chars, &boxes);
        assert_eq!(c.iter().collect::<String>(), "بَ");
    }
}
