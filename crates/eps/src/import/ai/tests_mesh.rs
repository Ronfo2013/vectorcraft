//! Gradient meshes from the editing data (`mesh`), on fixtures written from the structure that
//! module documents.

use kurbo::Point;
use vectorcraft_color::Color;
use vectorcraft_doc::live::{GradientMesh, H_DOWN, H_RIGHT, H_UP};
use vectorcraft_doc::{Document, NodeKind};

use super::tests::{layer, read_ok, rect_path, stream};
use super::*;

/// A corner entry: colour components, the point, its handle towards the next point, its handle
/// towards the previous one.
fn corner(color: &str, p: (f64, f64), next: (f64, f64), prev: (f64, f64)) -> String {
    format!("%_[{color} 1 {} {} {} {} 1 {} {} 1] /N X#\n", p.0, p.1, next.0, next.1, prev.0, prev.1)
}

/// An edge point: the point, its handle towards the previous point, its handle towards the next.
fn edge_point(p: (f64, f64), prev: (f64, f64), next: (f64, f64)) -> String {
    format!("%_[{} {} {} {} {} {} 1] /S X#\n", p.0, p.1, prev.0, prev.1, next.0, next.1)
}

/// The square patch `(col, row)` of the grid, from (`x`, `y`) with sides of `size`, straight
/// edges (handles a third of the way), no edge points, its corners `colors` (components) in the
/// order the format lists them: corner 0 at the bottom left, then clockwise (y up): corner 1 at
/// the top left, corner 2 at the top right, corner 3 at the bottom right.
fn square_patch((col, row): (u32, u32), x: f64, y: f64, size: f64, colors: [&str; 4]) -> String {
    let s = size;
    let t = s / 3.0;
    let (c0, c1, c2, c3) = ((x, y), (x, y + s), (x + s, y + s), (x + s, y));
    format!(
        "%_[{col} {row}] /P X#\n%_[0] /R X#\n{}{}{}{}%_/E X#\n",
        corner(colors[0], c0, (x, y + t), (x + t, y)),
        corner(colors[1], c1, (x + t, y + s), (x, y + s - t)),
        corner(colors[2], c2, (x + s, y + s - t), (x + s - t, y + s)),
        corner(colors[3], c3, (x + s - t, y), (x + s, y + t)),
    )
}

fn mesh_object(size: (u32, u32), cs: &str, patches: &str) -> String {
    format!(
        "/Mesh X!\n%_2 /Version X#\n%_/Cartesian /Type X#\n%_[{} {}] /Size X#\n%_/Data X#\n%_/{cs} /CS X#\n{patches}/End X!\n0 0 Xd\n6 (Mesh) XW\n",
        size.0, size.1
    )
}

/// The meshes of the first layer's first object (the mesh's group).
fn meshes(doc: &Document) -> Vec<GradientMesh> {
    let art = doc.layers.first().and_then(|l| l.children()).cloned().unwrap_or_default();
    let group = art.first().expect("the mesh's group");
    let NodeKind::Group { children, clip: false } = &group.kind else { panic!("{:?}", group.kind) };
    children
        .iter()
        .map(|n| match &n.kind {
            NodeKind::Mesh(m) => m.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn near(a: Point, b: (f64, f64)) -> bool {
    (a.x - b.0).abs() < 1e-6 && (a.y - b.1).abs() < 1e-6
}

/// A 2 × 2 mesh of square patches comes in as a group of four 1 × 1 meshes, each corner where it
/// was (art space y up → the document's y down: the crop marks are 100 high) with its colour and
/// straight-edge handles; the object is named by the markers after it, and nothing is left out.
#[test]
fn a_two_by_two_mesh_is_four_coons_patches() {
    let colors = ["1 0 0", "0 1 0", "0 0 1", "1 1 0"];
    let patches: String = [(0, 0, 10.0, 10.0), (1, 0, 40.0, 10.0), (0, 1, 10.0, 40.0), (1, 1, 40.0, 40.0)]
        .iter()
        .map(|&(c, r, x, y)| square_patch((c, r), x, y, 30.0, colors))
        .collect();
    let data = layer(&mesh_object((2, 2), "DeviceRGB", &patches));
    let s = read_ok(&data);
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    let doc = &s.doc;
    let group = &doc.layers[0].children().unwrap()[0];
    assert_eq!(group.name.as_deref(), None);
    let ms = meshes(doc);
    assert_eq!(ms.len(), 4);
    let m = &ms[0];
    assert_eq!((m.rows, m.cols), (1, 1));
    // Corner 0 at (10, 10) in art space is (10, 90) on the page; row 0 is v = 0 (corner 0 and
    // corner 3), row 1 is v = 1 (corners 1 and 2).
    assert!(near(m.points[0].p, (10.0, 90.0)) && near(m.points[1].p, (40.0, 90.0)), "{:?}", m.points);
    assert!(near(m.points[2].p, (10.0, 60.0)) && near(m.points[3].p, (40.0, 60.0)), "{:?}", m.points);
    assert_eq!(
        [m.points[0].color, m.points[2].color, m.points[3].color, m.points[1].color],
        [Color::rgb(1.0, 0.0, 0.0), Color::rgb(0.0, 1.0, 0.0), Color::rgb(0.0, 0.0, 1.0), Color::rgb(1.0, 1.0, 0.0)]
    );
    // Straight edges: the handle from corner 0 towards corner 1 (up in art space) is a third of
    // the way, down the page.
    let h = m.points[0].handles;
    assert!((h[H_DOWN].x).abs() < 1e-6 && (h[H_DOWN].y + 10.0).abs() < 1e-6, "{h:?}");
    assert!((h[H_RIGHT].x - 10.0).abs() < 1e-6 && h[H_RIGHT].y.abs() < 1e-6, "{h:?}");
    assert!(m.points[2].handles[H_UP].y > 0.0);
    // The last patch is at (40, 40): its top right corner (70, 70) is (70, 30) on the page.
    assert!(near(ms[3].points[3].p, (70.0, 30.0)), "{:?}", ms[3].points[3].p);
    // Through the import, as a `.ai` without its PDF part: no loss note.
    let (doc, notes) = crate::ai_alone(stream(&data).as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(meshes(&doc).len(), 4);
}

/// Edge points between corners (`/S`) split the patch into a grid of Coons patches where they
/// are; the points listed before corner 0 belong to the last edge, read backwards. The boundary
/// stays exact at every corner and edge point, the inside is the Coons surface, the colours are
/// bilinear between the corners'. A gray colour space gives gray corners.
#[test]
fn edge_points_split_a_patch_into_a_grid() {
    // A 30-point square at (10, 10): corner 1 → corner 2 (the top edge, u direction) has an edge
    // point at its middle; the last edge (corner 3 → corner 0, the bottom) has two, listed first,
    // from corner 0's end (at a third and two thirds), each with its handle towards corner 0's
    // side first (the handles read the other way round there).
    // Every segment's handles a third of its own length along, so each is a straight line run
    // through evenly.
    let top = edge_point((25.0, 40.0), (20.0, 40.0), (30.0, 40.0));
    let bottom = edge_point((20.0, 10.0), (16.6667, 10.0), (23.3333, 10.0)) + &edge_point((30.0, 10.0), (26.6667, 10.0), (33.3333, 10.0));
    let patch = format!(
        "%_[0 0] /P X#\n%_[0] /R X#\n{bottom}{}{}{top}{}{}%_/E X#\n",
        corner("0.2", (10.0, 10.0), (10.0, 20.0), (13.3333, 10.0)),
        corner("0.4", (10.0, 40.0), (15.0, 40.0), (10.0, 30.0)),
        corner("0.6", (40.0, 40.0), (40.0, 30.0), (35.0, 40.0)),
        corner("0.8", (40.0, 10.0), (36.6667, 10.0), (40.0, 20.0)),
    );
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceGray", &patch)));
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    let ms = meshes(&s.doc);
    assert_eq!(ms.len(), 1);
    let m = &ms[0];
    // u splits at the union of the top's (1/2) and bottom's (1/3, 2/3) breaks: 4 columns; v has
    // none: 1 row.
    assert_eq!((m.rows, m.cols), (1, 4), "{:?}", (m.rows, m.cols));
    let row0: Vec<Point> = (0..=4).map(|c| m.points[m.idx(0, c)].p).collect();
    for (p, x) in row0.iter().zip([10.0, 20.0, 25.0, 30.0, 40.0]) {
        assert!(near(*p, (x, 90.0)), "{row0:?}");
    }
    let row1: Vec<Point> = (0..=4).map(|c| m.points[m.idx(1, c)].p).collect();
    for (p, x) in row1.iter().zip([10.0, 20.0, 25.0, 30.0, 40.0]) {
        assert!(near(*p, (x, 60.0)), "{row1:?}");
    }
    // Gray: 0.2 in DeviceGray is dark (80% ink); bilinear along the bottom from 0.2 to 0.8.
    let gray = |c: usize| match m.points[m.idx(0, c)].color {
        Color::Gray { k } => k,
        other => panic!("{other:?}"),
    };
    assert!((gray(0) - 0.8).abs() < 1e-6 && (gray(4) - 0.2).abs() < 1e-6 && (gray(2) - 0.5).abs() < 1e-6, "{:?}", [gray(0), gray(2), gray(4)]);
}

/// A patch without four corners, or with an entry this can't read, is left out and counted; a
/// mesh with no patch at all is an unread object; another kind of object stays unread.
#[test]
fn patches_it_cant_read_are_counted_as_left_out() {
    let good = square_patch((0, 0), 10.0, 10.0, 30.0, ["1 0 0"; 4]);
    let three = "%_[1 0] /P X#\n%_[0] /R X#\n%_[1 0 0 1 50 10 50 20 1 60 10 1] /N X#\n%_[1 0 0 1 50 40 60 40 1 50 30 1] /N X#\n%_[1 0 0 1 80 40 80 30 1 70 40 1] /N X#\n%_/E X#\n";
    let odd = "%_[2 0] /P X#\n%_[0] /R X#\n%_[1 2 3] /N X#\n%_/E X#\n";
    let s = read_ok(&layer(&mesh_object((3, 1), "DeviceRGB", &format!("{good}{three}{odd}"))));
    assert_eq!(meshes(&s.doc).len(), 1);
    assert!(s.warnings.iter().any(|w| w.starts_with(read::OBJECTS_LEFT_OUT) && w.contains("Mesh patch: 2")), "{:?}", s.warnings);
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceRGB", three)));
    assert!(s.warnings.iter().any(|w| w.contains("Mesh: 1") && w.contains("Mesh patch: 1")), "{:?}", s.warnings);
    // An unknown colour space: its corners can't be read.
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceN", &good)));
    assert!(s.warnings.iter().any(|w| w.contains("Mesh: 1")), "{:?}", s.warnings);
    // Another kind of object.
    let s = read_ok(&layer(&format!("{}/Blob X!\n%_[1 2] /P X#\n/End X!\n", rect_path(0.0, 0.0, 1.0, 1.0) + "f\n")));
    assert!(s.warnings.iter().any(|w| w.ends_with("(Blob: 1)")), "{:?}", s.warnings);
}
