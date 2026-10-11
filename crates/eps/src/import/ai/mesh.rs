//! Gradient meshes in the editing data: `/Mesh X!`, then `X#` entries, `/End X!`.
//!
//! The object's entries, each `values /Key X#` (on `%_` lines): `/Version`, `/Type` (`/Cartesian`),
//! `[cols rows] /Size`, `/Data`, then one block per patch: `[col row] /P`, `[t] /R` (its place
//! along the mesh), a `/CS` colour space when it changes (`/DeviceRGB`, `/DeviceGray`,
//! `/DeviceCMYK`), the patch's boundary points in order, `/E`. A boundary point is a corner,
//! `[colour… x y hx hy 1 hx hy 1] /N` (the colour's components and a 1, the point, its handle
//! towards the next point, its handle towards the previous one), or a point on an edge between
//! two corners, `[x y hx hy hx hy 1] /S` (the point, the handle towards the previous point, the
//! handle towards the next). The boundary runs corner 0 → corner 1 → corner 2 → corner 3 → corner
//! 0; the points of the last edge (corner 3 → corner 0) are listed first, from corner 0's end, so
//! they read in reverse and with their handles the other way round.
//!
//! This was established on a user's Illustrator 30 file (used locally) by matching every corner,
//! handle and colour against the PDF part's own drawing of the same meshes (tensor-product patch
//! shadings, ISO 32000-1 §8.7.4.5.8), which Illustrator writes with each patch's edges split at
//! the `/S` points. Coordinates are art space (y up); the `1` after each colour and handle isn't
//! read (it was always 1).
//!
//! A patch without edge points is one Coons patch ([`GradientMesh::coons`]); with them, its
//! boundary edges are piecewise cubic and the patch is a grid of Coons patches over the Coons
//! surface of that boundary, split where the edge points are ([`GradientMesh::from_grid`]), its
//! colours bilinear between the corners'.

use kurbo::{CubicBez, ParamCurve, Point};
use vectorcraft_color::Color;
use vectorcraft_doc::live::{GradientMesh, lerp_color};

use super::obj::V;

/// Most patches read from one mesh.
const MAX_PATCHES: usize = 4096;
/// Most points on one patch's boundary.
const MAX_BOUNDARY: usize = 256;

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

/// Reads a mesh object's entries into its patches.
#[derive(Debug, Default)]
pub(super) struct Builder {
    /// The colour space's components (`/CS`): 3 (RGB, the default), 1 (gray) or 4 (CMYK).
    comps: Option<usize>,
    current: Option<Vec<Item>>,
    /// The current patch has an entry this can't read.
    bad: bool,
    patches: Vec<Vec<Item>>,
    /// Patches that couldn't be read (left out).
    pub skipped: usize,
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
                let space = vals.iter().rev().skip(1).find_map(|v| if let V::Name(n) = v { Some(n.as_str()) } else { None });
                self.comps = match space {
                    Some("DeviceRGB") => Some(3),
                    Some("DeviceGray") => Some(1),
                    Some("DeviceCMYK") => Some(4),
                    _ => None,
                };
            }
            "P" => {
                self.close(false);
                self.current = Some(Vec::new());
                self.bad = false;
            }
            "N" => {
                let comps = self.comps.unwrap_or(3);
                let color = match (comps, nums.as_slice()) {
                    (3, [r, g, b, ..]) => Some(Color::rgb(*r as f32, *g as f32, *b as f32)),
                    (1, [v, ..]) => Some(Color::gray(1.0 - *v as f32)),
                    (4, [c, m, y, k, ..]) => Some(Color::cmyk(*c as f32, *m as f32, *y as f32, *k as f32)),
                    _ => None,
                };
                // After the colour and its 1: the point and the two handles, each with a 1.
                let tail = nums.get(comps + 1..);
                match (color, tail) {
                    (Some(color), Some([x, y, h1x, h1y, _, h2x, h2y, _, ..])) if self.comps.is_some() => {
                        self.push(Item { p: Point::new(*x, *y), h1: Point::new(*h1x, *h1y), h2: Point::new(*h2x, *h2y), color: Some(color) });
                    }
                    _ => self.bad = true,
                }
            }
            "S" => match nums.as_slice() {
                [x, y, h1x, h1y, h2x, h2y, ..] => {
                    self.push(Item { p: Point::new(*x, *y), h1: Point::new(*h1x, *h1y), h2: Point::new(*h2x, *h2y), color: None });
                }
                _ => self.bad = true,
            },
            "E" => self.close(true),
            _ => {}
        }
    }

    fn push(&mut self, item: Item) {
        let finite = [item.p, item.h1, item.h2].iter().all(|p| p.is_finite());
        match self.current.as_mut() {
            Some(items) if finite && items.len() < MAX_BOUNDARY => items.push(item),
            _ => self.bad = true,
        }
    }

    /// The current patch ends (`ended`: by its `/E`; else another began, or the object ended).
    fn close(&mut self, ended: bool) {
        let Some(items) = self.current.take() else { return };
        let corners = items.iter().filter(|i| i.color.is_some()).count();
        if !ended || self.bad || corners != 4 || self.patches.len() >= MAX_PATCHES {
            self.skipped += 1;
            return;
        }
        self.patches.push(items);
    }

    /// The meshes of the patches read, in art space; patches whose geometry can't be built are
    /// counted in `skipped`.
    pub fn finish(mut self) -> (Vec<GradientMesh>, usize) {
        self.close(false);
        let mut out = vec![];
        for items in &self.patches {
            match patch_meshes(items) {
                Some(m) => out.push(m),
                None => self.skipped += 1,
            }
        }
        (out, self.skipped)
    }
}

/// An edge of a patch between two corners: its cubic segments in order.
struct Edge {
    segs: Vec<CubicBez>,
}

impl Edge {
    /// The point at `t` (0 to 1 over the whole edge, each segment an equal share).
    fn eval(&self, t: f64) -> Point {
        let n = self.segs.len().max(1) as f64;
        let k = ((t * n).floor() as usize).min(self.segs.len().saturating_sub(1));
        let local = (t * n - k as f64).clamp(0.0, 1.0);
        self.segs.get(k).map_or(Point::ZERO, |s| s.eval(local))
    }

    /// Where its segments meet, as shares of the edge (0 and 1 included).
    fn breaks(&self) -> impl Iterator<Item = f64> + '_ {
        let n = self.segs.len().max(1);
        (0..=n).map(move |k| k as f64 / n as f64)
    }
}

/// The patch `items` (as listed) as a gradient mesh: one Coons patch, or a grid of them where its
/// edges have points between the corners.
fn patch_meshes(items: &[Item]) -> Option<GradientMesh> {
    let first = items.iter().position(|i| i.color.is_some())?;
    // The boundary from corner 0 on: the points listed before it are the last edge's, from
    // corner 0's end: reversed, their handles the other way round.
    let mut cycle: Vec<Item> = items.get(first..)?.to_vec();
    cycle.extend(items.get(..first)?.iter().rev().map(|i| Item { h1: i.h2, h2: i.h1, ..*i }));
    let corners: Vec<usize> = cycle.iter().enumerate().filter(|(_, i)| i.color.is_some()).map(|(k, _)| k).collect();
    let [c0, c1, c2, c3] = corners.as_slice() else { return None };
    let colors = [c0, c1, c2, c3].map(|&k| cycle.get(k).and_then(|i| i.color).unwrap_or(Color::BLACK));
    // Each edge: corner, its edge points, the next corner (the last edge wraps to corner 0).
    let edge = |from: usize, to: usize| -> Option<Edge> {
        let pts: Vec<&Item> =
            if to > from { cycle.get(from..=to)?.iter().collect() } else { cycle.get(from..)?.iter().chain(cycle.get(..=to)?.iter()).collect() };
        let segs = pts
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                let out = if a.color.is_some() { a.h1 } else { a.h2 };
                let into = if b.color.is_some() { b.h2 } else { b.h1 };
                CubicBez::new(a.p, out, into, b.p)
            })
            .collect();
        Some(Edge { segs })
    };
    let edges = [edge(*c0, *c1)?, edge(*c1, *c2)?, edge(*c2, *c3)?, edge(*c3, *c0)?];
    if edges.iter().all(|e| e.segs.len() == 1) {
        let s = |i: usize| edges.get(i).and_then(|e| e.segs.first());
        let (e0, e1, e2, e3) = (s(0)?, s(1)?, s(2)?, s(3)?);
        let cp = [e0.p0, e0.p1, e0.p2, e0.p3, e1.p1, e1.p2, e1.p3, e2.p1, e2.p2, e2.p3, e3.p1, e3.p2];
        return GradientMesh::coons(&cp, colors);
    }
    // The Coons surface of the boundary: v runs along edge 0 (corner 0 → 1) at u = 0 and along
    // edge 2 backwards (corner 3 → 2) at u = 1; u runs along edge 3 backwards (corner 0 → 3) at
    // v = 0 and along edge 1 (corner 1 → 2) at v = 1.
    let [e0, e1, e2, e3] = &edges;
    let [p0, p1, p2, p3] = [c0, c1, c2, c3].map(|&k| cycle.get(k).map_or(Point::ZERO, |i| i.p));
    let surface = |u: f64, v: f64| -> Point {
        let (l, r, b, t) = (e0.eval(v), e2.eval(1.0 - v), e3.eval(1.0 - u), e1.eval(u));
        let bilinear =
            p0.to_vec2() * ((1.0 - u) * (1.0 - v)) + p1.to_vec2() * ((1.0 - u) * v) + p2.to_vec2() * (u * v) + p3.to_vec2() * (u * (1.0 - v));
        Point::ZERO + l.to_vec2() * (1.0 - u) + r.to_vec2() * u + b.to_vec2() * (1.0 - v) + t.to_vec2() * v - bilinear
    };
    let params = |a: &Edge, b: &Edge| -> Vec<f64> {
        let mut all: Vec<f64> = a.breaks().chain(b.breaks()).collect();
        all.sort_by(f64::total_cmp);
        all.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        all
    };
    let (us, vs) = (params(e3, e1), params(e0, e2));
    let color = |r: usize, c: usize, _: Point| -> (Color, f32) {
        let (u, v) = (us.get(c).copied().unwrap_or(0.0) as f32, vs.get(r).copied().unwrap_or(0.0) as f32);
        let bottom = lerp_color(&colors[0], &colors[3], u);
        let top = lerp_color(&colors[1], &colors[2], u);
        (lerp_color(&bottom, &top, v), 1.0)
    };
    let m = GradientMesh::from_grid(&us, &vs, &surface, &color);
    (m.is_valid() && m.points.iter().all(|p| p.p.is_finite() && p.handles.iter().all(|h| h.is_finite()))).then_some(m)
}
