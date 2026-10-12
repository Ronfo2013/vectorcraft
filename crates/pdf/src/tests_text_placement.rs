//! #722: type from a PDF lands where the file draws it. Each file here is written from scratch
//! from the PDF specification (ISO 32000-1 §9, "Text": the text state, text-positioning and
//! text-showing operators, Type 0, Type 3 and TrueType fonts, vertical writing), with Source Sans 3
//! from this repo's assets embedded as its font program. What the file draws (its glyphs opened as
//! outlines) and what the type it opens as draws must match contour by contour.

use std::sync::Arc;

use kurbo::{BezPath, PathEl, Point, Rect, Shape};
use vectorcraft_doc::{Document, NodeKind, TextKind, TextObject};
use vectorcraft_testkit::pdf::{PdfPage, first_extra, pdf_with};
use vectorcraft_text::{FontDb, FontFace};

use crate::tests_import_fidelity::stream;
use crate::*;

const SOURCE_SANS: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-Regular.ttf");

fn sans() -> Arc<FontFace> {
    FontDb::global().find_postscript("SourceSans3-Regular").expect("Source Sans 3 is built in")
}

/// Source Sans 3's advance of `c`, in thousandths of an em (as a PDF's widths give it).
fn width(c: char) -> f64 {
    let face = sans();
    (face.advance(face.glyph_for(c)) * 1000.0 / face.units_per_em()).round()
}

/// The font program of Source Sans 3 Regular as a stream (ASCII hex).
fn font_file() -> String {
    let hex: String = SOURCE_SANS.iter().map(|b| format!("{b:02X}")).collect::<String>() + ">";
    stream(&format!("/Length1 {} /Filter /ASCIIHexDecode", SOURCE_SANS.len()), &hex)
}

/// A font descriptor for the font program in object `file`.
fn descriptor(file: usize) -> String {
    format!(
        "<< /Type /FontDescriptor /FontName /SourceSans3-Regular /Flags 32 /FontBBox [-500 -300 1500 1000] /ItalicAngle 0 /Ascent 1024 /Descent -400 /CapHeight 660 /StemV 80 /FontFile2 {file} 0 R >>"
    )
}

/// A simple TrueType font (WinAnsi, Source Sans 3's widths) described by object `desc`.
fn simple(desc: usize) -> String {
    let widths: Vec<String> = (32u8..=126).map(|c| width(c as char).to_string()).collect();
    format!(
        "<< /Type /Font /Subtype /TrueType /BaseFont /SourceSans3-Regular /FirstChar 32 /LastChar 126 /Widths [{}] /FontDescriptor {desc} 0 R /Encoding /WinAnsiEncoding >>",
        widths.join(" ")
    )
}

/// The characters the Type 0 fonts map: printable ASCII, and symbols that stand upright in
/// vertical type and are narrower than an em.
fn cid_chars() -> impl Iterator<Item = char> {
    (32u8..=126).map(char::from).chain("§±×".chars())
}

/// `text` as a string of Source Sans 3's glyph ids, two bytes each (Identity-H or Identity-V,
/// CIDToGIDMap Identity).
fn gids(text: &str) -> String {
    let face = sans();
    let hex: String = text.chars().map(|c| format!("{:04X}", face.glyph_for(c))).collect();
    format!("<{hex}>")
}

/// A ToUnicode map of Source Sans 3's glyph ids for [`cid_chars`].
fn to_unicode() -> String {
    let face = sans();
    let rows: Vec<String> = cid_chars().map(|c| format!("<{:04X}> <{:04X}>", face.glyph_for(c), c as u32)).collect();
    let data = format!(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Test-UCS def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange {} beginbfchar\n{}\nendbfchar endcmap CMapName currentdict /CMap defineresource pop end end",
        rows.len(),
        rows.join("\n")
    );
    stream("", &data)
}

/// A CID-keyed TrueType font (the descendant of the Type 0 fonts) of Source Sans 3 with its
/// widths (/W), described by object `desc`.
fn cid_font(desc: usize) -> String {
    let face = sans();
    let w: Vec<String> = cid_chars().map(|c| format!("{} [{}]", face.glyph_for(c), width(c))).collect();
    format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /SourceSans3-Regular /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc} 0 R /CIDToGIDMap /Identity /DW 1000 /W [{}] >>",
        w.join(" ")
    )
}

/// A Type 3 font (object `first`, its glyph procedures the two after it) whose glyph space is
/// 1/`upm` of text space (FontMatrix [1/upm 0 0 1/upm 0 0]): `a` is half an em wide and draws a box
/// 0.4 em wide, `b` an em wide with a box of 0.9 em; both boxes are 0.7 em tall on the baseline.
fn type3(first: usize, upm: f64) -> Vec<String> {
    let s = 1.0 / upm;
    let (h, w) = (upm * 0.5, upm);
    let a = stream("", &format!("{h} 0 d0 0 0 {} {} re f", h * 0.8, upm * 0.7));
    let b = stream("", &format!("{w} 0 d0 0 0 {} {} re f", w * 0.9, upm * 0.7));
    let font = format!(
        "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 {w} {w}] /FontMatrix [{s} 0 0 {s} 0 0] /CharProcs << /a {} 0 R /b {} 0 R >> /Encoding << /Type /Encoding /Differences [97 /a /b] >> /FirstChar 97 /LastChar 98 /Widths [{h} {w}] /Resources << >> >>",
        first + 1,
        first + 2
    );
    vec![font, a, b]
}

/// A 300 × 200 pt page drawing `content`, plus `entries` in the page dictionary, with fonts:
/// - `/F1`: Source Sans 3, embedded, a simple TrueType font;
/// - `/F4`: Source Sans 3 as a Type 0 font, Identity-H (two-byte glyph ids, see [`gids`]);
/// - `/F5`: the same, Identity-V (vertical writing);
/// - `/T3`, `/T30`: Type 3 fonts of 1000 and of 100 units an em;
/// - `/HV`: Helvetica, not embedded.
fn page(content: &str, entries: &str) -> Vec<u8> {
    let n = first_extra(1);
    let type0 = |enc: &str| {
        format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /SourceSans3-Regular /Encoding /{enc} /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
            n + 3,
            n + 4
        )
    };
    let mut objs = vec![
        font_file(),         // n
        descriptor(n),       // n + 1
        simple(n + 1),       // n + 2: F1
        cid_font(n + 1),     // n + 3
        to_unicode(),        // n + 4
        type0("Identity-H"), // n + 5: F4
        type0("Identity-V"), // n + 6: F5
    ];
    let t3 = n + objs.len();
    objs.extend(type3(t3, 1000.0));
    objs.extend(type3(t3 + 3, 100.0));
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    let resources = format!(
        "/Font << /F1 {} 0 R /F4 {} 0 R /F5 {} 0 R /T3 {t3} 0 R /T30 {} 0 R /HV << /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >> >>",
        n + 2,
        n + 5,
        n + 6,
        t3 + 3
    );
    pdf_with(&[PdfPage { resources, entries: entries.into(), ..PdfPage::new(300.0, 200.0, content) }], &refs, None)
}

fn open_as(bytes: &[u8], text_as: TextAs) -> Document {
    import_with_report(bytes, &ImportOptions { text_as, ..Default::default() }).unwrap().document
}

fn texts(d: &Document) -> Vec<TextObject> {
    let mut v = vec![];
    d.walk(|n| {
        if let NodeKind::Text(t) = &n.kind {
            v.push((**t).clone());
        }
    });
    v
}

fn plain(d: &Document) -> Vec<String> {
    texts(d).iter().map(TextObject::plain_text).collect()
}

/// The largest difference between two boxes' edges.
fn off(a: Rect, b: Rect) -> f64 {
    [a.x0 - b.x0, a.y0 - b.y0, a.x1 - b.x1, a.y1 - b.y1].iter().fold(0.0f64, |m, v| m.max(v.abs()))
}

/// The bounds of each contour of `bp` (each subpath).
fn contours_of(bp: &BezPath, out: &mut Vec<Rect>) {
    let mut cur = BezPath::new();
    for el in bp.elements() {
        if matches!(el, PathEl::MoveTo(_)) && cur.elements().len() > 1 {
            out.push(std::mem::take(&mut cur).bounding_box());
        }
        if matches!(el, PathEl::MoveTo(_)) {
            cur = BezPath::new();
        }
        cur.push(*el);
    }
    if cur.elements().len() > 1 {
        out.push(cur.bounding_box());
    }
}

/// The contours `d` draws: its paths' and its type's glyphs', in document space.
fn contours(d: &Document) -> Vec<Rect> {
    let mut out = vec![];
    d.walk(|n| match &n.kind {
        NodeKind::Path { path, .. } => contours_of(&path.to_bezpath(), &mut out),
        NodeKind::Text(t) => {
            for g in vectorcraft_text::layout(FontDb::global(), t).glyphs {
                contours_of(&(t.xf * g.outline), &mut out);
            }
        }
        _ => {}
    });
    out
}

/// How far (points) the glyphs of the type `bytes` opens as are from where the file draws them:
/// the furthest any contour the file draws is from the nearest one the type draws, or the other
/// way round.
fn displacement(bytes: &[u8]) -> f64 {
    let drawn = contours(&open_as(bytes, TextAs::Outlines));
    let typed = contours(&open_as(bytes, TextAs::Text));
    assert!(!drawn.is_empty() && !typed.is_empty(), "the file draws glyphs");
    let worst = |a: &[Rect], b: &[Rect]| a.iter().map(|r| b.iter().map(|s| off(*r, *s)).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max);
    worst(&drawn, &typed).max(worst(&typed, &drawn))
}

/// The type `content` opens as lands where the file draws it, as `want` (its objects' text).
#[track_caller]
fn lands(content: &str, entries: &str, want: &[&str]) {
    let bytes = page(content, entries);
    assert_eq!(plain(&open_as(&bytes, TextAs::Text)), want, "{content}");
    let d = displacement(&bytes);
    assert!(d < 0.02, "{content}: glyphs {d:.3} pt from where the file draws them");
}

/// Three lines wrapped in a frame (each wrapped line ends in a space).
const PARAGRAPH: &str = "(The quick brown fox ) Tj T* (jumps over the lazy ) Tj T* (dog and runs away.) Tj";
const PARAGRAPH_TEXT: &str = "The quick brown fox jumps over the lazy dog and runs away.";

/// The text-positioning operators and the text matrix under any CTM: lines start where the file
/// starts them, the glyphs of each where it draws them.
#[test]
fn text_positioning_and_the_ctm_put_type_where_the_file_draws_it() {
    let lines = ["Line one", "Line two", "Line three"];
    for (content, entries, want) in [
        ("BT /F1 24 Tf 20 50 Td (Hello World) Tj ET", "", &["Hello World"][..]),
        ("BT /F1 1 Tf 24 0 0 24 20 50 Tm (Hello World) Tj ET", "", &["Hello World"]),
        ("q 2 0 0 2 10 10 cm BT /F1 12 Tf 10 20 Td (Hello World) Tj ET Q", "", &["Hello World"]),
        // A y-down CTM with a text matrix that turns the type upright again.
        ("q 1 0 0 -1 0 200 cm BT /F1 12 Tf 1 0 0 -1 20 50 Tm (Hello World) Tj ET Q", "", &["Hello World"]),
        ("BT /F1 12 Tf 14 TL 20 150 Td (Line one) Tj T* (Line two) Tj T* (Line three) Tj ET", "", &lines),
        ("BT /F1 12 Tf 20 150 Td (Line one) Tj 0 -16 TD (Line two) Tj T* (Line three) Tj ET", "", &lines),
        // ' moves to the next line; " also sets word and character spacing.
        ("BT /F1 12 Tf 14 TL 20 150 Td (Line one) Tj (Line two) ' 2 1 (Line three) \" ET", "", &lines),
        ("BT /F1 18 Tf 0.866 0.5 -0.5 0.866 40 40 Tm (Turned line) Tj ET", "", &["Turned line"]),
        ("BT /F1 1 Tf 30 0 0 15 20 50 Tm (Squashed) Tj ET", "", &["Squashed"]),
        ("q 1 0 0 0.5 0 0 cm BT /F1 20 Tf 20 100 Td (Squashed) Tj ET Q", "", &["Squashed"]),
        ("BT /F1 24 Tf 60 80 Td (Cropped) Tj ET", "/CropBox [40 30 260 180]", &["Cropped"]),
        ("BT /F1 24 Tf 20 50 Td (Turned page) Tj ET", "/Rotate 90", &["Turned page"]),
    ] {
        lands(content, entries, want);
    }
}

/// Horizontal scaling, character and word spacing, and `TJ`'s adjustments: each glyph keeps the
/// place the file gave it (they used to be evened out over the line, and laid out with the font's
/// kerning where the file had none, so the glyphs between the ends slid).
#[test]
fn spacing_and_scaling_keep_each_glyph_where_the_file_put_it() {
    for (content, want) in [
        ("BT /F1 20 Tf 50 Tz 20 50 Td (Narrow type) Tj ET", "Narrow type"),
        ("BT /F1 20 Tf 150 Tz 20 50 Td (Wide type) Tj ET", "Wide type"),
        ("BT /F1 16 Tf 2 Tc 20 50 Td (Spaced out) Tj ET", "Spaced out"),
        ("BT /F1 16 Tf -0.8 Tc 20 50 Td (Tight type here) Tj ET", "Tight type here"),
        ("BT /F1 16 Tf 6 Tw 20 50 Td (Word space here) Tj ET", "Word space here"),
        ("BT /F1 16 Tf 1.5 Tc 80 Tz 20 50 Td (Spaced and narrow) Tj ET", "Spaced and narrow"),
        ("BT /F1 16 Tf 20 50 Td [(A) 120 (V) -80 (A) 300 (Tower)] TJ ET", "AVATower"),
        ("BT /F1 16 Tf 20 50 Td (AVATAR WAVE) Tj ET", "AVATAR WAVE"),
    ] {
        lands(content, "", &[want]);
    }
    // Two-byte codes of a Type 0 font alike (word spacing applies only to single-byte code 32,
    // ISO 32000-1 §9.3.3: none here).
    for content in ["BT /F4 24 Tf 70 Tz 3 Ts 20 50 Td <> Tj ET", "BT /F4 24 Tf 9 Tw 2 Tc 20 50 Td <> Tj ET"] {
        lands(&content.replace("<>", &gids("Hello World")), "", &["Hello World"]);
    }
}

/// Word spacing stays on the spaces, as tracking of their own; evenly spaced letters keep one
/// tracking value for the line, and a file that didn't kern opens with kerning off.
#[test]
fn word_spacing_is_tracking_on_the_spaces_and_unkerned_type_opens_unkerned() {
    let d = open_as(&page("BT /F1 16 Tf 6 Tw 20 50 Td (Word space here) Tj ET", ""), TextAs::Text);
    let t = texts(&d);
    let letters: Vec<f64> = t[0].runs.iter().filter(|r| r.text != " ").map(|r| r.style.tracking).collect();
    let spaces: Vec<f64> = t[0].runs.iter().filter(|r| r.text == " ").map(|r| r.style.tracking).collect();
    assert_eq!(spaces.len(), 2, "{:?}", t[0].runs);
    for s in &spaces {
        assert!((s - 375.0).abs() < 0.5, "6 pt at 16 pt is 375/1000 em: {spaces:?}");
    }
    assert!(letters.iter().all(|l| l.abs() < 0.5), "the letters as the font spaces them: {letters:?}");
    // "AV", "VA", "WA" kern in Source Sans 3; this file set them on their widths alone.
    let d = open_as(&page("BT /F1 16 Tf 20 50 Td (AVATAR WAVE) Tj ET", ""), TextAs::Text);
    let t = texts(&d);
    assert_eq!(t[0].runs.len(), 1, "{:?}", t[0].runs);
    assert_eq!(t[0].runs[0].style.kerning, Some(0.0), "kerning off, as the file set it");
    assert!(t[0].runs[0].style.tracking.abs() < 0.5);
}

/// Type the font's kerning spaced (as editors write it) keeps the font's kerning.
#[test]
fn kerned_type_keeps_the_fonts_kerning() {
    let bytes = crate::tests_live_text::text_pdf(&[(20.0, 50.0, 24.0, "AVATAR WAVE")], false);
    let d = open_as(&bytes, TextAs::Text);
    let t = texts(&d);
    assert_eq!(t.len(), 1);
    assert!(t[0].runs.iter().all(|r| r.style.kerning.is_none()), "{:?}", t[0].runs);
    assert!(displacement(&bytes) < 0.02);
}

/// Text rise (`Ts`): a superscript or subscript stays in its line as baseline shift (it used to be
/// pulled onto the baseline within 0.15 em of it, or split off as an object of its own), and a line
/// that starts raised keeps the baseline of the rest of its text.
#[test]
fn raised_and_lowered_characters_stay_in_their_line_shifted_off_its_baseline() {
    for (content, want) in [
        ("BT /F1 12 Tf 20 50 Td 5 Ts (Raised line) Tj ET", "Raised line"),
        ("BT /F1 12 Tf 20 50 Td (E = mc) Tj 5 Ts (2) Tj 0 Ts ( here) Tj ET", "E = mc2 here"),
        ("BT /F1 12 Tf 20 50 Td (H) Tj -3 Ts (2) Tj 0 Ts (O water) Tj ET", "H2O water"),
        ("BT /F1 12 Tf 20 50 Td (H) Tj 1.5 Ts (2) Tj 0 Ts (O water) Tj ET", "H2O water"),
        ("BT /F1 12 Tf 20 50 Td (CO) Tj -1.2 Ts /F1 8 Tf (2) Tj 0 Ts /F1 12 Tf ( gas) Tj ET", "CO2 gas"),
        ("BT /F1 12 Tf 20 50 Td 1.5 Ts (1) Tj 0 Ts (Footnote text) Tj ET", "1Footnote text"),
    ] {
        lands(content, "", &[want]);
    }
    let t = texts(&open_as(&page("BT /F1 12 Tf 20 50 Td 1.5 Ts (1) Tj 0 Ts (Footnote text) Tj ET", ""), TextAs::Text));
    let origin = t[0].xf * Point::ZERO;
    assert!((origin.y - 150.0).abs() < 1e-6, "on the text's baseline, not the mark's: {origin:?}");
    assert_eq!(t[0].runs[0].text, "1");
    assert!((t[0].runs[0].style.baseline_shift - 1.5).abs() < 1e-6, "{:?}", t[0].runs[0].style.baseline_shift);
    assert!(t[0].runs[1..].iter().all(|r| r.style.baseline_shift == 0.0));
}

/// Lines wrapped in a frame (area type, as an editor writes it: each line placed with `T*` or `Td`)
/// come back as one area type object whose glyphs are where the lines' were: aligned left, or
/// justified with word spacing (`Tw`) on each wrapped line.
#[test]
fn text_boxes_open_where_their_lines_were() {
    for content in [
        format!("BT /F1 12 Tf 14 TL 20 150 Td {PARAGRAPH} ET"),
        format!("BT /F1 12 Tf 14 TL 4 Ts 80 Tz 20 150 Td {PARAGRAPH} ET"),
        format!("q 1.5 0 0 1.5 5 5 cm BT /F1 8 Tf 10 TL 10 90 Td {PARAGRAPH} ET Q"),
        format!("BT /F1 12 Tf 14 TL 0.866 0.5 -0.5 0.866 40 40 Tm {PARAGRAPH} ET"),
    ] {
        let t = texts(&open_as(&page(&content, ""), TextAs::Text));
        assert_eq!(t.len(), 1, "{content}: one text box");
        assert!(matches!(t[0].kind, TextKind::Area { .. }), "{content}");
        lands(&content, "", &[PARAGRAPH_TEXT]);
    }
    // Justified: every wrapped line spaced out to the frame's width with word spacing.
    let lines = ["The quick brown fox ", "jumps over the lazy ", "dog and runs away."];
    let (size, frame) = (12.0, 110.0);
    let mut content = format!("BT /F1 {size} Tf 20 170 Td ");
    for (i, l) in lines.iter().enumerate() {
        let natural = l.trim_end().chars().map(width).sum::<f64>() * size / 1000.0;
        let spaces = l.trim_end().matches(' ').count() as f64;
        let tw = if i + 1 < lines.len() { (frame - natural) / spaces } else { 0.0 };
        if i > 0 {
            content += "0 -14.4 Td ";
        }
        content += &format!("{tw:.4} Tw ({l}) Tj ");
    }
    content += "ET";
    let t = texts(&open_as(&page(&content, ""), TextAs::Text));
    assert_eq!(t.len(), 1, "one text box: {:?}", t.iter().map(TextObject::plain_text).collect::<Vec<_>>());
    assert_eq!(t[0].para.justify, vectorcraft_doc::Justify::JustifyLeft);
    lands(&content, "", &[PARAGRAPH_TEXT]);
}

/// Vertical writing (Identity-V): the file puts each glyph's origin half its width (/W) left of
/// the column (ISO 32000-1 §9.7.4.3); glyphs narrower than an em stay centred on it.
#[test]
fn vertical_glyphs_narrower_than_an_em_stay_on_their_column() {
    lands(&format!("BT /F5 24 Tf 150 180 Td {} Tj ET", gids("§±×")), "", &["§±×"]);
}

/// Type 3 glyphs (drawn by their procedures, kept as outlines) are scaled by the font's matrix:
/// the same boxes at 1000 and at 100 units an em.
#[test]
fn type3_glyphs_follow_the_font_matrix() {
    for font in ["T3", "T30"] {
        let d = open_as(&page(&format!("BT /{font} 20 Tf 20 50 Td (ab) Tj ET"), ""), TextAs::Text);
        assert!(texts(&d).is_empty(), "Type 3 text keeps its outlines");
        let mut boxes = contours(&d);
        boxes.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        // `a` at x 20 (0.4 em = 8 pt wide), `b` after its half-em advance; 0.7 em = 14 pt tall,
        // standing on the baseline at y 150 (200 - 50).
        let want = [Rect::new(20.0, 136.0, 28.0, 150.0), Rect::new(30.0, 136.0, 48.0, 150.0)];
        assert_eq!(boxes.len(), 2, "{font}: {boxes:?}");
        for (b, w) in boxes.iter().zip(want) {
            assert!(off(*b, w) < 1e-3, "{font}: {b:?} vs {w:?}");
        }
    }
}

/// A font that isn't embedded or installed is set in a stand-in: the line still starts where
/// the file starts it, on its baseline, and is as long as the file's.
#[test]
fn type_in_a_stand_in_font_still_starts_where_the_file_starts_it() {
    let t = texts(&open_as(&page("BT /HV 12 Tf 6 Tw 20 50 Td 5 Ts (Raised word line) Tj ET", ""), TextAs::Text));
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].plain_text(), "Raised word line");
    let origin = t[0].xf * Point::ZERO;
    assert!((origin.x - 20.0).abs() < 1e-6 && (origin.y - 145.0).abs() < 1e-6, "{origin:?}");
}
