//! Roman Hanging Punctuation and Optical Margin Alignment in layout (helpx "Formatting
//! paragraphs" › Hanging punctuation, <https://helpx.adobe.com/illustrator/using/formatting-paragraphs.html>).

use super::*;
use crate::hang::{Edge, ratio};
use kurbo::{Affine, Shape};
use vectorcraft_doc::{CharStyle, Composer, Justify, TextKind};
use vectorcraft_geom::PathData;

fn db() -> &'static FontDb {
    FontDb::global()
}

fn area(text: &str, size: f64, frame: Rect, justify: Justify) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, CharStyle { size, ..CharStyle::default() });
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&frame.to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    t.para.justify = justify;
    t
}

/// The character a glyph sets (an automatic hyphen has no source characters: `-`).
fn ch(t: &TextObject, g: &PositionedGlyph) -> char {
    if g.len == 0 { '-' } else { t.plain_text().get(g.byte..).and_then(|s| s.chars().next()).unwrap_or('\0') }
}

fn is_space(c: char) -> bool {
    c == ' ' || c == '\n'
}

/// Per line: (first character, how far it starts before the line's start edge, last character
/// or hyphen, how far it ends after the line's end edge), each distance less what the character's
/// hang ratio of its advance says (0 when it hangs exactly as the table says).
fn edges(t: &TextObject, l: &TextLayout, roman: bool, optical: bool) -> Vec<(char, f64, char, f64)> {
    l.lines
        .iter()
        .filter_map(|li| {
            let gs = l.glyphs.get(li.glyph_start..li.glyph_end)?;
            let first = gs.iter().find(|g| !is_space(ch(t, g)))?;
            let last = gs.iter().rev().find(|g| !is_space(ch(t, g)))?;
            let (c0, c1) = (ch(t, first), ch(t, last));
            Some((
                c0,
                li.avail.0 - first.origin.x - ratio(c0, Edge::Start, roman, optical) * first.advance,
                c1,
                last.origin.x + last.advance - li.avail.1 - ratio(c1, Edge::End, roman, optical) * last.advance,
            ))
        })
        .collect()
}

const TEXT: &str = "\u{201C}Hanging\u{201D} quotes, commas. And periods. \"Straight\" quotes and well-known \
hyphens - dashes \u{2014} colons: semicolons; and \u{2018}single\u{2019} ones. Extraordinarily comprehensive \
typographical considerations notwithstanding. \u{201C}Again\u{201D}, then more words follow here.";

/// Quotes, hyphens, full stops and commas hang wholly outside both edges, the half-hanging marks
/// half, everything else not at all: ragged and justified, with both composers. Justified lines
/// fill to the hung edges, and ragged lines never pass them.
#[test]
fn roman_hanging_punctuation_hangs_quotes_hyphens_and_periods_at_both_edges() {
    let frame = Rect::new(0.0, 0.0, 150.0, 4000.0);
    let (mut quote, mut hyphen, mut period) = (false, false, false);
    for justify in [Justify::Left, Justify::Right, Justify::JustifyLeft] {
        for composer in [Composer::SingleLine, Composer::EveryLine] {
            let mut t = area(TEXT, 12.0, frame, justify);
            t.para.hyphenate = true;
            t.para.composer = composer;
            let off = layout(db(), &t);
            // Off: every line starts at the frame's edge.
            if justify != Justify::Right {
                for (_, before, ..) in edges(&t, &off, false, false) {
                    assert!(before.abs() < 1e-6, "{justify:?} {composer:?}: {before}");
                }
            }
            t.para.roman_hanging = true;
            let on = layout(db(), &t);
            assert!(!on.overflow);
            let n = on.lines.len();
            for (k, (c0, before, c1, after)) in edges(&t, &on, true, false).into_iter().enumerate() {
                if justify != Justify::Right {
                    assert!(before.abs() < 1e-6, "{justify:?} {composer:?} line {k} starts {c0:?}: {before}");
                    quote |= matches!(c0, '\u{201C}' | '"' | '\u{2018}');
                }
                let full = justify == Justify::Right || (justify == Justify::JustifyLeft && k + 1 < n);
                if full {
                    assert!(after.abs() < 1e-6, "{justify:?} {composer:?} line {k} ends {c1:?}: {after}");
                    hyphen |= c1 == '-';
                    period |= matches!(c1, '.' | ',');
                } else {
                    assert!(after <= 1e-6, "{justify:?} {composer:?} line {k} passes the hung edge: {after}");
                }
            }
        }
    }
    assert!(quote && hyphen && period, "quote {quote}, hyphen {hyphen}, period {period}");
}

/// The hung amounts in numbers: an opening quote at the start of a left-aligned line stands its
/// whole advance outside it, a period, hyphen or comma at the end of a right-aligned line too, a
/// colon half; in area type (frame 0–400) and point type (origin 0) alike.
#[test]
fn hung_amounts_of_a_quote_a_period_and_a_colon() {
    for point in [false, true] {
        let make = |text: &str, justify| {
            let mut t = if point {
                let mut t = TextObject::point(Point::ZERO, text, CharStyle { size: 20.0, ..CharStyle::default() });
                t.para.justify = justify;
                t
            } else {
                area(text, 20.0, Rect::new(0.0, 0.0, 400.0, 100.0), justify)
            };
            t.para.roman_hanging = true;
            t
        };
        let l = layout(db(), &make("\u{201C}Word", Justify::Left));
        let q = &l.glyphs[0];
        assert!(q.advance > 0.0);
        assert!((q.origin.x + q.advance).abs() < 1e-6, "point {point}: {:?} {}", q.origin, q.advance);
        // The W starts on the edge.
        assert!(l.glyphs[1].origin.x.abs() < 1e-6);
        for (text, r) in [("Word.", 1.0), ("Word:", 0.5), ("Word-", 1.0), ("Word,", 1.0), ("Word\u{2019}", 1.0), ("Word*", 0.5)] {
            let l = layout(db(), &make(text, Justify::Right));
            let g = l.glyphs.last().unwrap();
            let right = if point { 0.0 } else { 400.0 };
            assert!((g.origin.x + g.advance - (right + r * g.advance)).abs() < 1e-6, "point {point} {text}: {:?}", g.origin);
        }
    }
}

/// Optical Margin Alignment is the story's: punctuation hangs, and the edges of letters such as W
/// and A stand slightly outside; letters with a straight side don't move.
#[test]
fn optical_margin_alignment_hangs_letter_edges_and_punctuation() {
    let frame = Rect::new(0.0, 0.0, 400.0, 200.0);
    let mut t = area("Wave\nHill\n\u{201C}Quote\u{201D}", 20.0, frame, Justify::Left);
    t.optical_margin = true;
    let l = layout(db(), &t);
    let w = &l.glyphs[0];
    assert!((w.origin.x + 0.1 * w.advance).abs() < 1e-6, "W hangs a tenth: {:?}", w.origin);
    let starts: Vec<f64> = l.lines.iter().map(|li| l.glyphs[li.glyph_start].origin.x).collect();
    assert_eq!(starts[1], 0.0, "H keeps to the edge");
    assert!((starts[2] + l.glyphs[l.lines[2].glyph_start].advance).abs() < 1e-6);
    for (_, before, _, _) in edges(&t, &l, false, true) {
        assert!(before.abs() < 1e-6);
    }
    // Right edge: a right-aligned A ends a tenth of its advance outside.
    let mut t = area("ABBA", 20.0, frame, Justify::Right);
    t.optical_margin = true;
    let l = layout(db(), &t);
    let a = l.glyphs.last().unwrap();
    assert!((a.origin.x + a.advance - 400.0 - 0.1 * a.advance).abs() < 1e-6);
    // Right-to-left paragraphs don't hang.
    let mut t = area("\u{05E9}\u{05DC}\u{05D5}\u{05DD}.", 20.0, frame, Justify::Right);
    let before = layout(db(), &t);
    t.optical_margin = true;
    t.para.roman_hanging = true;
    let after = layout(db(), &t);
    assert_eq!(before.glyphs.iter().map(|g| g.origin).collect::<Vec<_>>(), after.glyphs.iter().map(|g| g.origin).collect::<Vec<_>>());
}

/// Justified lines fill to the hung edges: every line of Justify All Lines starts at the left
/// edge less what its first character hangs and ends at the right edge plus what its last one
/// hangs, with both composers.
#[test]
fn justified_lines_fill_to_the_hung_edges() {
    let frame = Rect::new(0.0, 0.0, 170.0, 4000.0);
    for composer in [Composer::SingleLine, Composer::EveryLine] {
        let mut t = area(TEXT, 11.0, frame, Justify::JustifyAll);
        t.para.composer = composer;
        t.para.hyphenate = true;
        t.optical_margin = true;
        let l = layout(db(), &t);
        let mut hung = 0;
        for (c0, before, c1, after) in edges(&t, &l, false, true) {
            assert!(before.abs() < 1e-6 && after.abs() < 1e-6, "{composer:?}: {c0:?} {before} / {c1:?} {after}");
            hung += usize::from(ratio(c0, Edge::Start, false, true) > 0.0) + usize::from(ratio(c1, Edge::End, false, true) > 0.0);
        }
        assert!(hung >= 3, "{composer:?}: {hung}");
    }
}

/// Golden: a text object saved before the options (no `opticalMargin`, no `roman_hanging`) loads
/// with both off, saves without them, and lays out exactly as with both off: the opening quote
/// starts at the frame's edge.
#[test]
fn old_files_lay_out_unchanged() {
    let t = area(TEXT, 12.0, Rect::new(0.0, 0.0, 150.0, 4000.0), Justify::JustifyLeft);
    let json = serde_json::to_value(&t).unwrap();
    assert!(json.get("opticalMargin").is_none());
    assert!(json["para"].get("roman_hanging").is_none());
    let old: TextObject = serde_json::from_value(json).unwrap();
    assert!(!old.optical_margin && !old.para.roman_hanging);
    let a = layout(db(), &old);
    let b = layout(db(), &t);
    assert_eq!(a.glyphs.len(), b.glyphs.len());
    for (x, y) in a.glyphs.iter().zip(&b.glyphs) {
        assert_eq!((x.origin, x.advance, x.line), (y.origin, y.advance, y.line));
    }
    assert_eq!(a.glyphs[0].origin.x, 0.0);
    // Set, both are saved and read back.
    let mut on = t.clone();
    on.optical_margin = true;
    on.para.roman_hanging = true;
    let json = serde_json::to_value(&on).unwrap();
    assert_eq!(json["opticalMargin"], true);
    assert_eq!(json["para"]["roman_hanging"], true);
    let back: TextObject = serde_json::from_value(json).unwrap();
    assert!(back.optical_margin && back.para.roman_hanging);
}

/// The non-space glyphs of each line.
fn inked<'a>(t: &TextObject, l: &'a TextLayout) -> Vec<(&'a LineInfo, Vec<&'a PositionedGlyph>)> {
    l.lines
        .iter()
        .map(|li| (li, l.glyphs.get(li.glyph_start..li.glyph_end).unwrap_or_default().iter().filter(|g| !is_space(ch(t, g))).collect()))
        .collect()
}

/// A right-to-left run inside a left-to-right paragraph: its characters (and the full stops the
/// bidi algorithm puts inside it) don't hang, since the reordering may set them away from the
/// edge; nothing passes the frame but what hangs at the line's visual edge.
#[test]
fn right_to_left_runs_in_a_left_to_right_paragraph_dont_hang() {
    let shalom = "\u{05E9}\u{05DC}\u{05D5}\u{05DD}";
    let text = format!("Go {shalom} {shalom} {shalom}. {shalom} {shalom}. abc def");
    for composer in [Composer::SingleLine, Composer::EveryLine] {
        let mut t = area(&text, 12.0, Rect::new(0.0, 0.0, 56.0, 400.0), Justify::JustifyLeft);
        t.para.composer = composer;
        t.para.roman_hanging = true;
        let l = layout(db(), &t);
        assert!(!l.overflow);
        assert!(l.glyphs.iter().any(|g| g.rtl));
        for (li, gs) in inked(&t, &l) {
            let leftmost = gs.iter().copied().min_by(|a, b| a.origin.x.total_cmp(&b.origin.x));
            let rightmost = gs.iter().copied().max_by(|a, b| a.origin.x.total_cmp(&b.origin.x));
            for g in &gs {
                let c = ch(&t, g);
                // Only a left-to-right character at the line's visual start or end may hang.
                let lo = if !g.rtl && leftmost.is_some_and(|f| std::ptr::eq(f, *g)) { ratio(c, Edge::Start, true, false) * g.advance } else { 0.0 };
                let hi = if !g.rtl && rightmost.is_some_and(|f| std::ptr::eq(f, *g)) { ratio(c, Edge::End, true, false) * g.advance } else { 0.0 };
                assert!(g.origin.x >= li.avail.0 - lo - 1e-6, "{composer:?}: {c:?} starts at {} before {}", g.origin.x, li.avail.0);
                assert!(
                    g.origin.x + g.advance <= li.avail.1 + hi + 1e-6,
                    "{composer:?}: {c:?} ends at {} past {}",
                    g.origin.x + g.advance,
                    li.avail.1
                );
            }
        }
    }
}

/// Centred lines centre what doesn't hang: the hung characters stand outside it at either end.
#[test]
fn centred_lines_centre_the_text_between_the_hung_characters() {
    let mut t = area(TEXT, 12.0, Rect::new(0.0, 0.0, 150.0, 4000.0), Justify::Center);
    t.para.roman_hanging = true;
    let l = layout(db(), &t);
    let mut hung = 0;
    for (k, (li, gs)) in inked(&t, &l).into_iter().enumerate() {
        let (Some(first), Some(last)) = (gs.first(), gs.last()) else { continue };
        let (r0, r1) = (ratio(ch(&t, first), Edge::Start, true, false), ratio(ch(&t, last), Edge::End, true, false));
        hung += usize::from(r0 > 0.0) + usize::from(r1 > 0.0);
        let body_start = first.origin.x + r0 * first.advance;
        let body_end = last.origin.x + last.advance - r1 * last.advance;
        assert!(((body_start - li.avail.0) - (li.avail.1 - body_end)).abs() < 1e-6, "line {k}: {body_start} … {body_end} in {:?}", li.avail);
    }
    assert!(hung >= 2, "{hung}");
}

/// Vertical type doesn't hang: both options leave its layout as it was, point and area type.
#[test]
fn vertical_type_doesnt_hang() {
    for frame in [None, Some(Rect::new(0.0, 0.0, 200.0, 100.0))] {
        let mut t =
            TextObject::point(Point::ZERO, "\u{201C}Quote,\u{201D} said W. \u{201C}Again.\u{201D}", CharStyle { size: 20.0, ..CharStyle::default() });
        t.xf = Affine::IDENTITY;
        t.vertical = true;
        t.para.justify = Justify::JustifyAll;
        if let Some(frame) = frame {
            t.kind = TextKind::Area { frame: PathData::from_bezpath(&frame.to_path(0.1)) };
        }
        let before = layout(db(), &t);
        t.para.roman_hanging = true;
        t.optical_margin = true;
        let after = layout(db(), &t);
        let at = |l: &TextLayout| l.glyphs.iter().map(|g| (g.origin, g.advance)).collect::<Vec<_>>();
        assert_eq!(at(&before), at(&after), "area {}", frame.is_some());
    }
}

/// Glyph Scaling and Letter Spacing of justified lines leave the hung first and last characters
/// as they are, so they hang exactly as far as the table says and the text beside them stands on
/// the edges.
#[test]
fn glyph_scaling_and_letter_spacing_keep_the_hung_edges() {
    use vectorcraft_doc::{Justification, SpacingRange};
    for composer in [Composer::SingleLine, Composer::EveryLine] {
        let mut t = area(TEXT, 12.0, Rect::new(0.0, 0.0, 160.0, 4000.0), Justify::JustifyAll);
        t.para.composer = composer;
        t.para.roman_hanging = true;
        t.para.justification = Justification {
            letter_spacing: SpacingRange::new(-5.0, 0.0, 20.0),
            glyph_scaling: SpacingRange::new(95.0, 100.0, 105.0),
            ..Justification::default()
        };
        let l = layout(db(), &t);
        let mut hung = 0;
        for (c0, before, c1, after) in edges(&t, &l, true, false) {
            assert!(before.abs() < 1e-6 && after.abs() < 1e-6, "{composer:?}: {c0:?} {before} / {c1:?} {after}");
            hung += usize::from(ratio(c0, Edge::Start, true, false) > 0.0) + usize::from(ratio(c1, Edge::End, true, false) > 0.0);
        }
        assert!(hung >= 3, "{composer:?}: {hung}");
        // The character after a mark hung wholly outside the start stands on the edge.
        for (li, gs) in inked(&t, &l) {
            if let [first, second, ..] = gs[..]
                && ratio(ch(&t, first), Edge::Start, true, false) == 1.0
            {
                assert!((second.origin.x - li.avail.0).abs() < 1e-6, "{composer:?}: {:?} at {}", ch(&t, second), second.origin.x);
            }
        }
    }
}

/// Standard burasagari with a hung opening quote: the quote's hang is room on the line, so a
/// comma that fits once the quote hangs stays inside (ends on the edge) instead of hanging.
/// Needs a font with full-width Japanese punctuation (skipped without one).
#[test]
fn burasagari_counts_the_hung_start() {
    use vectorcraft_doc::Burasagari;
    let lay = |text: &str, roman: bool| {
        let mut t = area(text, 20.0, Rect::new(0.0, 0.0, 100.0, 200.0), Justify::JustifyLeft);
        t.para.burasagari = Burasagari::Standard;
        t.para.roman_hanging = roman;
        layout(db(), &t)
    };
    if (lay("\u{4E00}\u{3001}\u{4E8C}", false).glyphs[1].advance - 20.0).abs() > 2.0 {
        return; // no font with full-width punctuation here
    }
    let l = lay("\u{201C}\u{4E00}\u{4E8C}\u{4E09}\u{56DB}\u{3001}\u{4E94}\u{516D}\u{4E03}", true);
    let (quote, comma) = (&l.glyphs[0], &l.glyphs[5]);
    assert_eq!(comma.line, quote.line, "、 ends the first line");
    assert!((quote.origin.x + quote.advance).abs() < 1e-6, "the quote hangs: {:?}", quote.origin);
    assert!((comma.origin.x + comma.advance - 100.0).abs() < 1e-6, "、 inside, on the edge: {:?} {}", comma.origin, comma.advance);
}

/// Line-end Punctuation Half Width with Roman Hanging Punctuation: both composers break and place
/// alike (a bracket set flush at a line's start, quotes hanging), every line on its hung edges.
#[test]
fn mojikumi_and_roman_hanging_agree() {
    use vectorcraft_doc::Mojikumi;
    let text = format!("{TEXT} \u{300C}\u{4E00}\u{4E8C}\u{300D} \u{201C}words\u{201D} {TEXT}");
    for composer in [Composer::SingleLine, Composer::EveryLine] {
        let mut t = area(&text, 12.0, Rect::new(0.0, 0.0, 150.0, 4000.0), Justify::JustifyLeft);
        t.para.composer = composer;
        t.para.mojikumi = Mojikumi::LineEndHalf;
        t.para.hyphenate = true;
        t.para.roman_hanging = true;
        let l = layout(db(), &t);
        assert!(!l.overflow);
        let n = l.lines.len();
        for (k, (c0, before, c1, after)) in edges(&t, &l, true, false).into_iter().enumerate() {
            if c0 != '\u{300C}' {
                assert!(before.abs() < 1e-6, "{composer:?} line {k} starts {c0:?}: {before}");
            }
            if k + 1 < n && !matches!(c1, '\u{300D}' | '\u{3001}' | '\u{3002}') {
                assert!(
                    after.abs() < 1e-6,
                    "{composer:?} line {k} ends {c1:?}: {after} {:?}",
                    l.lines.iter().map(|li| &text[li.start..li.end]).collect::<Vec<_>>()
                );
            }
        }
    }
}
