//! SVG import units: physical lengths keep their printed size, pixels and user units map 1:1 to
//! points, and the document's units follow the file's.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vectorcraft_doc::{Document, Node, NodeKind, Unit};
use vectorcraft_geom::Rect;
use vectorcraft_svg::import;

fn art(d: &Document) -> Vec<&Node> {
    let mut v = Vec::new();
    d.walk(|n| {
        if !n.is_layer() {
            v.push(n)
        }
    });
    v
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

fn close_rect(a: Rect, b: Rect) -> bool {
    close(a.x0, b.x0) && close(a.y0, b.y0) && close(a.x1, b.x1) && close(a.y1, b.y1)
}

fn text_sizes(d: &Document) -> Vec<f64> {
    art(d)
        .iter()
        .filter_map(|n| match &n.kind {
            NodeKind::Text(t) => Some(t.runs.iter().map(|r| r.style.size).collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a4_in_millimetres_opens_at_its_printed_size() {
    let d = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="210mm" height="297mm" viewBox="0 0 210 297">
        <rect x="10" y="20" width="100" height="50"/></svg>"#,
    )
    .unwrap();
    let mm = 72.0 / 25.4;
    assert!(close_rect(d.artboards[0].rect, Rect::new(0.0, 0.0, 595.28, 841.89)), "{:?}", d.artboards[0].rect);
    assert_eq!(d.units, Unit::Millimeters);
    let b = art(&d)[0].geometric_bounds().unwrap();
    assert!(close_rect(b, Rect::new(10.0 * mm, 20.0 * mm, 110.0 * mm, 70.0 * mm)), "{b:?}");
}

#[test]
fn physical_units_without_a_view_box() {
    let d = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="8.5in" height="11in">
        <rect width="1in" height="2cm" stroke="black" stroke-width="1mm"/></svg>"#,
    )
    .unwrap();
    assert!(close_rect(d.artboards[0].rect, Rect::new(0.0, 0.0, 612.0, 792.0)));
    assert_eq!(d.units, Unit::Inches);
    let n = art(&d)[0];
    assert!(close_rect(n.geometric_bounds().unwrap(), Rect::new(0.0, 0.0, 72.0, 72.0 / 2.54 * 2.0)));
    assert!(close(n.appearance.stroke_width(), 72.0 / 25.4));
    for (w, unit) in [("100pt", Unit::Points), ("10pc", Unit::Picas), ("10cm", Unit::Centimeters)] {
        let d = import(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="10"/>"#)).unwrap();
        assert_eq!(d.units, unit, "{w}");
    }
}

#[test]
fn unitless_and_pixels_map_one_to_one() {
    for w in ["100", "100px"] {
        let d = import(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="50"><rect width="100" height="50"/></svg>"#)).unwrap();
        assert_eq!(d.artboards[0].rect, Rect::new(0.0, 0.0, 100.0, 50.0), "{w}");
        assert_eq!(d.units, Unit::Pixels, "{w}");
        assert!(close_rect(art(&d)[0].geometric_bounds().unwrap(), Rect::new(0.0, 0.0, 100.0, 50.0)));
    }
    // No size at all: the viewBox gives it, in user units.
    let d = import(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 40"/>"#).unwrap();
    assert_eq!(d.artboards[0].rect, Rect::new(0.0, 0.0, 30.0, 40.0));
    assert_eq!(d.units, Unit::Pixels);
}

#[test]
fn font_size_units_are_honoured() {
    let d = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
        <text x="0" y="20" font-size="12pt">a</text>
        <text x="0" y="40" font-size="16px">b</text>
        <text x="0" y="60" font-size="0.5in">c</text>
        <g font-size="10"><text x="0" y="80" font-size="2em">d</text><text x="0" y="100" font-size="150%">e</text></g>
        <text x="0" y="120">f<tspan font-size="larger">g</tspan></text>
        <text x="0" y="140" font-size="x-large">h</text>
        </svg>"#,
    )
    .unwrap();
    let sizes = text_sizes(&d);
    let want = [12.0, 16.0, 36.0, 20.0, 15.0, 12.0, 14.4, 18.0];
    assert_eq!(sizes.len(), want.len(), "{sizes:?}");
    for (s, w) in sizes.iter().zip(want) {
        assert!(close(*s, w), "{sizes:?}");
    }
}

#[test]
fn text_lengths_in_physical_units() {
    let d = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10in" height="10in">
        <text x="1in" y="2in" font-size="20" letter-spacing="0.1em" stroke="red" stroke-width="1pt">Hi</text></svg>"#,
    )
    .unwrap();
    let NodeKind::Text(t) = &art(&d)[0].kind else { panic!() };
    let o = t.xf.translation();
    assert!(close(o.x, 72.0) && close(o.y, 144.0), "{o:?}");
    let st = t.first_style();
    assert!(close(st.tracking, 100.0), "{}", st.tracking);
    // User units of a physical root are CSS pixels (0.75 pt): that scale moves into the type.
    assert!(close(t.xf.as_coeffs()[0], 1.0), "{:?}", t.xf);
    assert!(close(st.stroke_width, 1.0), "{}", st.stroke_width);
    assert!(close(st.size, 15.0), "{}", st.size);
}

#[test]
fn absolute_lengths_in_a_pixel_document_keep_their_size() {
    let d = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
        <rect width="1in" height="10mm" stroke="black" stroke-width="2pt"/></svg>"#,
    )
    .unwrap();
    let n = art(&d)[0];
    assert!(close_rect(n.geometric_bounds().unwrap(), Rect::new(0.0, 0.0, 72.0, 720.0 / 25.4)), "{:?}", n.geometric_bounds());
    assert!(close(n.appearance.stroke_width(), 2.0));
}

/// Lengths inside the content of a root in absolute units with a `viewBox` convert as browsers
/// convert them (SVG 1.1 §7.10): an absolute length is CSS pixels in user units, which the
/// `viewBox` then scales like any other. Here a user unit is 1 mm (2.835 pt), so `10mm` is 37.8
/// user units, 107.2 pt (37.8 mm) on the page, and `12pt` type is 16 user units, 45.4 pt: what a
/// browser shows, not the physical 28.35 and 12 pt.
#[test]
fn inner_absolute_lengths_under_a_physical_root_with_a_view_box_scale_as_in_browsers() {
    let d = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="210mm" height="297mm" viewBox="0 0 210 297">
        <rect x="0" y="0" width="10mm" height="10mm"/><text x="0" y="100" font-size="12pt">Hi</text></svg>"#,
    )
    .unwrap();
    let per_user = 210.0 * 72.0 / 25.4 / 210.0;
    let px = 96.0 / 25.4 * 10.0;
    assert!(close_rect(d.artboards[0].rect, Rect::new(0.0, 0.0, 595.28, 841.89)), "{:?}", d.artboards[0].rect);
    let b = art(&d)[0].geometric_bounds().unwrap();
    assert!(close(b.width(), px * per_user) && close(b.height(), px * per_user), "{b:?}");
    let sizes = text_sizes(&d);
    assert!(sizes.len() == 1 && close(sizes[0], 16.0 * per_user), "{sizes:?}");
}

/// A `viewBox` of no size is ignored, as usvg ignores it: the root's size still comes from its
/// attributes, exactly, and user units are CSS pixels of it.
#[test]
fn a_view_box_of_no_size_is_ignored() {
    let d = import(r#"<svg xmlns="http://www.w3.org/2000/svg" width="200pt" height="100pt" viewBox="0 0 0 0"><rect width="10" height="10"/></svg>"#)
        .unwrap();
    assert_eq!(d.artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 100.0));
    assert_eq!(d.units, Unit::Points);
    let b = art(&d)[0].geometric_bounds().unwrap();
    assert!(close_rect(b, Rect::new(0.0, 0.0, 7.5, 7.5)), "{b:?}");
}

/// The root size is written with enough decimals to come back exactly whatever the content's
/// `decimals` option (0 here): 200 pt is 7.0556 cm, which 7.1 cm would not have given back.
#[test]
fn the_root_size_survives_a_low_decimals_export() {
    use vectorcraft_svg::{ExportOptions, export};
    let mut d = Document::new(200.0, 100.0);
    d.units = Unit::Centimeters;
    let svg = export(&d, &ExportOptions { decimals: 1, ..ExportOptions::default() });
    assert!(svg.contains(r#"width="7.05556cm" height="3.52778cm""#), "{svg}");
    let back = import(&svg).unwrap();
    let r = back.artboards[0].rect;
    assert!((r.width() - 200.0).abs() < 1e-3 && (r.height() - 100.0).abs() < 1e-3, "{r:?}");
}

/// #864: a document's units survive an SVG round trip, and so does its artboard size. The root
/// `width`/`height` carry the unit (`width="210mm"`, SVG 1.1 §7.10 / §5.1.2), with the `viewBox`
/// in user units (points, as the model is); a document in pixels writes unitless lengths, which
/// are pixels (SVG 1.1 §7.10: "a user unit is equivalent to a CSS px"), and comes back in pixels.
#[test]
fn document_units_survive_a_round_trip() {
    use vectorcraft_svg::{ExportOptions, export};
    for (unit, w, h) in [
        (Unit::Millimeters, 210.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4),
        (Unit::Centimeters, 10.0 * 72.0 / 2.54, 5.0 * 72.0 / 2.54),
        (Unit::Inches, 8.5 * 72.0, 11.0 * 72.0),
        (Unit::Points, 200.0, 100.0),
        (Unit::Picas, 12.0 * 30.0, 12.0 * 20.0),
        (Unit::Pixels, 400.0, 300.0),
    ] {
        let mut d = Document::new(w, h);
        d.units = unit;
        let l = d.layers[0].id;
        let r = Rect::new(10.0, 20.0, 110.0, 70.0);
        let node = Node::path(d.alloc_id(), vectorcraft_geom::shapes::rectangle(r), vectorcraft_doc::Appearance::default_art());
        d.insert(Some(l), usize::MAX, node).unwrap();
        let svg = export(&d, &ExportOptions::default());
        let back = import(&svg).unwrap_or_else(|e| panic!("{unit:?}: {e}\n{svg}"));
        assert_eq!(back.units, unit, "{svg}");
        assert!(close_rect(back.artboards[0].rect, Rect::new(0.0, 0.0, w, h)), "{unit:?}: {:?}\n{svg}", back.artboards[0].rect);
        let b = art(&back)[0].geometric_bounds().unwrap();
        assert!(close_rect(b, r), "{unit:?}: {b:?}\n{svg}");
    }
    // Pixels write unitless lengths, as the web expects.
    let mut d = Document::new(400.0, 300.0);
    d.units = Unit::Pixels;
    let svg = export(&d, &ExportOptions::default());
    assert!(svg.contains(r#"width="400" height="300""#), "{svg}");
    // The units SVG has no name for write the nearest it has, and come back as that.
    for (unit, w, back_unit) in [(Unit::Feet, 72.0 * 12.0, Unit::Inches), (Unit::Meters, 72.0 / 0.0254, Unit::Centimeters)] {
        let mut d = Document::new(w, w);
        d.units = unit;
        let svg = export(&d, &ExportOptions::default());
        let back = import(&svg).unwrap();
        assert_eq!(back.units, back_unit, "{svg}");
        assert!(close_rect(back.artboards[0].rect, Rect::new(0.0, 0.0, w, w)), "{unit:?}: {:?}\n{svg}", back.artboards[0].rect);
    }
}
