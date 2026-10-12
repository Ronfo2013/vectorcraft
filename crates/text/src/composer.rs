//! Every-line composer: a Knuth–Plass style total-fit line breaker.
//!
//! Boxes are the paragraph's glyphs, glue is its spaces, and penalties are break opportunities
//! after hyphens/dashes/CJK and at hyphenation points. The breaks minimise the sum of squared
//! (badness + penalty) demerits over the whole paragraph, plus a demerit when neighbouring lines
//! fall in fitness classes more than one apart (a tight line next to a loose one).
//!
//! - **Justified** lines: word spacing, letter spacing and glyph scaling may stretch to their
//!   Justification Maximum and shrink to their Minimum ([`Glue`]; by default word spacing alone,
//!   to 150% and 80%); the badness comes from the stretch/shrink ratio, so loose lines are traded
//!   for evenly spaced ones.
//! - **Hyphen Limit**: no more than that many consecutive lines end in an automatic hyphen.
//! - **Ragged** lines (left/centre/right aligned): spaces keep their width, so a line never
//!   shrinks (it must fit). The badness comes from the line's leftover space measured against a
//!   rag zone of a sixth of the line width, so the rag is even rather than full lines followed by
//!   a short one, and hyphenating costs less. The last line costs nothing unless it is shorter
//!   than a third of the width (a word or two left dangling).

use crate::shape::SGlyph;

const HYPHEN_PENALTY: f64 = 50.0;
const DOUBLE_HYPHEN_DEMERITS: f64 = 3000.0;
const LINE_PENALTY: f64 = 10.0;
/// Demerits for adjacent lines whose fitness classes differ by more than one.
const ADJ_DEMERITS_JUSTIFIED: f64 = 10_000.0;
const ADJ_DEMERITS_RAGGED: f64 = 1_000.0;
/// Ragged lines only gain from hyphenation by filling the rag, so it costs less than in
/// justified text (where it also evens the word spacing).
const HYPHEN_PENALTY_RAGGED: f64 = 25.0;
/// Ragged text: leftover space is measured in units of this fraction of the line width.
const RAG_ZONE: f64 = 1.0 / 6.0;
/// Ragged text: a last line shorter than this fraction of the width is penalised, by how much
/// shorter it is in units of `SHORT_LAST_ZONE` of the width.
const SHORT_LAST: f64 = 1.0 / 3.0;
const SHORT_LAST_ZONE: f64 = 1.0 / 3.0;
/// Give up (fall back to the single-line composer) beyond this many active breakpoints.
const MAX_ACTIVE: usize = 4096;
const EPS: f64 = 1e-6;

/// A place a line may end: glyph index `end` (exclusive), plus the width of the hyphen that is
/// added when breaking there (0 = no hyphen).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Breakpoint {
    pub end: usize,
    pub hyphen: f64,
    /// An automatic hyphenation point (not a soft hyphen typed in the text): Hyphen Limit applies.
    pub auto: bool,
}

/// How far a justified line's spacing may stretch and shrink (Justification Maximum − Desired and
/// Desired − Minimum), as fractions: of each word space's width, of a space's width per letter
/// gap ([`compose`]'s `units`), and of the characters' widths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Glue {
    pub word_stretch: f64,
    pub word_shrink: f64,
    pub letter_stretch: f64,
    pub letter_shrink: f64,
    pub glyph_stretch: f64,
    pub glyph_shrink: f64,
}

impl Default for Glue {
    /// Word spacing 80/100/150 %, letter spacing and glyph scaling fixed.
    fn default() -> Self {
        Self { word_stretch: 0.5, word_shrink: 0.2, letter_stretch: 0.0, letter_shrink: 0.0, glyph_stretch: 0.0, glyph_shrink: 0.0 }
    }
}

/// How a paragraph is composed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Params {
    /// The last line is justified too (Justify All).
    pub justify_last: bool,
    /// Left/centre/right aligned text: spaces keep their width and lines are scored by their rag.
    pub ragged: bool,
    /// Maximum stretch ratio (justified) or leftover space in rag zones (ragged) of a line.
    pub tolerance: f64,
    /// Lines from this index on all have the same width, so their line numbers are interchangeable
    /// (keeps the number of active breakpoints small).
    pub uniform_from: usize,
    /// Justified lines' stretch and shrink.
    pub glue: Glue,
    /// Hyphen Limit: the most consecutive lines ending in a hyphen before an automatic hyphen is
    /// refused (0: no limit).
    pub hyphen_limit: usize,
}

struct Node {
    pos: usize,
    /// Line number of the next line, clamped to `uniform_from`.
    line: usize,
    fitness: u8,
    demerits: f64,
    hyphenated: bool,
    /// Consecutive lines up to this break that end in a hyphen.
    run: usize,
    prev: Option<usize>,
}

/// Fitness class 0 (tight) … 3 (very loose) of a line with adjustment ratio `r`.
fn fitness(r: f64, ragged: bool) -> u8 {
    let bounds = if ragged { [0.15, 0.4, 0.75] } else { [-0.5, 0.5, 1.0] };
    if r < bounds[0] {
        0
    } else if r <= bounds[1] {
        1
    } else if r <= bounds[2] {
        2
    } else {
        3
    }
}

/// Compose the paragraph `g` into lines. `width(k)` is the available width of line `k`;
/// `start_credit(i)` is the width a line starting at glyph `i` gives back (Mojikumi sets an
/// opening bracket starting a wrapped line flush with the line's start; hanging punctuation hangs
/// its first character outside it); `end_credit(t, hyphenated)` the width a line ending before
/// glyph `t` (trailing spaces dropped) gives back by hanging its last character or its hyphen;
/// `cands` are the break opportunities
/// in increasing order (the paragraph end is added); `units[i]` is the width a letter gap after
/// glyph `i` stretches or shrinks by per unit of letter spacing (a space's width; missing: 0).
/// Returns `(end, hyphen)` per line, or `None` if no set of breaks fits within the tolerance
/// (the caller then falls back to single-line breaking).
pub(crate) fn compose(
    g: &[SGlyph],
    width: &dyn Fn(usize) -> f64,
    start_credit: &dyn Fn(usize) -> f64,
    end_credit: &dyn Fn(usize, bool) -> f64,
    cands: &[Breakpoint],
    units: &[f64],
    p: &Params,
) -> Option<Vec<(usize, bool)>> {
    let n = g.len();
    if n == 0 {
        return Some(vec![]);
    }
    // Prefix sums of advances, of space advances, of letter-gap units and of the other glyphs'
    // advances.
    let mut w = vec![0.0; n + 1];
    let mut sp = vec![0.0; n + 1];
    let mut lu = vec![0.0; n + 1];
    let mut cw = vec![0.0; n + 1];
    for (i, gl) in g.iter().enumerate() {
        let space = gl.is_space();
        w[i + 1] = w[i] + gl.adv;
        sp[i + 1] = sp[i] + if space { gl.adv } else { 0.0 };
        lu[i + 1] = lu[i] + if space { 0.0 } else { units.get(i).copied().filter(|u| u.is_finite()).unwrap_or(0.0) };
        // The characters Glyph Scaling acts on, as the line's own fill counts them.
        cw[i + 1] = cw[i] + if crate::justify::is_char(gl) { gl.adv } else { 0.0 };
    }
    if !w[n].is_finite() || !lu[n].is_finite() {
        return None;
    }
    let glue = p.glue;
    // How far line `a..t` may stretch and shrink (the last glyph has no letter gap after it).
    let give = |a: usize, t: usize| {
        let spaces = sp[t] - sp[a];
        let letters = if t > a { lu[t - 1] - lu[a] } else { 0.0 };
        let chars = cw[t] - cw[a];
        (
            spaces * glue.word_stretch + letters * glue.letter_stretch + chars * glue.glyph_stretch,
            spaces * glue.word_shrink + letters * glue.letter_shrink + chars * glue.glyph_shrink,
        )
    };
    // Trailing spaces are dropped at a break.
    let trim = |mut b: usize, a: usize| {
        while b > a && g.get(b - 1).is_some_and(SGlyph::is_space) {
            b -= 1;
        }
        b
    };
    let mut all: Vec<Breakpoint> = cands.iter().copied().filter(|c| c.end > 0 && c.end < n).collect();
    all.push(Breakpoint { end: n, hyphen: 0.0, auto: false });
    let (adj_demerits, hyphen_penalty) =
        if p.ragged { (ADJ_DEMERITS_RAGGED, HYPHEN_PENALTY_RAGGED) } else { (ADJ_DEMERITS_JUSTIFIED, HYPHEN_PENALTY) };
    let mut nodes = vec![Node { pos: 0, line: 0, fitness: 1, demerits: 0.0, hyphenated: false, run: 0, prev: None }];
    let mut active: Vec<usize> = vec![0];
    for bp in &all {
        let last = bp.end == n;
        let mut best: Vec<(usize, u8, usize, f64, usize)> = vec![]; // (line, fitness, run, demerits, from)
        let mut keep = Vec::with_capacity(active.len());
        for &ai in &active {
            let Some(a) = nodes.get(ai) else { continue };
            // Hyphen Limit: this line would be one too many in a row ending in a hyphen.
            let refused = bp.auto && bp.hyphen > 0.0 && p.hyphen_limit > 0 && a.run >= p.hyphen_limit;
            let t = trim(bp.end, a.pos);
            let credit = start_credit(a.pos) + if t > a.pos { end_credit(t, bp.hyphen > 0.0) } else { 0.0 };
            let natural = w[t] - w[a.pos] - credit + bp.hyphen;
            let lw = width(a.line);
            // Adjustment ratio, badness and whether the line ends the active node's reach.
            let ratio = if p.ragged {
                if natural > lw + EPS {
                    // Ragged lines never shrink; longer lines from this node only get worse.
                    continue;
                }
                if last && !p.justify_last {
                    ((lw * SHORT_LAST - natural) / (lw * SHORT_LAST_ZONE).max(1.0)).max(0.0)
                } else {
                    (lw - natural).max(0.0) / (lw * RAG_ZONE).max(1.0)
                }
            } else {
                let (stretch, shrink) = give(a.pos, t);
                if natural < lw {
                    if last && !p.justify_last {
                        0.0
                    } else if stretch > 0.0 {
                        (lw - natural) / stretch
                    } else {
                        f64::INFINITY
                    }
                } else if natural > lw {
                    if shrink > 0.0 { (lw - natural) / shrink } else { f64::NEG_INFINITY }
                } else {
                    0.0
                }
            };
            if ratio < -1.0 {
                // Too long already; longer lines from this node only get worse.
                continue;
            }
            keep.push(ai);
            if refused {
                continue;
            }
            let free_last = last && (p.ragged || !p.justify_last);
            if ratio > p.tolerance && !(last && (p.ragged || (ratio.is_infinite() && p.justify_last))) {
                continue;
            }
            let badness = if ratio.is_finite() { (100.0 * ratio.abs().powi(3)).min(10_000.0) } else { 10_000.0 };
            let penalty = if bp.hyphen > 0.0 { hyphen_penalty } else { 0.0 };
            let mut d = (LINE_PENALTY + badness).powi(2) + penalty * penalty;
            if bp.hyphen > 0.0 && a.hyphenated {
                d += DOUBLE_HYPHEN_DEMERITS;
            }
            // A ragged last line's length says nothing about the rag: it keeps its neighbour's class.
            let fit = if p.ragged && free_last { a.fitness } else { fitness(ratio, p.ragged) };
            if a.prev.is_some() && fit.abs_diff(a.fitness) > 1 {
                d += adj_demerits;
            }
            let total = a.demerits + d;
            let line = (a.line + 1).min(p.uniform_from);
            // With a Hyphen Limit, breaks after different runs of hyphens lead to different
            // choices later, so they are kept apart.
            let run = if bp.hyphen > 0.0 { a.run + 1 } else { 0 };
            let key = if p.hyphen_limit > 0 { run.min(p.hyphen_limit) } else { 0 };
            match best.iter_mut().find(|b| b.0 == line && b.1 == fit && b.2 == key) {
                Some(b) if b.3 <= total => {}
                Some(b) => *b = (line, fit, key, total, ai),
                None => best.push((line, fit, key, total, ai)),
            }
        }
        active = keep;
        for (line, fitness, run, demerits, from) in best {
            nodes.push(Node { pos: bp.end, line, fitness, demerits, hyphenated: bp.hyphen > 0.0, run, prev: Some(from) });
            active.push(nodes.len() - 1);
        }
        if active.is_empty() || active.len() > MAX_ACTIVE {
            return None;
        }
    }
    let end = nodes.iter().enumerate().filter(|(_, nd)| nd.pos == n && nd.prev.is_some()).min_by(|a, b| a.1.demerits.total_cmp(&b.1.demerits))?.0;
    let mut out = vec![];
    let mut cur = Some(end);
    while let Some(i) = cur {
        let nd = nodes.get(i)?;
        if nd.prev.is_some() {
            out.push((nd.pos, nd.hyphenated));
        }
        cur = nd.prev;
        if out.len() > n {
            return None;
        }
    }
    out.reverse();
    Some(out)
}
