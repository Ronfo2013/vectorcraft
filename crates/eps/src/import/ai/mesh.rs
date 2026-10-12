//! Gradient meshes in the editing data: `/Mesh X!`, then `X#` entries, `/End X!`.
//!
//! The object's entries, each `values /Key X#` (on `%_` lines): `/Version`, `/Type` (`/Cartesian`),
//! `[cols rows] /Size`, `/Data`, then one block per patch: `[col row] /P`, `[t] /R` (its place
//! along the mesh), a `/CS` colour space when it changes (`/DeviceRGB`, `/DeviceGray`,
//! `/DeviceCMYK`; RGB until one is given), the patch's boundary points in order, `/E`. A boundary
//! point is a corner, `[colour… 1 x y hx hy 1 hx hy 1] /N` (the colour's components and a 1, the
//! point, its handle towards the next point, its handle towards the previous one), or a point on
//! an edge between two corners, `[x y hx hy hx hy 1] /S` (the point, the handle towards the
//! previous point, the handle towards the next). The boundary runs corner 0 → corner 1 → corner 2
//! → corner 3 → corner 0; the points of the last edge (corner 3 → corner 0) are listed first, from
//! corner 0's end, so they read in reverse and with their handles the other way round.
//!
//! This was established on a user's Illustrator 30 file (used locally) by matching every corner,
//! handle and colour against the PDF part's own drawing of the same meshes (tensor-product patch
//! shadings, ISO 32000-1 §8.7.4.5.8), which Illustrator writes with each patch's edges split at
//! the `/S` points. Coordinates are art space (y up). The 1 after each handle isn't read. The 1
//! after a corner's colour was 1 on every corner of that file and its PDF part paints the meshes
//! opaque, so what else it can be isn't known: a corner where it isn't 1 is read opaque and
//! counted ([`MeshRead::odd_opacity`]), for a note that says so.
//!
//! A patch without edge points is one Coons patch ([`GradientMesh::coons`]); with them, it is a
//! grid of Coons patches split where the edge points are: its boundary is the file's own cubics
//! (split exactly where the grid's lines from the opposite edge meet it), its inner lines follow
//! the Coons surface of that boundary ([`GradientMesh::from_grid`]), its colours are bilinear
//! between the corners'.

use std::collections::BTreeMap;

use kurbo::{CubicBez, ParamCurve, Point};
use vectorcraft_color::Color;
use vectorcraft_doc::live::{GradientMesh, H_DOWN, H_LEFT, H_RIGHT, H_UP, lerp_color};

use super::obj::V;

/// Most patches read from one mesh.
const MAX_PATCHES: usize = 4096;
/// Most points on one patch's boundary.
const MAX_BOUNDARY: usize = 256;
/// Most points in the grid one patch becomes (its edge points can make it fine).
pub(super) const MAX_GRID_POINTS: usize = 4096;
/// Most mesh points read from one file, all meshes together (each point is about a hundred bytes).
pub const MAX_MESH_POINTS: usize = 1 << 20;

/// What a patch that isn't read is counted as in the note on what was left out.
const PATCH: &str = "Mesh patch";

/// A point on a patch's boundary: its handles are absolute points.
#[derive(Clone, Copy, Debug)]
struct Item {
    p: Point,
    /// A corner's handle towards the next point; an edge point's towards the previous.
    h1: Point,
    /// A corner's handle towards the previous point; an edge point's towards the next.
    h2: Point,
    /// A corner's colour (an edge point has none).
    color: Option<Color>,
}

/// The colour space of the corners that follow.
#[derive(Clone, Debug)]
enum Space {
    /// Read: its number of components (RGB 3, gray 1, CMYK 4).
    Known(usize),
    /// Not read: its name, for the note.
    Unknown(String),
}

/// The meshes of a mesh object, and what of it wasn't read.
#[derive(Debug, Default)]
pub(super) struct MeshRead {
    /// One 1 × 1 or finer mesh per patch, in art space.
    pub meshes: Vec<GradientMesh>,
    /// Patches left out, by why (`Mesh patch`, `Mesh patch (DeviceN colour)` …).
    pub skipped: BTreeMap<String, usize>,
    /// Corners whose value after the colour isn't 1 (read as opaque).
    pub odd_opacity: usize,
}

/// Reads a mesh object's entries into its patches.
#[derive(Debug)]
pub(super) struct Builder {
    space: Space,
    current: Option<Vec<Item>>,
    /// Why the current patch can't be read, if it can't.
    bad: Option<String>,
    patches: Vec<Vec<Item>>,
    read: MeshRead,
}

impl Default for Builder {
    fn default() -> Self {
        Self { space: Space::Known(3), current: None, bad: None, patches: vec![], read: MeshRead::default() }
    }
}

/// The name of a colour space operand: a name, or an array's first name (`[/Separation …]`).
fn space_name(vals: &[V]) -> String {
    vals.iter()
        .rev()
        .skip(1)
        .find_map(|v| match v {
            V::Name(n) => Some(n.clone()),
            V::Arr(a) => a.iter().find_map(|x| if let V::Name(n) = x { Some(n.clone()) } else { None }),
            _ => None,
        })
        .unwrap_or_else(|| "unnamed".into())
}

impl Builder {
    /// An `X#` entry: its operands, the key last.
    pub fn entry(&mut self, vals: &[V]) {
        let Some(V::Name(key)) = vals.last() else { return };
        let nums: Vec<f64> = match vals.iter().find(|v| matches!(v, V::Arr(_))) {
            Some(V::Arr(a)) => a.iter().filter_map(V::num).collect(),
            _ => vals.iter().filter_map(V::num).collect(),
        };
        match key.as_str() {
            "CS" => {
                let name = space_name(vals);
                self.space = match name.as_str() {
                    "DeviceRGB" => Space::Known(3),
                    "DeviceGray" => Space::Known(1),
                    "DeviceCMYK" => Space::Known(4),
                    _ => Space::Unknown(name.chars().take(32).collect()),
                };
            }
            "P" => {
                self.close(false);
                self.current = Some(Vec::new());
                self.bad = None;
            }
            "N" => {
                let comps = match &self.space {
                    Space::Known(n) => *n,
                    Space::Unknown(name) => {
                        self.fail(format!("{PATCH} ({name} colour)"));
                        return;
                    }
                };
                let color = match (comps, nums.as_slice()) {
                    (3, [r, g, b, ..]) => Some(Color::rgb(*r as f32, *g as f32, *b as f32)),
                    // A gray is its lightness here, an amount of ink in the document.
                    (1, [v, ..]) => Some(Color::gray(1.0 - *v as f32)),
                    (4, [c, m, y, k, ..]) => Some(Color::cmyk(*c as f32, *m as f32, *y as f32, *k as f32)),
                    _ => None,
                };
                let Some(color) = color.filter(|_| nums.get(..comps).is_some_and(|c| c.iter().all(|v| v.is_finite()))) else {
                    self.fail(PATCH.into());
                    return;
                };
                // After the colour, its 1, then the point and the two handles, each with a 1.
                if nums.get(comps).is_some_and(|v| *v != 1.0) {
                    self.read.odd_opacity += 1;
                }
                match nums.get(comps + 1..) {
                    Some([x, y, h1x, h1y, _, h2x, h2y, _, ..]) => {
                        self.push(Item { p: Point::new(*x, *y), h1: Point::new(*h1x, *h1y), h2: Point::new(*h2x, *h2y), color: Some(color) });
                    }
                    _ => self.fail(PATCH.into()),
                }
            }
            "S" => match nums.as_slice() {
                [x, y, h1x, h1y, h2x, h2y, ..] => {
                    self.push(Item { p: Point::new(*x, *y), h1: Point::new(*h1x, *h1y), h2: Point::new(*h2x, *h2y), color: None });
                }
                _ => self.fail(PATCH.into()),
            },
            "E" => self.close(true),
            _ => {}
        }
    }

    /// The current patch can't be read (the first reason is kept).
    fn fail(&mut self, why: String) {
        if self.current.is_some() && self.bad.is_none() {
            self.bad = Some(why);
        }
    }

    fn push(&mut self, item: Item) {
        let finite = [item.p, item.h1, item.h2].iter().all(|p| p.is_finite());
        match self.current.as_mut() {
            Some(items) if finite && items.len() < MAX_BOUNDARY => items.push(item),
            _ => self.fail(PATCH.into()),
        }
    }

    fn skip(&mut self, why: String) {
        *self.read.skipped.entry(why).or_default() += 1;
    }

    /// The current patch ends (`ended`: by its `/E`; else another began, or the object ended).
    fn close(&mut self, ended: bool) {
        let Some(items) = self.current.take() else { return };
        let corners = items.iter().filter(|i| i.color.is_some()).count();
        match self.bad.take() {
            Some(why) => self.skip(why),
            None if !ended || corners != 4 || self.patches.len() >= MAX_PATCHES => self.skip(PATCH.into()),
            None => self.patches.push(items),
        }
    }

    /// The meshes of the patches read, in art space. A patch whose grid would take more points
    /// than [`MAX_GRID_POINTS`] or than `budget` has left (the file's, [`MAX_MESH_POINTS`]), or
    /// whose geometry can't be built, is left out and counted; `budget` loses the points used.
    pub fn finish(mut self, budget: &mut usize) -> MeshRead {
        self.close(false);
        for items in std::mem::take(&mut self.patches) {
            match patch_mesh(&items, (*budget).min(MAX_GRID_POINTS)) {
                Ok(m) => {
                    *budget = budget.saturating_sub(m.points.len());
                    self.read.meshes.push(m);
                }
                Err(why) => self.skip(why),
            }
        }
        self.read
    }
}

/// Is every point and handle of `m` finite (and its grid whole)?
pub(super) fn sound(m: &GradientMesh) -> bool {
    m.is_valid() && m.points.iter().all(|p| p.p.is_finite() && p.handles.iter().all(|h| h.is_finite()))
}

/// An edge of a patch between two corners: its cubic segments in order, each an equal share of
/// the edge's parameter.
struct Edge {
    segs: Vec<CubicBez>,
}

impl Edge {
    /// The segment `t` (0 to 1 over the whole edge) falls in, and where in it.
    fn locate(&self, t: f64) -> (usize, f64) {
        let n = self.segs.len().max(1) as f64;
        let k = ((t * n).floor().max(0.0) as usize).min(self.segs.len().saturating_sub(1));
        (k, (t * n - k as f64).clamp(0.0, 1.0))
    }

    /// The point at `t`.
    fn eval(&self, t: f64) -> Point {
        let (k, local) = self.locate(t);
        self.segs.get(k).map_or(Point::ZERO, |s| s.eval(local))
    }

    /// The edge between `a` and `b` (`a` < `b`, both in one segment: the grid's lines include
    /// every segment end), exactly: that part of the file's cubic.
    fn part(&self, a: f64, b: f64) -> Option<CubicBez> {
        // The segment from the middle: `a` and `b` may be a hair past the segment's ends (the
        // grid's lines of an edge read backwards are `1 - t`).
        let (k, _) = self.locate((a + b) / 2.0);
        let n = self.segs.len().max(1) as f64;
        let (ta, tb) = ((a * n - k as f64).clamp(0.0, 1.0), (b * n - k as f64).clamp(0.0, 1.0));
        Some(self.segs.get(k)?.subsegment(ta..tb))
    }

    /// Where its segments meet, as shares of the edge (0 and 1 included).
    fn breaks(&self) -> impl Iterator<Item = f64> + '_ {
        let n = self.segs.len().max(1);
        (0..=n).map(move |k| k as f64 / n as f64)
    }
}

/// The patch `items` (as listed) as a gradient mesh, of at most `max_points` points: one Coons
/// patch, or a grid of them where its edges have points between the corners. Why not, when not.
fn patch_mesh(items: &[Item], max_points: usize) -> Result<GradientMesh, String> {
    let unread = || PATCH.to_string();
    let first = items.iter().position(|i| i.color.is_some()).ok_or_else(unread)?;
    // The boundary from corner 0 on: the points listed before it are the last edge's, from
    // corner 0's end: reversed, their handles the other way round.
    let mut cycle: Vec<Item> = items.get(first..).ok_or_else(unread)?.to_vec();
    cycle.extend(items.get(..first).unwrap_or_default().iter().rev().map(|i| Item { h1: i.h2, h2: i.h1, ..*i }));
    let corners: Vec<usize> = cycle.iter().enumerate().filter(|(_, i)| i.color.is_some()).map(|(k, _)| k).collect();
    let [c0, c1, c2, c3] = corners.as_slice() else { return Err(unread()) };
    let colors = [c0, c1, c2, c3].map(|&k| cycle.get(k).and_then(|i| i.color).unwrap_or(Color::BLACK));
    // Each edge: corner, its edge points, the next corner (the last edge wraps to corner 0).
    let edge = |from: usize, to: usize| -> Option<Edge> {
        let pts: Vec<&Item> =
            if to > from { cycle.get(from..=to)?.iter().collect() } else { cycle.get(from..)?.iter().chain(cycle.get(..=to)?.iter()).collect() };
        let segs = pts
            .windows(2)
            .filter_map(|w| {
                let (a, b) = (w.first()?, w.get(1)?);
                let out = if a.color.is_some() { a.h1 } else { a.h2 };
                let into = if b.color.is_some() { b.h2 } else { b.h1 };
                Some(CubicBez::new(a.p, out, into, b.p))
            })
            .collect();
        Some(Edge { segs })
    };
    let edges = [edge(*c0, *c1), edge(*c1, *c2), edge(*c2, *c3), edge(*c3, *c0)];
    let [Some(e0), Some(e1), Some(e2), Some(e3)] = edges else { return Err(unread()) };
    if [&e0, &e1, &e2, &e3].iter().all(|e| e.segs.len() == 1) {
        let s = |e: &Edge| e.segs.first().copied();
        let (Some(a), Some(b), Some(c), Some(d)) = (s(&e0), s(&e1), s(&e2), s(&e3)) else { return Err(unread()) };
        let cp = [a.p0, a.p1, a.p2, a.p3, b.p1, b.p2, b.p3, c.p1, c.p2, c.p3, d.p1, d.p2];
        return GradientMesh::coons(&cp, colors).filter(sound).ok_or_else(unread);
    }
    // The grid: u runs along edge 3 backwards (corner 0 → 3) at v = 0 and along edge 1 (corner 1
    // → 2) at v = 1; v runs along edge 0 (corner 0 → 1) at u = 0 and along edge 2 backwards
    // (corner 3 → 2) at u = 1. Its lines are at every segment end of the two opposite edges.
    let params = |a: &Edge, a_backwards: bool, b: &Edge, b_backwards: bool| -> Vec<f64> {
        let flip = |back: bool| move |t: f64| if back { 1.0 - t } else { t };
        let mut all: Vec<f64> = a.breaks().map(flip(a_backwards)).chain(b.breaks().map(flip(b_backwards))).collect();
        all.sort_by(f64::total_cmp);
        all.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        all
    };
    let (us, vs) = (params(&e3, true, &e1, false), params(&e0, false, &e2, true));
    if us.len().saturating_mul(vs.len()) > max_points {
        return Err(format!("{PATCH} (over the size limit)"));
    }
    let [p0, p1, p2, p3] = [c0, c1, c2, c3].map(|&k| cycle.get(k).map_or(Point::ZERO, |i| i.p));
    // The Coons surface of the boundary, for the inner lines.
    let surface = |u: f64, v: f64| -> Point {
        let (l, r, b, t) = (e0.eval(v), e2.eval(1.0 - v), e3.eval(1.0 - u), e1.eval(u));
        let bilinear =
            p0.to_vec2() * ((1.0 - u) * (1.0 - v)) + p1.to_vec2() * ((1.0 - u) * v) + p2.to_vec2() * (u * v) + p3.to_vec2() * (u * (1.0 - v));
        Point::ZERO + l.to_vec2() * (1.0 - u) + r.to_vec2() * u + b.to_vec2() * (1.0 - v) + t.to_vec2() * v - bilinear
    };
    let color = |r: usize, c: usize, _: Point| -> (Color, f32) {
        let (u, v) = (us.get(c).copied().unwrap_or(0.0) as f32, vs.get(r).copied().unwrap_or(0.0) as f32);
        let bottom = lerp_color(&colors[0], &colors[3], u);
        let top = lerp_color(&colors[1], &colors[2], u);
        (lerp_color(&bottom, &top, v), 1.0)
    };
    let mut m = GradientMesh::from_grid(&us, &vs, &surface, &color);
    // The boundary is the file's own cubics, split where the grid's lines meet it.
    let (rows, cols) = (vs.len().saturating_sub(1), us.len().saturating_sub(1));
    let mut set = |r: usize, c: usize, h: usize, at: Point, to: Point| {
        let i = m.idx(r, c);
        if let Some(pt) = m.points.get_mut(i) {
            pt.p = at;
            if let Some(slot) = pt.handles.get_mut(h) {
                *slot = to - at;
            }
        }
    };
    for (c, w) in us.windows(2).enumerate() {
        let (Some(&ua), Some(&ub)) = (w.first(), w.get(1)) else { continue };
        // Bottom (v = 0): edge 3 backwards.
        if let Some(b) = e3.part(1.0 - ub, 1.0 - ua) {
            set(0, c, H_RIGHT, b.p3, b.p2);
            set(0, c + 1, H_LEFT, b.p0, b.p1);
        }
        // Top (v = 1): edge 1.
        if let Some(t) = e1.part(ua, ub) {
            set(rows, c, H_RIGHT, t.p0, t.p1);
            set(rows, c + 1, H_LEFT, t.p3, t.p2);
        }
    }
    for (r, w) in vs.windows(2).enumerate() {
        let (Some(&va), Some(&vb)) = (w.first(), w.get(1)) else { continue };
        // Left (u = 0): edge 0.
        if let Some(l) = e0.part(va, vb) {
            set(r, 0, H_DOWN, l.p0, l.p1);
            set(r + 1, 0, H_UP, l.p3, l.p2);
        }
        // Right (u = 1): edge 2 backwards.
        if let Some(rt) = e2.part(1.0 - vb, 1.0 - va) {
            set(r, cols, H_DOWN, rt.p3, rt.p2);
            set(r + 1, cols, H_UP, rt.p0, rt.p1);
        }
    }
    if sound(&m) { Ok(m) } else { Err(unread()) }
}
