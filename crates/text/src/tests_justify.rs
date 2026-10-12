//! Paragraph panel menu › Hyphenation… and Justification… in composition (helpx:
//! <https://helpx.adobe.com/illustrator/using/hyphenation-line-breaks.html> and
//! <https://helpx.adobe.com/illustrator/desktop/design-with-text/edit-format-text/adjust-word-and-letterspacing-in-justified-text.html>).

use super::*;
use kurbo::{Affine, Shape};
use vectorcraft_doc::{CharStyle, Hyphenation, Justification, Justify, SingleWordJustify, SpacingRange, TextKind};
use vectorcraft_geom::PathData;

fn db() -> &'static FontDb {
    FontDb::global()
}

fn style(size: f64) -> CharStyle {
    CharStyle { size, ..CharStyle::default() }
}

fn area(text: &str, size: f64, frame: Rect, justify: Justify) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, style(size));
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&frame.to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    t.para.justify = justify;
    t
}

const LONG: &str = "Extraordinarily comprehensive typographical considerations notwithstanding everything. \
Unquestionably international understanding characterises establishments everywhere. \
Representational transformations accompanying superintendents notwithstanding.";

/// Lines that end in an automatic hyphen (the hyphen glyph has no source characters).
fn hyphenated_lines(l: &TextLayout) -> Vec<bool> {
    (0..l.lines.len()).map(|li| l.glyphs.iter().any(|g| g.line == li && g.len == 0)).collect()
}

fn hyphens(l: &TextLayout) -> usize {
    hyphenated_lines(l).iter().filter(|&&h| h).count()
}

fn width(l: &TextLayout) -> f64 {
    l.lines.iter().map(|li| li.x1 - li.x0).fold(0.0, f64::max)
}

#[test]
fn hyphenation_letter_counts_and_capitalized_words() {
    let opts = |min_word, after_first, before_last| Hyphenation { min_word, after_first, before_last, ..Hyphenation::default() };
    // Words Longer Than: "typography" (10 letters) breaks at 6, not at 11.
    assert!(!hyphen::hyphen_points_with("typography", &opts(6, 3, 3)).is_empty());
    assert!(hyphen::hyphen_points_with("typography", &opts(11, 3, 3)).is_empty());
    // After First / Before Last: no hyphen nearer the ends than they allow.
    for (a, b) in [(1, 1), (2, 2), (3, 3), (4, 5), (5, 2)] {
        for w in ["typography", "considerations", "notwithstanding", "aromatic"] {
            let n = w.chars().count();
            let pts = hyphen::hyphen_points_with(w, &opts(2, a, b));
            assert!(pts.iter().all(|&p| p >= a as usize && p <= n - b as usize), "{w} {a}/{b}: {pts:?}");
        }
    }
    let looser = hyphen::hyphen_points_with("considerations", &opts(2, 2, 2));
    let stricter = hyphen::hyphen_points_with("considerations", &opts(2, 5, 5));
    assert!(looser.len() > stricter.len(), "{looser:?} vs {stricter:?}");
    // Hyphenate Capitalized Words off: a capitalised word keeps whole.
    let off = Hyphenation { capitalized: false, ..Hyphenation::default() };
    assert!(!hyphen::hyphen_points_with("Typography", &Hyphenation::default()).is_empty());
    assert!(hyphen::hyphen_points_with("Typography", &off).is_empty());
    assert!(!hyphen::hyphen_points_with("typography", &off).is_empty());
}

#[test]
fn words_longer_than_and_capitalized_words_act_in_layout() {
    let mut t = area(LONG, 14.0, Rect::new(0.0, 0.0, 120.0, 2000.0), Justify::Left);
    t.para.hyphenate = true;
    assert!(hyphens(&layout(db(), &t)) > 0);
    // No word here is longer than 25 letters.
    t.para.hyphenation.min_word = 25;
    assert_eq!(hyphens(&layout(db(), &t)), 0);
    // Capitalised words only: none hyphenated with Hyphenate Capitalized Words off.
    let caps = "Extraordinarily Comprehensive Typographical Considerations Notwithstanding Everything.";
    let mut t = area(caps, 14.0, Rect::new(0.0, 0.0, 120.0, 2000.0), Justify::Left);
    t.para.hyphenate = true;
    assert!(hyphens(&layout(db(), &t)) > 0);
    t.para.hyphenation.capitalized = false;
    assert_eq!(hyphens(&layout(db(), &t)), 0);
}

#[test]
fn hyphen_limit_caps_consecutive_hyphenated_lines() {
    for composer in [Composer::EveryLine, Composer::SingleLine] {
        for justify in [Justify::Left, Justify::JustifyLeft] {
            let mut t = area(&LONG.repeat(3), 14.0, Rect::new(0.0, 0.0, 110.0, 4000.0), justify);
            t.para.hyphenate = true;
            t.para.composer = composer;
            let free = hyphenated_lines(&layout(db(), &t));
            assert!(free.windows(2).any(|w| w[0] && w[1]), "{composer:?} {justify:?}: consecutive hyphens without a limit: {free:?}");
            t.para.hyphenation.hyphen_limit = 1;
            let limited = hyphenated_lines(&layout(db(), &t));
            assert!(limited.iter().any(|&h| h), "{composer:?} {justify:?}: still hyphenates");
            assert!(!limited.windows(2).any(|w| w[0] && w[1]), "{composer:?} {justify:?}: {limited:?}");
        }
    }
}

#[test]
fn hyphenation_zone_keeps_words_whole_near_the_right_edge() {
    let mut t = area(LONG, 14.0, Rect::new(0.0, 0.0, 150.0, 2000.0), Justify::Left);
    t.para.hyphenate = true;
    t.para.composer = Composer::SingleLine;
    let free = layout(db(), &t);
    assert!(hyphens(&free) > 0);
    // A zone as wide as the frame: every line could end before the word, so none breaks.
    t.para.hyphenation.zone = 150.0;
    let zoned = layout(db(), &t);
    assert_eq!(hyphens(&zoned), 0);
    assert!(zoned.lines.len() >= free.lines.len());
    // A narrower zone hyphenates no more than none at all.
    t.para.hyphenation.zone = 30.0;
    assert!(hyphens(&layout(db(), &t)) <= hyphens(&free));
}

#[test]
fn desired_spacing_sets_every_line() {
    let lay = |j: Justification| {
        let mut t = TextObject::point(Point::ZERO, "spacing in type", style(20.0));
        t.para.justification = j;
        layout(db(), &t)
    };
    let base = width(&lay(Justification::default()));
    let word = width(&lay(Justification { word_spacing: SpacingRange::new(0.0, 300.0, 1000.0), ..Justification::default() }));
    let letter = width(&lay(Justification { letter_spacing: SpacingRange::new(0.0, 50.0, 100.0), ..Justification::default() }));
    let glyph = width(&lay(Justification { glyph_scaling: SpacingRange::new(50.0, 150.0, 200.0), ..Justification::default() }));
    let narrow = width(&lay(Justification { glyph_scaling: SpacingRange::new(50.0, 50.0, 200.0), ..Justification::default() }));
    assert!(word > base + 1.0, "{word} vs {base}");
    assert!(letter > base + 1.0, "{letter} vs {base}");
    assert!(glyph > base * 1.3, "{glyph} vs {base}");
    assert!(narrow < base * 0.7, "{narrow} vs {base}");
    // Glyph Scaling widens the outlines too.
    let wide = lay(Justification { glyph_scaling: SpacingRange::new(50.0, 200.0, 200.0), ..Justification::default() });
    let plain = lay(Justification::default());
    let ow = |l: &TextLayout| l.glyphs.first().map(|g| g.outline.bounding_box().width()).unwrap_or(0.0);
    assert!(ow(&wide) > ow(&plain) * 1.8, "{} vs {}", ow(&wide), ow(&plain));
    // Untrusted values are clamped (no NaN reaches the layout).
    let wild = lay(Justification {
        word_spacing: SpacingRange::new(f64::NAN, f64::INFINITY, -5.0),
        glyph_scaling: SpacingRange::new(-1.0, 1e9, f64::NAN),
        auto_leading: f64::NAN,
        ..Justification::default()
    });
    assert!(wild.glyphs.iter().all(|g| g.origin.x.is_finite() && g.advance.is_finite()));
}

#[test]
fn justified_lines_use_letter_spacing_within_its_limits() {
    let frame = Rect::new(0.0, 0.0, 200.0, 2000.0);
    let lay = |j: Justification| {
        let mut t = area(LONG, 12.0, frame, Justify::JustifyLeft);
        t.para.justification = j;
        layout(db(), &t)
    };
    // Gaps between letters of the same word on the first line, beyond the natural advance.
    let letter_gap = |l: &TextLayout, plain: &TextLayout| {
        let first: Vec<_> = l.glyphs.iter().filter(|g| g.line == 0 && !g.outline.elements().is_empty()).collect();
        let nat: Vec<_> = plain.glyphs.iter().filter(|g| g.line == 0 && !g.outline.elements().is_empty()).collect();
        first.first().zip(nat.first()).map_or(0.0, |(a, b)| a.advance - b.advance)
    };
    let fixed_words = SpacingRange::new(100.0, 100.0, 100.0);
    let plain = lay(Justification::default());
    let none = lay(Justification { word_spacing: fixed_words, ..Justification::default() });
    let some = lay(Justification { word_spacing: fixed_words, letter_spacing: SpacingRange::new(0.0, 0.0, 20.0), ..Justification::default() });
    // Default: letters are untouched, the word spaces take the slack.
    assert!(letter_gap(&plain, &plain).abs() < 1e-9);
    assert!(letter_gap(&none, &plain).abs() < 1e-9);
    assert!(letter_gap(&some, &plain) > 1e-3, "letters spaced: {}", letter_gap(&some, &plain));
    // Every justified (non-last) line reaches the frame's width.
    for l in [&plain, &none, &some] {
        let n = l.lines.len();
        for li in l.lines.iter().take(n.saturating_sub(1)) {
            if l.glyphs[li.glyph_start..li.glyph_end].iter().any(|g| g.len == 0) {
                continue;
            }
            assert!((li.x1 - frame.width()).abs() < 0.5, "{li:?}");
        }
    }
}

#[test]
fn justified_lines_use_glyph_scaling_within_its_limits() {
    let frame = Rect::new(0.0, 0.0, 200.0, 2000.0);
    let lay = |j: Justification| {
        let mut t = area(LONG, 12.0, frame, Justify::JustifyLeft);
        t.para.justification = j;
        layout(db(), &t)
    };
    let fixed_words = SpacingRange::new(100.0, 100.0, 100.0);
    let plain = lay(Justification { word_spacing: fixed_words, ..Justification::default() });
    let scaled = lay(Justification { word_spacing: fixed_words, glyph_scaling: SpacingRange::new(95.0, 100.0, 105.0), ..Justification::default() });
    let first = |l: &TextLayout| l.glyphs.first().map(|g| g.advance).unwrap_or(0.0);
    let ratio = first(&scaled) / first(&plain);
    assert!(ratio > 1.0 + 1e-6 && ratio <= 1.05 + 1e-6, "glyph scaling within 105 %: {ratio}");
}

#[test]
fn single_word_justification() {
    // "Typography" alone on a justified non-last line.
    let frame = Rect::new(0.0, 0.0, 160.0, 400.0);
    let lay = |single_word| {
        let mut t = area("Typography arrangement", 20.0, frame, Justify::JustifyLeft);
        t.para.justification.single_word = single_word;
        layout(db(), &t)
    };
    let full = lay(SingleWordJustify::FullJustify);
    let left = lay(SingleWordJustify::AlignLeft);
    let centre = lay(SingleWordJustify::AlignCenter);
    let right = lay(SingleWordJustify::AlignRight);
    assert!(full.lines.len() >= 2);
    let l0 = |l: &TextLayout| (l.lines[0].x0, l.lines[0].x1);
    let (fx0, fx1) = l0(&full);
    assert!(fx0.abs() < 1e-6 && (fx1 - 160.0).abs() < 0.5, "spread: {fx0} {fx1}");
    let (lx0, lx1) = l0(&left);
    assert!(lx0.abs() < 1e-6 && lx1 < 150.0, "left: {lx0} {lx1}");
    let (rx0, rx1) = l0(&right);
    assert!(rx0 > 10.0 && (rx1 - 160.0).abs() < 0.5, "right: {rx0} {rx1}");
    let (cx0, cx1) = l0(&centre);
    assert!(((cx0 - 0.0) - (160.0 - cx1)).abs() < 0.5 && cx0 > 5.0, "centre: {cx0} {cx1}");
}

#[test]
fn auto_leading_sets_the_leading_of_auto_lines() {
    let lay = |pct: f64| {
        let mut t = area("one two three four five six seven eight", 10.0, Rect::new(0.0, 0.0, 60.0, 2000.0), Justify::Left);
        t.para.justification.auto_leading = pct;
        layout(db(), &t)
    };
    let step = |l: &TextLayout| l.lines[1].baseline - l.lines[0].baseline;
    let default = lay(120.0);
    assert!((step(&default) - 12.0).abs() < 0.6, "the font's auto leading: {}", step(&default));
    assert!((step(&lay(200.0)) - 20.0).abs() < 1e-6, "{}", step(&lay(200.0)));
    assert!((step(&lay(150.0)) - 15.0).abs() < 1e-6);
    // Fixed leading ignores Auto Leading.
    let mut t = area("one two three four five six seven eight", 10.0, Rect::new(0.0, 0.0, 60.0, 2000.0), Justify::Left);
    t.para.justification.auto_leading = 300.0;
    for r in &mut t.runs {
        r.style.leading = Some(14.0);
    }
    let fixed = layout(db(), &t);
    assert!((step(&fixed) - 14.0).abs() < 1e-6, "{}", step(&fixed));
}

#[test]
fn default_options_leave_composition_unchanged() {
    // The defaults are VectorCraft's composition before the dialogs: a paragraph with them set
    // explicitly lays out identically.
    let mut t = area(LONG, 12.0, Rect::new(0.0, 0.0, 180.0, 2000.0), Justify::JustifyLeft);
    t.para.hyphenate = true;
    let a = layout(db(), &t);
    t.para.hyphenation = Hyphenation::default();
    t.para.justification = Justification::default();
    let b = layout(db(), &t);
    assert_eq!(a.glyphs.len(), b.glyphs.len());
    for (x, y) in a.glyphs.iter().zip(&b.glyphs) {
        assert_eq!(x.origin, y.origin);
    }
}
