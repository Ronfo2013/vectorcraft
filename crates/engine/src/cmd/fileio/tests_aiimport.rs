//! Illustrator EPS and `.ai` files open from the editing data they carry: layers, groups, names,
//! hidden objects, artboards, colours and images as they were; what the reader doesn't read yet
//! opens as before, with a warning saying why.

use std::io::Write as _;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_testkit::ai;

use super::*;

fn open(s: &mut Session, name: &str, bytes: &[u8], params: Value) -> Result<Value> {
    let mut p = json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(bytes)});
    if let (Some(p), Some(extra)) = (p.as_object_mut(), params.as_object()) {
        p.extend(extra.clone());
    }
    s.execute("document.open", &p)
}

/// The `.ai` private data of `data`, compressed as older `.ai` files compress it.
fn compressed(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    [&b"%AI12_CompressedData"[..], &e.finish().unwrap()].concat()
}

fn names(nodes: &[Arc<Node>]) -> Vec<String> {
    nodes.iter().map(|n| n.name.clone().unwrap_or_default()).collect()
}

/// The sample's structure, as [`ai::sample_data`] builds it.
fn check_sample(s: &Session) {
    let doc = &s.doc().unwrap().doc;
    assert_eq!(names(&doc.layers), ["Art", "Hidden"]);
    assert_eq!(doc.artboards.len(), 1);
    assert_eq!(doc.artboards[0].rect, vectorcraft_geom::Rect::new(0.0, 0.0, 200.0, 100.0));
    let art = doc.layers[0].children().unwrap();
    assert_eq!(names(art), ["Pair", "Ring", "Window"]);
    let pair = art[0].children().unwrap();
    assert_eq!(names(pair), ["Red", ""]);
    assert!(pair[0].visible && !pair[1].visible, "the hidden member stays hidden");
    assert!(matches!(art[1].kind, NodeKind::Compound { .. }));
    assert!(matches!(&art[2].kind, NodeKind::Group { clip: true, children } if matches!(children[0].kind, NodeKind::Path { clipping: true, .. })));
    let hidden = &doc.layers[1];
    assert!(!hidden.visible);
    let kids = hidden.children().unwrap();
    assert_eq!(names(kids), ["Shaded", "", "Sub", ""]);
    assert!(matches!(kids[0].appearance.fill().map(|f| &f.paint), Some(vectorcraft_color::Paint::Gradient(_))));
    assert_eq!((kids[1].opacity, kids[1].blend), (0.5, vectorcraft_color::BlendMode::Screen));
    assert!(kids[2].locked && matches!(kids[2].kind, NodeKind::Layer { .. }));
    let NodeKind::Image(im) = &kids[3].kind else { panic!("an image") };
    assert_eq!((im.width, im.height), (2, 1));
}

#[test]
fn an_illustrator_eps_opens_with_its_layers() {
    let mut s = Session::new();
    let r = open(&mut s, "art.eps", &ai::eps(&ai::sample_data(), ai::page_ps()), json!({})).unwrap();
    assert_eq!((&r["format"], &r["warnings"]), (&json!("eps"), &json!([])), "{r}");
    check_sample(&s);
    // Saved as EPS and opened again, it comes back the same.
    let eps = s.execute("document.exportEps", &json!({})).unwrap();
    let bytes = vectorcraft_format::base64_decode(eps["dataBase64"].as_str().unwrap()).unwrap();
    let r = open(&mut s, "again.eps", &bytes, json!({})).unwrap();
    assert_eq!(r["restored"], json!(true), "{r}");
    check_sample(&s);
}

#[test]
fn an_illustrator_ai_file_opens_from_its_editing_data() {
    let mut s = Session::new();
    let file = ai::ai(&compressed(&ai::sample_data()), ai::page_pdf());
    let r = open(&mut s, "art.ai", &file, json!({})).unwrap();
    assert_eq!(r["format"], json!("ai"), "{r}");
    assert!(r["warnings"].as_array().unwrap().iter().all(|w| !w.as_str().unwrap().contains("weren't read")), "{r}");
    check_sample(&s);
    // Pages picked: the PDF's.
    open(&mut s, "art.ai", &file, json!({"pages": "1"})).unwrap();
    assert_ne!(names(&s.doc().unwrap().doc.layers), ["Art", "Hidden"]);
}

#[test]
fn what_the_reader_doesnt_read_opens_as_before_with_a_warning() {
    let symbols = ai::editing_data(200.0, 100.0, &ai::layer("Art", "/SymbolInstance :\n(Dot) /SymbolRef ,\n;\n"));
    let mut s = Session::new();
    let r = open(&mut s, "symbols.eps", &ai::eps(symbols.as_bytes(), ai::page_ps()), json!({})).unwrap();
    assert!(r["warnings"].to_string().contains("layers weren't read") && r["warnings"].to_string().contains("symbols"), "{r}");
    assert_eq!(names(&s.doc().unwrap().doc.layers), ["Layer 1"]);
    let r = open(&mut s, "symbols.ai", &ai::ai(&compressed(symbols.as_bytes()), ai::page_pdf()), json!({})).unwrap();
    assert!(r["warnings"].to_string().contains("symbols") && r["warnings"].to_string().contains("PDF part"), "{r}");
}

/// A page showing only a placeholder line, as a `.ai` saved without PDF compatibility has.
const PLACEHOLDER: &str = "BT /F1 12 Tf 10 50 Td (Saved without its PDF part) Tj ET";

#[test]
fn an_ai_file_saved_without_its_pdf_part_opens_from_its_editing_data() {
    let mut s = Session::new();
    let r = open(&mut s, "art.ai", &ai::ai(&compressed(&ai::sample_data()), PLACEHOLDER), json!({})).unwrap();
    assert_eq!(r["format"], json!("ai"), "{r}");
    check_sample(&s);
    // Its editing data damaged: it says so (the placeholder isn't the art).
    let e = open(&mut s, "damaged.ai", &ai::ai(b"%AI12_CompressedData not zlib", PLACEHOLDER), json!({})).unwrap_err().to_string();
    assert!(e.contains("without PDF compatibility") && e.contains("zlib"), "{e}");
    // Without editing data, the placeholder still isn't opened as the art.
    let e = open(&mut s, "plain.ai", &ai::ai(b"", PLACEHOLDER), json!({})).unwrap_err().to_string();
    assert!(e.contains("placeholder"), "{e}");
}

#[test]
fn editing_data_false_opens_only_what_the_page_prints() {
    let mut s = Session::new();
    for (name, file) in [("art.eps", ai::eps(&ai::sample_data(), ai::page_ps())), ("art.ai", ai::ai(&compressed(&ai::sample_data()), ai::page_pdf()))]
    {
        let r = open(&mut s, name, &file, json!({"editingData": false})).unwrap();
        let layers = &s.doc().unwrap().doc.layers;
        assert_eq!(layers.len(), 1, "{name}: {r}");
        assert_ne!(names(layers), ["Art"], "{name}: the page's layer");
    }
    let e = open(&mut s, "art.eps", &ai::eps(&ai::sample_data(), ai::page_ps()), json!({"editingData": "no"})).unwrap_err().to_string();
    assert!(e.contains("editingData must be true or false"), "{e}");
    // A file that is only its editing data can't open without it.
    let e = open(&mut s, "art.ai", &ai::ai(&compressed(&ai::sample_data()), PLACEHOLDER), json!({"editingData": false})).unwrap_err().to_string();
    assert!(e.contains("placeholder"), "{e}");
}

#[test]
fn fit_to_artwork_bounds_leaves_out_the_hidden_art_and_guides_the_editing_data_has() {
    // A shown square; a hidden one and a long guide far from it, which the page doesn't draw.
    let art =
        format!("0 g\n{}f\n1 Xw\n{}f\n0 Xw\n-500 20 m\n700 20 L\n(N) *\n", ai::rect(10.0, 10.0, 50.0, 50.0), ai::rect(150.0, 60.0, 190.0, 95.0));
    let data = ai::editing_data(200.0, 100.0, &ai::layer("Art", &art));
    let mut s = Session::new();
    let r = open(&mut s, "fit.eps", &ai::eps(data.as_bytes(), "0 setgray 10 10 40 40 rectfill\n"), json!({})).unwrap();
    let layer = &s.doc().unwrap().doc.layers[0];
    let kids = layer.children().unwrap();
    assert_eq!(kids.len(), 3, "{r}");
    assert!(!kids[1].visible && matches!(kids[2].kind, NodeKind::Path { guide: true, .. }));
    s.execute("artboard.fitToArt", &json!({})).unwrap();
    // The square, 50 to 90 down the 100-point page.
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, vectorcraft_geom::Rect::new(10.0, 50.0, 50.0, 90.0));
}

#[test]
fn type_asked_for_as_outlines_opens_a_file_whose_type_shows_from_its_pdf_part() {
    let text = "/AI11Text :\n0 /FreeUndo ,\n0 /FrameIndex ,\n0 /StoryIndex ,\n;\n";
    let data = ai::editing_data(200.0, 100.0, &ai::layer("Art", &format!("0 g\n{}f\n{text}", ai::rect(10.0, 10.0, 50.0, 50.0))));
    let file = ai::ai(&compressed(data.as_bytes()), "0 g 10 10 40 40 re f BT /F1 12 Tf 100 50 Td (Hi) Tj ET");
    let mut s = Session::new();
    // As type, the page's type goes into the text object of the layers.
    let r = open(&mut s, "type.ai", &file, json!({})).unwrap();
    assert_eq!(names(&s.doc().unwrap().doc.layers), ["Art"], "{r}");
    // As outlines, only the PDF part has them.
    let r = open(&mut s, "type.ai", &file, json!({"textAs": "outlines"})).unwrap();
    assert!(r["warnings"].to_string().contains("outlines"), "{r}");
    assert_ne!(names(&s.doc().unwrap().doc.layers), ["Art"], "{r}");
}

/// A layer named `name` with its states (`visible`, `locked`, `printable`) holding `art`.
fn layer_with(name: &str, visible: bool, locked: bool, printable: bool, art: &str) -> String {
    format!(
        "%AI5_BeginLayer\n{} 1 {} {} 0 0 1 0 79 128 255 0 50 0 Lb\n({name}) Ln\n{art}\nLB\n%AI5_EndLayer--\n",
        u8::from(visible),
        u8::from(!locked),
        u8::from(printable)
    )
}

/// Editing data (200 × 100 pt) with nested layers, sublayers, groups, a compound path, a clipping
/// group, a hidden object, a guide on a non-printing layer and a hidden layer, and the PDF content
/// its page draws of it (the art that shows, as the PDF part of a `.ai` has it).
fn nested_data() -> (Vec<u8>, &'static str) {
    let black = "0 g\n";
    let inner = format!("0 Ae\nu\n{black}{}f\nU\n{}", ai::rect(50.0, 60.0, 80.0, 90.0), ai::named("Inner"));
    let ring = format!(
        "0 Ae\n*u\n{black}1 XR\n{}f\n{}f\n*U\n{}0 XR\n",
        ai::rect(100.0, 60.0, 140.0, 90.0),
        ai::rect(110.0, 70.0, 130.0, 80.0),
        ai::named("Ring")
    );
    let figures = format!("0 Ae\nu\n{black}{}f\n{inner}{ring}U\n{}", ai::rect(10.0, 60.0, 40.0, 90.0), ai::named("Figures"));
    let window = format!(
        "0 Ae\nq\n{}h\nW\nn\n{black}{}f\nQ\n{}",
        ai::rect(150.0, 60.0, 190.0, 90.0),
        ai::rect(160.0, 50.0, 200.0, 100.0),
        ai::named("Window")
    );
    let captions = layer_with("Captions", true, true, true, &window);
    let hidden_path = format!("1 Xw\n{black}{}f\n0 Xw\n", ai::rect(10.0, 10.0, 40.0, 40.0));
    let units = ai::layer("Units", &format!("{figures}{captions}{hidden_path}"));
    let dieline = layer_with("Dieline", true, false, false, &format!("{}(N) *\n", ai::rect(5.0, 5.0, 195.0, 95.0)));
    let notes = layer_with("Notes", false, false, true, &format!("{black}{}f\n", ai::rect(50.0, 10.0, 80.0, 40.0)));
    let data = ai::editing_data(200.0, 100.0, &format!("{units}{dieline}{notes}"));
    (data.into_bytes(), "0 g 10 60 30 30 re f 50 60 30 30 re f 100 60 40 30 re 110 70 20 10 re f* q 150 60 40 30 re W n 160 50 40 50 re f Q")
}

/// How many nodes under `nodes` (them included) satisfy `p`.
fn count(nodes: &[Arc<Node>], p: &dyn Fn(&Node) -> bool) -> usize {
    nodes.iter().map(|n| usize::from(p(n)) + n.children().map_or(0, |c| count(c, p))).sum()
}

/// The structure [`nested_data`] writes, as opened.
fn check_nested(s: &Session, what: &str) {
    let doc = &s.doc().unwrap().doc;
    assert_eq!(names(&doc.layers), ["Units", "Dieline", "Notes"], "{what}");
    let units = doc.layers[0].children().unwrap();
    assert_eq!(names(units), ["Figures", "Captions", ""], "{what}");
    let figures = units[0].children().unwrap();
    assert_eq!(names(figures), ["", "Inner", "Ring"], "{what}");
    assert_eq!(figures[1].children().unwrap().len(), 1, "{what}");
    assert!(matches!(&figures[2].kind, NodeKind::Compound { children, .. } if children.len() == 2), "{what}: {:?}", figures[2].kind);
    assert!(matches!(units[1].kind, NodeKind::Layer { .. }) && units[1].locked && units[1].visible, "{what}: a locked sublayer");
    let captions = units[1].children().unwrap();
    assert_eq!(names(captions), ["Window"], "{what}");
    let NodeKind::Group { children, clip: true } = &captions[0].kind else { panic!("{what}: a clipping group: {:?}", captions[0].kind) };
    assert!(children.len() == 2 && matches!(children[0].kind, NodeKind::Path { clipping: true, .. }), "{what}");
    assert!(!units[2].visible, "{what}: the hidden object");
    let NodeKind::Layer { printable: false, children, .. } = &doc.layers[1].kind else { panic!("{what}: Dieline prints") };
    assert!(children.len() == 1 && matches!(children[0].kind, NodeKind::Path { guide: true, .. }), "{what}: the guide");
    assert!(!doc.layers[2].visible && doc.layers[2].children().unwrap().len() == 1, "{what}: the hidden layer keeps its art");
    assert_eq!(count(&doc.layers, &|n| matches!(n.kind, NodeKind::Layer { .. })), 4, "{what}: three layers and a sublayer");
    assert_eq!(count(&doc.layers, &|n| matches!(n.kind, NodeKind::Path { guide: true, .. })), 1, "{what}");
}

/// Nested layers, sublayers, groups, compound paths, clipping groups, hidden objects, guides and
/// hidden and non-printing layers open as written, from a `.ai` with its PDF part, one saved
/// without it, and an EPS (G9: #951 saw 109 layers for 14, #868 a flat tree, #779 lost guides).
#[test]
fn nested_layers_groups_and_guides_open_as_written() {
    let (data, page) = nested_data();
    let mut s = Session::new();
    let r = open(&mut s, "nested.ai", &ai::ai(&compressed(&data), page), json!({})).unwrap();
    assert!(!r["warnings"].to_string().contains("weren't read"), "{r}");
    check_nested(&s, "the .ai");
    open(&mut s, "alone.ai", &ai::ai(&compressed(&data), PLACEHOLDER), json!({})).unwrap();
    check_nested(&s, "the .ai without its PDF part");
    let ps = "0 setgray 10 60 30 30 rectfill 50 60 30 30 rectfill newpath 100 60 moveto 140 60 lineto 140 90 lineto 100 90 lineto closepath \
              110 70 moveto 130 70 lineto 130 80 lineto 110 80 lineto closepath eofill \
              gsave newpath 150 60 moveto 190 60 lineto 190 90 lineto 150 90 lineto closepath clip newpath 160 50 40 50 rectfill grestore\n";
    let r = open(&mut s, "nested.eps", &ai::eps(&data, ps), json!({})).unwrap();
    assert!(!r["warnings"].to_string().contains("weren't read"), "{r}");
    check_nested(&s, "the EPS");
}

/// A `.ai` whose private data can't be read opens as its PDF part with a note saying why, rather
/// than silently (#951 and #868 reported files read as their PDF part with no reason given).
#[test]
fn a_file_whose_private_data_cant_be_read_says_why() {
    let (data, page) = nested_data();
    let mut s = Session::new();
    // The first stream missing from the file.
    let missing = |file: Vec<u8>| String::from_utf8_lossy(&file).replace("/AIPrivateData1 5 0 R", "/AIPrivateData1 99 0 R").into_bytes();
    let r = open(&mut s, "broken.ai", &missing(ai::ai(&compressed(&data), page)), json!({})).unwrap();
    let w = r["warnings"].to_string();
    assert!(w.contains("weren't read from its editing data") && w.contains("private data stream 1"), "{r}");
    assert_eq!(names(&s.doc().unwrap().doc.layers), ["Page 1"]);
    // Editing data without layers: a note, and the PDF part's own note stays (the art outside the
    // artboards is still only in the private data, which supplied nothing).
    let r = open(&mut s, "bare.ai", &ai::ai(&compressed(b"%!PS-Adobe-3.0\n%%EOF\n"), page), json!({})).unwrap();
    let w = r["warnings"].as_array().unwrap();
    assert!(w.contains(&json!(vectorcraft_eps::NO_LAYERS_NOTE)) && w.contains(&json!(vectorcraft_pdf::OFF_ARTBOARD_NOTE)), "{r}");
    // Data that isn't editing data: the layers weren't read (a loss), with the PDF part's note.
    let r = open(&mut s, "odd.ai", &ai::ai(&compressed(b"nothing of the kind"), page), json!({})).unwrap();
    let w = r["warnings"].to_string();
    assert!(w.contains("weren't read from its editing data") && w.contains("isn't editing data") && w.contains("outside its artboards"), "{r}");
    // When the layers came from the editing data, the PDF part's note goes.
    let r = open(&mut s, "nested.ai", &ai::ai(&compressed(&data), page), json!({})).unwrap();
    assert!(!r["warnings"].to_string().contains("outside its artboards"), "{r}");
    // Saved without its PDF part: the reason is the error.
    let e = open(&mut s, "broken.ai", &missing(ai::ai(&compressed(&data), PLACEHOLDER)), json!({})).unwrap_err().to_string();
    assert!(e.contains("private data stream 1"), "{e}");
}
