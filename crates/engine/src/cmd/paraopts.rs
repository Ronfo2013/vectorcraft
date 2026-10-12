//! Paragraph panel menu › Hyphenation… and Justification…: the paragraph options the line breaker
//! and justified lines use (helpx: "Set hyphenation and line breaks",
//! <https://helpx.adobe.com/illustrator/using/hyphenation-line-breaks.html>, and "Adjust word and
//! letterspacing in justified text",
//! <https://helpx.adobe.com/illustrator/desktop/design-with-text/edit-format-text/adjust-word-and-letterspacing-in-justified-text.html>).
//!
//! Also Paragraph panel menu › Roman Hanging Punctuation (per paragraph) and Type › Optical Margin
//! Alignment (per story), helpx "Formatting paragraphs" › Hanging punctuation,
//! <https://helpx.adobe.com/illustrator/using/formatting-paragraphs.html>: toggles, `query: true`
//! asks (see `vectorcraft_text::hang` for what hangs and how far).
//!
//! Each command with no option queries the first target paragraph's values (the dialog's fields);
//! with options it sets them on the paragraphs of the selected type (with `start`/`end`: those the
//! range touches), one undo step.

use serde_json::{Map, Value, json};
use vectorcraft_doc::{Hyphenation, Justification, NodeId, NodeKind, ParaStyle, SingleWordJustify, SpacingRange};

use super::typecmd::{TextRange, para_span, refresh_bounds, text_targets};
use super::*;

const HYPHENATION: &str = "text.hyphenation";
const JUSTIFICATION: &str = "text.justification";
const ROMAN_HANGING: &str = "text.romanHangingPunctuation";
const OPTICAL_MARGIN: &str = "type.opticalMarginAlignment";

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
            "{ids?|id?, start?: byte, end?: byte, wordSpacingMin?, wordSpacingDesired?, wordSpacingMax?: % of a space (0–1000), letterSpacingMin?, letterSpacingDesired?, letterSpacingMax?: % of a space added between letters (-100–500), glyphScalingMin?, glyphScalingDesired?, glyphScalingMax?: % of the glyphs' width (50–200), autoLeading?: % of the type size (0–500; leading of characters whose leading is Auto), singleWordJustification?: fullJustify|alignLeft|alignCenter|alignRight (a justified line holding one word)} set the paragraphs' Justification options: Desired spacing applies to every line, justified lines stretch or shrink within Minimum and Maximum (word spacing first, then letter spacing, then glyph scaling); values given together must be in order (Minimum ≤ Desired ≤ Maximum), and one given alone moves the paragraph's others to keep that order; no option → the first target paragraph's values (singleWordJustification of a paragraph from before the option: how it is set, fullJustify in Justify All Lines, else alignLeft)",
            has_doc,
            justification
        ),
        cmd!(
            "text.romanHangingPunctuation",
            "Roman Hanging Punctuation",
            [],
            None,
            "{ids?|id?, start?: byte, end?: byte, on?: bool, query?: bool} Paragraph panel menu › Roman Hanging Punctuation, per paragraph: quotation marks, hyphens, full stops and commas at either edge of a line stand wholly outside it, asterisks, tildes, ellipses, en and em dashes, colons and semicolons half outside; justified lines fill to the hung edges (left-to-right paragraphs, area and point type). `on` sets it on the target paragraphs (with `start`/`end`: those the range touches), without it toggles (on unless every target paragraph has it), one undo step → {ids, on}; `query: true` → the first target paragraph's {on}",
            has_doc,
            roman_hanging
        ),
        cmd!(
            "type.opticalMarginAlignment",
            "Optical Margin Alignment",
            [],
            None,
            "{ids?|id?, on?: bool, query?: bool} Type › Optical Margin Alignment, for every paragraph of the selected type objects (and the frames threaded with them): punctuation (as Roman Hanging Punctuation) and the edges of letters such as A, T, V, W and Y stand partly outside both edges of each line so they look straight; justified lines fill to the hung edges. `on` sets it, without it toggles (on unless every target has it), one undo step → {ids, on}; `query: true` → {on (the first target's), mixed, ids}",
            has_doc,
            optical_margin
        ),
    ]
}

/// The target parameters both commands take besides their options.
const TARGET_KEYS: [&str; 4] = ["ids", "id", "start", "end"];

/// An error for a parameter that is neither a target nor one of `options` (a misspelt option
/// would otherwise be a silent query).
fn reject_unknown(p: &Value, options: &[&str], cmd: &str) -> Result<()> {
    let Some(o) = p.as_object() else { return Ok(()) };
    match o.keys().find(|k| !TARGET_KEYS.contains(&k.as_str()) && !options.contains(&k.as_str())) {
        Some(k) => Err(bad(cmd, format!("unknown parameter `{k}`"))),
        None => Ok(()),
    }
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
    reject_unknown(p, &HYPHENATION_KEYS, C)?;
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

/// Every option key of `text.justification`.
const JUSTIFICATION_KEYS: [&str; 11] = [
    "wordSpacingMin",
    "wordSpacingDesired",
    "wordSpacingMax",
    "letterSpacingMin",
    "letterSpacingDesired",
    "letterSpacingMax",
    "glyphScalingMin",
    "glyphScalingDesired",
    "glyphScalingMax",
    "autoLeading",
    "singleWordJustification",
];

fn justification(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = JUSTIFICATION;
    reject_unknown(p, &JUSTIFICATION_KEYS, C)?;
    // (prefix, [min, desired, max]) given.
    let mut given: Vec<(&str, [Option<f64>; 3])> = vec![];
    for (prefix, limits) in RANGES {
        let v = [
            number(p, &format!("{prefix}Min"), limits, C)?,
            number(p, &format!("{prefix}Desired"), limits, C)?,
            number(p, &format!("{prefix}Max"), limits, C)?,
        ];
        // Values given together contradict each other out of order.
        let after = |a: Option<f64>, b: Option<f64>| matches!((a, b), (Some(a), Some(b)) if a > b);
        let [min, desired, max] = v;
        if after(min, desired) || after(desired, max) || after(min, max) {
            return Err(bad(C, format!("`{prefix}Min`, `{prefix}Desired` and `{prefix}Max` must be in order (Minimum ≤ Desired ≤ Maximum)")));
        }
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
        let para = first_para(s, p, C)?;
        let j = para.justification.clamped();
        let mut o = Map::new();
        for (prefix, r) in [("wordSpacing", j.word_spacing), ("letterSpacing", j.letter_spacing), ("glyphScaling", j.glyph_scaling)] {
            o.insert(format!("{prefix}Min"), json!(r.min));
            o.insert(format!("{prefix}Desired"), json!(r.desired));
            o.insert(format!("{prefix}Max"), json!(r.max));
        }
        o.insert("autoLeading".into(), json!(j.auto_leading));
        // A paragraph from before the option shows the choice it is set like.
        let shown = j.single_word.effective(para.justify);
        let sw = SINGLE_WORD.iter().find(|(_, w)| *w == shown).map_or("fullJustify", |(k, _)| k);
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

/// The target text objects of `type.opticalMarginAlignment`, with every frame threaded with them:
/// the option is the story's.
fn story_targets(s: &Session, p: &Value, cmd: &str) -> Result<Vec<NodeId>> {
    let mut ids = text_targets(s, p, cmd)?;
    let st = s.doc()?;
    for thread in &st.doc.text_threads {
        if thread.iter().any(|id| ids.contains(id)) {
            for id in thread {
                if !ids.contains(id) && matches!(st.doc.node(*id).map(|n| &n.kind), Some(NodeKind::Text(_))) {
                    ids.push(*id);
                }
            }
        }
    }
    Ok(ids)
}

/// `on` and `query` of a toggle command: Some(true/false) sets, None toggles; `query` alone asks.
fn toggle_params(p: &Value, cmd: &str, extra: &[&str]) -> Result<(Option<bool>, bool)> {
    let keys: Vec<&str> = ["on", "query"].into_iter().chain(extra.iter().copied()).collect();
    reject_unknown(p, &keys, cmd)?;
    let on = flag(p, "on", cmd)?;
    let query = flag(p, "query", cmd)?.unwrap_or(false);
    if query && on.is_some() {
        return Err(bad(cmd, "`query` takes no `on`"));
    }
    Ok((on, query))
}

fn optical_margin(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = OPTICAL_MARGIN;
    let (on, query) = toggle_params(p, C, &[])?;
    let ids = story_targets(s, p, C)?;
    let states: Vec<bool> = {
        let st = s.doc()?;
        ids.iter()
            .filter_map(|id| match st.doc.node(*id).map(|n| &n.kind) {
                Some(NodeKind::Text(t)) => Some(t.optical_margin),
                _ => None,
            })
            .collect()
    };
    let ids_json = ids.iter().map(|i| i.0).collect::<Vec<_>>();
    if query {
        return Ok(
            json!({ "on": states.first().copied().unwrap_or(false), "mixed": states.iter().any(|&v| v != states.first().copied().unwrap_or(false)), "ids": ids_json }),
        );
    }
    // Toggled: on unless every target already has it.
    let on = on.unwrap_or_else(|| !states.iter().all(|&v| v));
    s.edit("Optical Margin Alignment", |d, _| {
        for id in &ids {
            if let Some(NodeKind::Text(t)) = d.node_mut(*id).map(|n| &mut n.kind)
                && t.optical_margin != on
            {
                t.optical_margin = on;
                refresh_bounds(t);
            }
        }
        Ok(())
    })?;
    Ok(json!({ "on": on, "ids": ids_json }))
}

fn roman_hanging(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = ROMAN_HANGING;
    let (on, query) = toggle_params(p, C, &[])?;
    if query {
        let para = first_para(s, p, C)?;
        return Ok(json!({ "on": para.roman_hanging }));
    }
    let on = match on {
        Some(v) => v,
        None => {
            // Toggled: on unless every target paragraph already has it.
            let ids = text_targets(s, p, C)?;
            let range = TextRange::parse(p, C)?;
            let st = s.doc()?;
            !ids.iter().all(|id| match st.doc.node(*id).map(|n| &n.kind) {
                Some(NodeKind::Text(t)) => {
                    let span = para_span(range, t).unwrap_or(0..t.paragraph_count());
                    span.into_iter().all(|k| t.para_at(k).roman_hanging)
                }
                _ => true,
            })
        }
    };
    let mut out = edit_paras(s, p, C, "Roman Hanging Punctuation", |para| para.roman_hanging = on)?;
    if let Some(o) = out.as_object_mut() {
        o.insert("on".into(), json!(on));
    }
    Ok(out)
}
