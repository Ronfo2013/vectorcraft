//! Paragraph panel menu › Justification… in composition: the Desired word spacing, letter spacing
//! and glyph scaling set every paragraph's spacing, and justified lines take up their slack
//! within Minimum and Maximum (helpx, "Adjust word and letterspacing in justified text":
//! <https://helpx.adobe.com/illustrator/desktop/design-with-text/edit-format-text/adjust-word-and-letterspacing-in-justified-text.html>).
//!
//! A justified line is filled in this order: word spacing up to its limit, then letter spacing,
//! then glyph scaling; whatever is left goes to the word spaces past their limit (a line with no
//! word space follows Single Word Justification). Shrinking takes the same steps the other way.

use vectorcraft_doc::{Justification, SingleWordJustify, SpacingRange};

use crate::composer::Glue;
use crate::shape::SGlyph;

const EPS: f64 = 1e-9;

/// A word space: the glyph whose width Word Spacing sets.
pub(crate) fn is_word_space(g: &SGlyph) -> bool {
    g.inline.is_none() && matches!(g.ch, ' ' | '\u{00A0}')
}

/// A glyph Letter Spacing and Glyph Scaling act on: not a space, tab, control character or inline
/// graphic.
pub(crate) fn is_char(g: &SGlyph) -> bool {
    g.inline.is_none() && !g.is_space() && !g.ch.is_control() && !g.is_soft_hyphen()
}

/// The width of a space in the glyph's font and size: 100 % letter spacing adds one.
pub(crate) fn space_unit(g: &SGlyph) -> f64 {
    let gid = g.face.glyph_for(' ');
    let w = if gid != 0 { g.face.advance(gid) * g.sx } else { g.face.units_per_em() * g.sx * 0.25 };
    if w.is_finite() { w.max(0.0) } else { 0.0 }
}

/// Does the paragraph use anything but the default spacing (Desired word spacing 100 %, letter
/// spacing 0 %, glyph scaling 100 %)?
pub(crate) fn changes_spacing(j: &Justification) -> bool {
    j.word_spacing.desired != 100.0 || j.letter_spacing.desired != 0.0 || j.glyph_scaling.desired != 100.0
}

/// Each glyph's letter-spacing unit (a space of its font; 0 for spaces and the like), or none
/// when letter spacing never changes.
pub(crate) fn letter_units(sg: &[SGlyph], j: &Justification) -> Vec<f64> {
    let r = &j.letter_spacing;
    if r.min == 0.0 && r.desired == 0.0 && r.max == 0.0 {
        return vec![];
    }
    sg.iter().map(|g| if is_char(g) { space_unit(g) } else { 0.0 }).collect()
}

/// Set the Desired spacing on a paragraph's glyphs: word spaces at Word Spacing % of their
/// width, letter spacing added after each character, characters at Glyph Scaling % of their
/// width. `units` are [`letter_units`].
pub(crate) fn apply_desired(sg: &mut [SGlyph], j: &Justification, units: &[f64]) {
    let j = j.clamped();
    let word = j.word_spacing.desired / 100.0;
    let letter = j.letter_spacing.desired / 100.0;
    let scale = j.glyph_scaling.desired / 100.0;
    for (i, g) in sg.iter_mut().enumerate() {
        if is_word_space(g) {
            g.adv *= word;
        } else if is_char(g) {
            if scale != 1.0 {
                scale_glyph(g, scale);
            }
            g.adv += letter * units.get(i).copied().unwrap_or(0.0);
        }
    }
}

/// Scale a glyph's width (and its advance) by `f`.
pub(crate) fn scale_glyph(g: &mut SGlyph, f: f64) {
    if !f.is_finite() || f <= 0.0 {
        return;
    }
    g.sx *= f;
    g.dx *= f;
    g.adv *= f;
}

/// The composer's stretch and shrink for a paragraph's Justification options (relative to the
/// spacing already set at the Desired values).
pub(crate) fn glue(j: &Justification) -> Glue {
    let j = j.clamped();
    let rel = |r: &SpacingRange| if r.desired > EPS { ((r.max - r.desired) / r.desired, (r.desired - r.min) / r.desired) } else { (0.0, 0.0) };
    let (word_stretch, word_shrink) = rel(&j.word_spacing);
    let (glyph_stretch, glyph_shrink) = rel(&j.glyph_scaling);
    let l = &j.letter_spacing;
    Glue {
        word_stretch,
        word_shrink,
        letter_stretch: (l.max - l.desired) / 100.0,
        letter_shrink: (l.desired - l.min) / 100.0,
        glyph_stretch,
        glyph_shrink,
    }
}

/// What a justified line's glyphs get on top of their advances.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Fill {
    /// Added to each word space.
    pub per_space: f64,
    /// Added after each character but the line's last, per unit of its [`letter_units`].
    pub per_unit: f64,
    /// Characters' widths scale by this.
    pub glyph_scale: f64,
    /// Added after each glyph but the last (a single word spread across the line).
    pub per_gap: f64,
}

/// What a justified line holds: its word spaces (count and width), letter gaps (their units),
/// characters' width and gaps between glyphs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LineStuff {
    pub spaces: usize,
    pub space_width: f64,
    /// The narrowest word space (they never shrink below zero).
    pub min_space: f64,
    pub units: f64,
    pub chars: f64,
    pub gaps: usize,
}

/// Share `extra` (positive: room left, negative: overfull) between the line's spacing within the
/// Justification limits, in order word spacing, letter spacing, glyph scaling; the rest goes to
/// the word spaces (or, with none, spreads a single word when `spread`).
pub(crate) fn fill(extra: f64, s: &LineStuff, glue: &Glue, spread: bool) -> Fill {
    let mut f = Fill { glyph_scale: 1.0, ..Fill::default() };
    if !extra.is_finite() || extra.abs() < EPS {
        return f;
    }
    let grow = extra > 0.0;
    let take = |r: &mut f64, room: f64| {
        let room = room.max(0.0);
        let t = if grow { r.min(room) } else { r.max(-room) };
        *r -= t;
        t
    };
    let mut r = extra;
    let mut word = take(&mut r, s.space_width * if grow { glue.word_stretch } else { glue.word_shrink });
    let letters = take(&mut r, s.units * if grow { glue.letter_stretch } else { glue.letter_shrink });
    let glyphs = take(&mut r, s.chars * if grow { glue.glyph_stretch } else { glue.glyph_shrink });
    if s.spaces > 0 {
        word += r;
        // Never below zero.
        f.per_space = (word / s.spaces as f64).max(-s.min_space);
    } else if grow && spread && s.gaps > 0 {
        f.per_gap = r / s.gaps as f64;
    }
    if s.units > EPS {
        f.per_unit = letters / s.units;
    }
    if s.chars > EPS {
        f.glyph_scale = 1.0 + glyphs / s.chars;
    }
    f
}

/// How a justified line with no word space is set: spread across the line, or aligned (0 left,
/// 1 centre, 2 right).
pub(crate) fn single_word(j: &Justification) -> Option<u8> {
    match j.single_word {
        SingleWordJustify::FullJustify => None,
        SingleWordJustify::AlignLeft => Some(0),
        SingleWordJustify::AlignCenter => Some(1),
        SingleWordJustify::AlignRight => Some(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_takes_word_then_letter_then_glyph_room_then_overflows_into_words() {
        let glue = Glue { word_stretch: 0.5, word_shrink: 0.2, letter_stretch: 0.1, letter_shrink: 0.1, glyph_stretch: 0.02, glyph_shrink: 0.02 };
        let s = LineStuff { spaces: 2, space_width: 10.0, min_space: 5.0, units: 50.0, chars: 100.0, gaps: 12 };
        // Word room 5, letter room 5, glyph room 2.
        let f = fill(4.0, &s, &glue, true);
        assert!((f.per_space - 2.0).abs() < 1e-9 && f.per_unit == 0.0 && f.glyph_scale == 1.0);
        let f = fill(8.0, &s, &glue, true);
        assert!((f.per_space - 2.5).abs() < 1e-9 && (f.per_unit - 0.06).abs() < 1e-9 && f.glyph_scale == 1.0);
        let f = fill(13.0, &s, &glue, true);
        assert!((f.glyph_scale - 1.02).abs() < 1e-9);
        assert!((f.per_space - 3.0).abs() < 1e-9, "the last point goes to the word spaces: {f:?}");
        // Shrinking: word spaces to their minimum first, never below zero.
        let f = fill(-1.0, &s, &glue, true);
        assert!((f.per_space + 0.5).abs() < 1e-9);
        let f = fill(-100.0, &s, &glue, true);
        assert!(f.per_space >= -5.0 && f.glyph_scale < 1.0 && f.per_unit < 0.0);
        // A single word spreads only when told to.
        let one = LineStuff { spaces: 0, gaps: 4, ..s };
        assert!(fill(8.0, &one, &Glue::default(), true).per_gap > 0.0);
        assert_eq!(fill(8.0, &one, &Glue::default(), false).per_gap, 0.0);
        assert_eq!(fill(f64::NAN, &s, &glue, true), Fill { glyph_scale: 1.0, ..Fill::default() });
    }

    #[test]
    fn default_glue_is_the_old_composer_stretch() {
        assert_eq!(glue(&Justification::default()), Glue::default());
    }
}
