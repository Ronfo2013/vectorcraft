//! Paragraph panel menu › Hyphenation… and Justification…: the paragraph options the line breaker
//! and justified lines use (helpx: "Set hyphenation and line breaks",
//! <https://helpx.adobe.com/illustrator/using/hyphenation-line-breaks.html>, and "Adjust word and
//! letterspacing in justified text",
//! <https://helpx.adobe.com/illustrator/desktop/design-with-text/edit-format-text/adjust-word-and-letterspacing-in-justified-text.html>).
//!
//! Each command with no option queries the first target paragraph's values (the dialog's fields);
//! with options it sets them on the paragraphs of the selected type (with `start`/`end`: those the
//! range touches), one undo step.

use serde_json::{Map, Value, json};
use vectorcraft_doc::{Hyphenation, Justification, NodeKind, ParaStyle, SingleWordJustify, SpacingRange};

use super::typecmd::{TextRange, para_span, refresh_bounds, text_targets};
use super::*;

const HYPHENATION: &str = "text.hyphenation";
const JUSTIFICATION: &str = "text.justification";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "text.hyphenation",
            "Hyphenation",
            [],
            None,
            "{ids?|id?, start?: byte, end?: byte, hyphenate?: bool (automatic hyphenation), minWord?: letters (Words Longer Than, 1–25), afterFirst?: letters (After First, 1–25), beforeLast?: letters (Before Last, 1–25), hyphenLimit?: lines (most consecutive hyphenated lines, 0–25; 0: no limit), zone?: pt (Hyphenation Zone, 0–1296: with the Single-line Composer a word whose line could already end this close to the right edge goes whole to the next line; 0: none), capitalized?: bool (Hyphenate Capitalized Words)} set the paragraphs' Hyphenation options; no option → the first target paragraph's {hyphenate, minWord, afterFirst, beforeLast, hyphenLimit, zone, capitalized}",
            has_doc,
            hyphenation
        ),
        cmd!(
            "text.justification",
            "Justification",
            [],
            None,
            "{ids?|id?, start?: byte, end?: byte, wordSpacingMin?, wordSpacingDesired?, wordSpacingMax?: % of a space (0–1000), letterSpacingMin?, letterSpacingDesired?, letterSpacingMax?: % of a space added between letters (-100–500), glyphScalingMin?, glyphScalingDesired?, glyphScalingMax?: % of the glyphs' width (50–200), autoLeading?: % of the type size (0–500; leading of characters whose leading is Auto), singleWordJustification?: fullJustify|alignLeft|alignCenter|alignRight (a justified line holding one word)} set the paragraphs' Justification options: Desired spacing applies to every line, justified lines stretch or shrink within Minimum and Maximum (word spacing first, then letter spacing, then glyph scaling); a Desired outside Minimum or Maximum moves them; no option → the first target paragraph's values",
            has_doc,
            justification
        ),
    ]
}

/// Option `k` of `p` as a number within `limits` (None: not given).
fn number(p: &Value, k: &str, limits: (f64, f64), cmd: &str) -> Result<Option<f64>> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{k}` must be a number")))?;
            if !(limits.0..=limits.1).contains(&n) {
                return Err(bad(cmd, format!("`{k}` must be between {} and {}", limits.0, limits.1)));
            }
            Ok(Some(n))
        }
    }
}

/// Option `k` of `p` as a whole number within `limits`.
fn count(p: &Value, k: &str, limits: (u32, u32), cmd: &str) -> Result<Option<u32>> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let n = v
                .as_u64()
                .or_else(|| v.as_f64().filter(|f| f.is_finite() && f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))
                .ok_or_else(|| bad(cmd, format!("`{k}` must be a whole number")))?;
            let n = u32::try_from(n).unwrap_or(u32::MAX);
            if !(limits.0..=limits.1).contains(&n) {
                return Err(bad(cmd, format!("`{k}` must be between {} and {}", limits.0, limits.1)));
            }
            Ok(Some(n))
        }
    }
}

fn flag(p: &Value, k: &str, cmd: &str) -> Result<Option<bool>> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| bad(cmd, format!("`{k}` must be true or false"))),
    }
}

/// The first target paragraph (at `start`, else the first) of the first target text object.
fn first_para(s: &Session, p: &Value, cmd: &str) -> Result<ParaStyle> {
    let ids = text_targets(s, p, cmd)?;
    let range = TextRange::parse(p, cmd)?;
    let st = s.doc()?;
    ids.iter()
        .find_map(|id| match st.doc.node(*id).map(|n| &n.kind) {
            Some(NodeKind::Text(t)) => Some(t.para_at(para_span(range, t).map_or(0, |r| r.start)).clone()),
            _ => None,
        })
        .ok_or_else(|| bad(cmd, "no text objects selected"))
}

/// Apply `f` to the target paragraphs in one undo step named `label`.
fn edit_paras(s: &mut Session, p: &Value, cmd: &str, label: &str, f: impl Fn(&mut ParaStyle)) -> Result<Value> {
    let ids = text_targets(s, p, cmd)?;
    let range = TextRange::parse(p, cmd)?;
    s.edit(label, |d, _| {
        for id in &ids {
            if let Some(NodeKind::Text(t)) = d.node_mut(*id).map(|n| &mut n.kind) {
                let span = para_span(range, t);
                t.edit_paras(span, &f);
                refresh_bounds(t);
            }
        }
        Ok(())
    })?;
    Ok(json!({ "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>() }))
}

const HYPHENATION_KEYS: [&str; 7] = ["hyphenate", "minWord", "afterFirst", "beforeLast", "hyphenLimit", "zone", "capitalized"];

fn hyphenation(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = HYPHENATION;
    if !HYPHENATION_KEYS.iter().any(|k| p.get(*k).is_some_and(|v| !v.is_null())) {
        let para = first_para(s, p, C)?;
        let h = para.hyphenation.clamped();
        return Ok(json!({
            "hyphenate": para.hyphenate,
            "minWord": h.min_word,
            "afterFirst": h.after_first,
            "beforeLast": h.before_last,
            "hyphenLimit": h.hyphen_limit,
            "zone": h.zone,
            "capitalized": h.capitalized,
        }));
    }
    let hyphenate = flag(p, "hyphenate", C)?;
    let min_word = count(p, "minWord", Hyphenation::LETTERS, C)?;
    let after_first = count(p, "afterFirst", Hyphenation::LETTERS, C)?;
    let before_last = count(p, "beforeLast", Hyphenation::LETTERS, C)?;
    let hyphen_limit = count(p, "hyphenLimit", Hyphenation::LIMIT, C)?;
    let zone = number(p, "zone", Hyphenation::ZONE, C)?;
    let capitalized = flag(p, "capitalized", C)?;
    edit_paras(s, p, C, "Hyphenation", |para| {
        if let Some(v) = hyphenate {
            para.hyphenate = v;
        }
        let h = &mut para.hyphenation;
        if let Some(v) = min_word {
            h.min_word = v;
        }
        if let Some(v) = after_first {
            h.after_first = v;
        }
        if let Some(v) = before_last {
            h.before_last = v;
        }
        if let Some(v) = hyphen_limit {
            h.hyphen_limit = v;
        }
        if let Some(v) = zone {
            h.zone = v;
        }
        if let Some(v) = capitalized {
            h.capitalized = v;
        }
        *h = h.clamped();
    })
}

/// The Justification fields: (param prefix, limits).
const RANGES: [(&str, (f64, f64)); 3] =
    [("wordSpacing", Justification::WORD_LIMITS), ("letterSpacing", Justification::LETTER_LIMITS), ("glyphScaling", Justification::GLYPH_LIMITS)];

const SINGLE_WORD: [(&str, SingleWordJustify); 4] = [
    ("fullJustify", SingleWordJustify::FullJustify),
    ("alignLeft", SingleWordJustify::AlignLeft),
    ("alignCenter", SingleWordJustify::AlignCenter),
    ("alignRight", SingleWordJustify::AlignRight),
];

fn range_mut<'a>(j: &'a mut Justification, prefix: &str) -> Option<&'a mut SpacingRange> {
    match prefix {
        "wordSpacing" => Some(&mut j.word_spacing),
        "letterSpacing" => Some(&mut j.letter_spacing),
        "glyphScaling" => Some(&mut j.glyph_scaling),
        _ => None,
    }
}

fn justification(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = JUSTIFICATION;
    // (prefix, [min, desired, max]) given.
    let mut given: Vec<(&str, [Option<f64>; 3])> = vec![];
    for (prefix, limits) in RANGES {
        let v = [
            number(p, &format!("{prefix}Min"), limits, C)?,
            number(p, &format!("{prefix}Desired"), limits, C)?,
            number(p, &format!("{prefix}Max"), limits, C)?,
        ];
        if v.iter().any(Option::is_some) {
            given.push((prefix, v));
        }
    }
    let auto_leading = number(p, "autoLeading", Justification::AUTO_LEADING_LIMITS, C)?;
    let single_word = match p.get("singleWordJustification") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            SINGLE_WORD
                .iter()
                .find(|(k, _)| Some(*k) == v.as_str())
                .map(|(_, w)| *w)
                .ok_or_else(|| bad(C, "`singleWordJustification` must be \"fullJustify\", \"alignLeft\", \"alignCenter\" or \"alignRight\""))?,
        ),
    };
    if given.is_empty() && auto_leading.is_none() && single_word.is_none() {
        let j = first_para(s, p, C)?.justification.clamped();
        let mut o = Map::new();
        for (prefix, r) in [("wordSpacing", j.word_spacing), ("letterSpacing", j.letter_spacing), ("glyphScaling", j.glyph_scaling)] {
            o.insert(format!("{prefix}Min"), json!(r.min));
            o.insert(format!("{prefix}Desired"), json!(r.desired));
            o.insert(format!("{prefix}Max"), json!(r.max));
        }
        o.insert("autoLeading".into(), json!(j.auto_leading));
        let sw = SINGLE_WORD.iter().find(|(_, w)| *w == j.single_word).map_or("fullJustify", |(k, _)| k);
        o.insert("singleWordJustification".into(), json!(sw));
        return Ok(Value::Object(o));
    }
    edit_paras(s, p, C, "Justification", |para| {
        let j = &mut para.justification;
        for (prefix, [min, desired, max]) in &given {
            let Some(r) = range_mut(j, prefix) else { continue };
            // A Desired value outside the range moves Minimum or Maximum with it.
            if let Some(d) = desired {
                r.desired = *d;
                r.min = r.min.min(*d);
                r.max = r.max.max(*d);
            }
            if let Some(v) = min {
                r.min = *v;
                r.desired = r.desired.max(*v);
                r.max = r.max.max(*v);
            }
            if let Some(v) = max {
                r.max = *v;
                r.desired = r.desired.min(*v);
                r.min = r.min.min(*v);
            }
        }
        if let Some(v) = auto_leading {
            j.auto_leading = v;
        }
        if let Some(v) = single_word {
            j.single_word = v;
        }
        *j = j.clamped();
    })
}
