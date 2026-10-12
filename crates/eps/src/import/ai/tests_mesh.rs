//! Gradient meshes from the editing data (`mesh`), on fixtures written from the structure that
//! module documents.

use kurbo::Point;
use vectorcraft_color::Color;
use vectorcraft_doc::live::{GradientMesh, H_DOWN, H_LEFT, H_RIGHT, H_UP};
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

/// The list in the note on objects left out (`Mesh: 1, Mesh patch: 2`), if there is one.
fn left_out(warnings: &[String]) -> Option<&str> {
    warnings.iter().find_map(|w| w.strip_prefix(read::OBJECTS_LEFT_OUT)?.strip_prefix(" (")?.strip_suffix(')'))
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
    assert_eq!(left_out(&s.warnings), Some("Mesh patch: 2"), "{:?}", s.warnings);
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceRGB", three)));
    assert_eq!(left_out(&s.warnings), Some("Mesh: 1, Mesh patch: 1"), "{:?}", s.warnings);
    // An unknown colour space: its corners can't be read, and the note names it.
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceN", &good)));
    assert_eq!(left_out(&s.warnings), Some("Mesh: 1, Mesh patch (DeviceN colour): 1"), "{:?}", s.warnings);
    // Another kind of object.
    let s = read_ok(&layer(&format!("{}/Blob X!\n%_[1 2] /P X#\n/End X!\n", rect_path(0.0, 0.0, 1.0, 1.0) + "f\n")));
    assert_eq!(left_out(&s.warnings), Some("Blob: 1"), "{:?}", s.warnings);
}

/// The corners of a mesh with no `/CS` are RGB, as are those after a `/DeviceRGB`. A colour space
/// this doesn't read names itself in the note, and the patches after a later one it reads are read
/// again.
#[test]
fn no_colour_space_is_rgb_and_one_it_doesnt_read_is_named() {
    let patch = |c: u32, color: &str| square_patch((c, 0), 10.0 + 30.0 * f64::from(c), 10.0, 30.0, [color; 4]);
    let object = format!(
        "/Mesh X!\n%_[3 1] /Size X#\n%_/Data X#\n{}%_/DeviceN /CS X#\n{}%_/DeviceGray /CS X#\n{}/End X!\n",
        patch(0, "1 0 0"),
        patch(1, "1 0 0"),
        patch(2, "0.25")
    );
    let s = read_ok(&layer(&object));
    let ms = meshes(&s.doc);
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[0].points[0].color, Color::rgb(1.0, 0.0, 0.0), "RGB without a /CS");
    assert!(matches!(ms[1].points[0].color, Color::Gray { k } if (k - 0.75).abs() < 1e-6), "{:?}", ms[1].points[0].color);
    assert_eq!(left_out(&s.warnings), Some("Mesh patch (DeviceN colour): 1"), "{:?}", s.warnings);
    // A colour space written as an array names itself by its family.
    let separation = mesh_object((1, 1), "Separation", &patch(0, "0.5")).replace("%_/Separation /CS", "%_[/Separation (Gold) /DeviceCMYK {}] /CS");
    let s = read_ok(&layer(&separation));
    assert_eq!(left_out(&s.warnings), Some("Mesh: 1, Mesh patch (Separation colour): 1"), "{:?}", s.warnings);
}

/// A mesh outside the layers (where no art can go) is left out with the note, as an object this
/// doesn't read was before meshes were read, and the file goes on.
#[test]
fn a_mesh_outside_the_layers_is_left_out() {
    let object = mesh_object((1, 1), "DeviceRGB", &square_patch((0, 0), 10.0, 10.0, 30.0, ["1 0 0"; 4]));
    let s = read_ok(&format!("{object}{}", layer(&(rect_path(0.0, 0.0, 1.0, 1.0) + "f\n"))));
    assert_eq!(s.doc.layers.len(), 1);
    assert_eq!(left_out(&s.warnings), Some("Mesh: 1"), "{:?}", s.warnings);
}

/// The value after a corner's colour was 1 on every corner seen; one that isn't is read opaque, and
/// a note says so (a loss: it may be the point's opacity).
#[test]
fn a_corner_value_other_than_one_is_read_opaque_with_a_note() {
    let patch = square_patch((0, 0), 10.0, 10.0, 30.0, ["1 0 0"; 4]).replacen("1 0 0 1 ", "1 0 0 0.5 ", 1);
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceRGB", &patch)));
    let ms = meshes(&s.doc);
    assert_eq!(ms.len(), 1);
    assert!(ms[0].points.iter().all(|p| p.opacity == 1.0));
    let note = s.warnings.iter().find(|w| w.starts_with(read::MESH_OPACITY_UNREAD)).unwrap_or_else(|| panic!("{:?}", s.warnings));
    assert!(note.ends_with("(1)") && crate::is_loss(note), "{note}");
    assert_eq!(left_out(&s.warnings), None, "{:?}", s.warnings);
}

/// A patch whose edge points would make a grid of more than [`mesh::MAX_GRID_POINTS`] points is
/// left out and counted, and the reader goes on.
#[test]
fn a_patch_past_the_size_limit_is_left_out() {
    // 100 points on the last edge (listed before corner 0) and 100 on the first (after it): a grid
    // of 102 × 102 points.
    let bottom: String = (1..=100).map(|i| edge_point((40.0 - 0.29 * f64::from(i), 10.0), (10.0, 10.0), (40.0, 10.0))).collect();
    let left: String = (1..=100).map(|i| edge_point((10.0, 10.0 + 0.29 * f64::from(i)), (10.0, 10.0), (10.0, 40.0))).collect();
    let big = format!(
        "%_[0 0] /P X#\n%_[0] /R X#\n{bottom}{}{left}{}{}{}%_/E X#\n",
        corner("1 0 0", (10.0, 10.0), (10.0, 20.0), (20.0, 10.0)),
        corner("1 0 0", (10.0, 40.0), (20.0, 40.0), (10.0, 30.0)),
        corner("1 0 0", (40.0, 40.0), (40.0, 30.0), (30.0, 40.0)),
        corner("1 0 0", (40.0, 10.0), (30.0, 10.0), (40.0, 20.0)),
    );
    const { assert!(102 * 102 > mesh::MAX_GRID_POINTS) };
    let small = square_patch((1, 0), 40.0, 10.0, 30.0, ["1 0 0"; 4]);
    let s = read_ok(&layer(&mesh_object((2, 1), "DeviceRGB", &format!("{big}{small}"))));
    assert_eq!(meshes(&s.doc).len(), 1, "the small patch is read");
    assert_eq!(left_out(&s.warnings), Some("Mesh patch (over the size limit): 1"), "{:?}", s.warnings);
}

/// The file's mesh points are capped ([`mesh::MAX_MESH_POINTS`]): patches past it are left out and
/// counted, and the reader goes on.
#[test]
fn the_files_mesh_points_are_capped() {
    // Each patch: 62 points on its last edge and 62 on its first, a grid of 64 × 64 points, as
    // fine as one patch may be.
    let side: u32 = 62;
    assert_eq!(((side + 2) * (side + 2)) as usize, mesh::MAX_GRID_POINTS);
    let bottom: String = (1..=side).map(|i| edge_point((40.0 - 0.45 * f64::from(i), 10.0), (10.0, 10.0), (40.0, 10.0))).collect();
    let left: String = (1..=side).map(|i| edge_point((10.0, 10.0 + 0.45 * f64::from(i)), (10.0, 10.0), (10.0, 40.0))).collect();
    let patch = format!(
        "%_[0 0] /P X#\n%_[0] /R X#\n{bottom}{}{left}{}{}{}%_/E X#\n",
        corner("1 0 0", (10.0, 10.0), (10.0, 20.0), (20.0, 10.0)),
        corner("1 0 0", (10.0, 40.0), (20.0, 40.0), (10.0, 30.0)),
        corner("1 0 0", (40.0, 40.0), (40.0, 30.0), (30.0, 40.0)),
        corner("1 0 0", (40.0, 10.0), (30.0, 10.0), (40.0, 20.0)),
    );
    let fit = mesh::MAX_MESH_POINTS / mesh::MAX_GRID_POINTS;
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceRGB", &patch.repeat(fit + 4))));
    let ms = meshes(&s.doc);
    assert_eq!(ms.len(), fit);
    assert_eq!(ms.iter().map(|m| m.points.len()).sum::<usize>(), mesh::MAX_MESH_POINTS);
    assert_eq!(left_out(&s.warnings), Some("Mesh patch (over the size limit): 4"), "{:?}", s.warnings);
}

/// A split patch's boundary is the file's own cubics: an edge point where the edge turns (a cusp,
/// its two handles in different directions) keeps both handles as they are, and a segment much
/// shorter than its neighbour keeps its shape; the edge across from it is split exactly where the
/// grid's line meets it.
#[test]
fn split_patches_keep_the_files_own_boundary_curves() {
    // A 30-point square at (10, 10). Its top edge (corner 1 (10, 40) → corner 2 (40, 40)) has an
    // edge point at (15, 40), 5 points along a 30-point edge: its handle back goes up and left to
    // (12, 45), its handle on up and right to (25, 50).
    let patch = format!(
        "%_[0 0] /P X#\n%_[0] /R X#\n{}{}{}{}{}%_/E X#\n",
        corner("1 0 0", (10.0, 10.0), (10.0, 20.0), (20.0, 10.0)),
        corner("1 0 0", (10.0, 40.0), (12.0, 40.0), (10.0, 30.0)),
        edge_point((15.0, 40.0), (12.0, 45.0), (25.0, 50.0)),
        corner("1 0 0", (40.0, 40.0), (40.0, 30.0), (35.0, 40.0)),
        corner("1 0 0", (40.0, 10.0), (30.0, 10.0), (40.0, 20.0)),
    );
    let s = read_ok(&layer(&mesh_object((1, 1), "DeviceRGB", &patch)));
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    let ms = meshes(&s.doc);
    let m = &ms[0];
    assert_eq!((m.rows, m.cols), (1, 2));
    let h = |v: kurbo::Vec2| (v.x, v.y);
    let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
    // The top row (v = 1, row 1) on the page: y = 100 - y.
    let cusp = &m.points[m.idx(1, 1)];
    assert!(near(cusp.p, (15.0, 60.0)), "{:?}", cusp.p);
    assert!(close(h(cusp.handles[H_LEFT]), (-3.0, -5.0)) && close(h(cusp.handles[H_RIGHT]), (10.0, -10.0)), "{:?}", cusp.handles);
    assert!(close(h(m.points[m.idx(1, 0)].handles[H_RIGHT]), (2.0, 0.0)), "{:?}", m.points[m.idx(1, 0)].handles);
    assert!(close(h(m.points[m.idx(1, 2)].handles[H_LEFT]), (-5.0, 0.0)), "{:?}", m.points[m.idx(1, 2)].handles);
    // The short segment is the file's cubic: its middle is where the file's has it.
    let file = kurbo::CubicBez::new((10.0, 40.0), (12.0, 40.0), (12.0, 45.0), (15.0, 40.0));
    let mid = kurbo::ParamCurve::eval(&file, 0.5);
    let got = kurbo::ParamCurve::eval(&m.edge_u(1, 0), 0.5);
    assert!(near(got, (mid.x, 100.0 - mid.y)), "{got:?} {mid:?}");
    // The bottom edge, straight, is split at its middle (u = 1/2, the top's one edge point): the
    // halves are straight, their handles a third of their length.
    let b = &m.points[m.idx(0, 1)];
    assert!(near(b.p, (25.0, 90.0)), "{:?}", b.p);
    assert!(close(h(b.handles[H_LEFT]), (-5.0, 0.0)) && close(h(b.handles[H_RIGHT]), (5.0, 0.0)), "{:?}", b.handles);
}
