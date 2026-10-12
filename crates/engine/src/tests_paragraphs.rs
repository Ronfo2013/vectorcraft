//! Paragraph attributes per paragraph: ranged Paragraph panel commands, paragraph styles on one
//! paragraph, splitting and merging paragraphs while editing, undo and threaded text.

use serde_json::json;
use vectorcraft_doc::{Justify, NodeId, NodeKind, TextObject};
use vectorcraft_tools::{Mods, PointerEvent, PointerKind, ToolKey};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    s
}

fn area(s: &mut Session, text: &str) -> u64 {
    s.execute("text.create", &json!({"x": 10, "y": 10, "text": text, "area": {"width": 300, "height": 300}})).unwrap()["id"].as_u64().unwrap()
}

fn text(s: &Session, id: u64) -> TextObject {
    match &s.doc().unwrap().doc.node(NodeId(id)).unwrap().kind {
        NodeKind::Text(t) => (**t).clone(),
        _ => panic!("not text"),
    }
}

fn justs(s: &Session, id: u64) -> Vec<Justify> {
    text(s, id).paragraph_styles().iter().map(|p| p.justify).collect()
}

#[test]
fn ranged_paragraph_commands_touch_only_their_paragraphs() {
    let mut s = session();
    let id = area(&mut s, "one\ntwo\nthree");
    // A caret in "two" (byte 5): only the second paragraph.
    s.execute("text.setStyle", &json!({"id": id, "justify": "center", "start": 5, "end": 5})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Center, Justify::Auto]);
    // A selection from "one" into "two": both.
    s.execute("text.setFormat", &json!({"ids": [id], "spaceBefore": 9, "leftIndent": 4, "start": 1, "end": 5})).unwrap();
    let t = text(&s, id);
    let sb: Vec<f64> = t.paragraph_styles().iter().map(|p| p.space_before).collect();
    assert_eq!(sb, [9.0, 9.0, 0.0]);
    assert_eq!(t.para.left_indent, 4.0, "paragraph 0 is `para`");
    // Character attributes of setFormat follow the range too.
    s.execute("text.setFormat", &json!({"ids": [id], "underline": true, "start": 4, "end": 7})).unwrap();
    let t = text(&s, id);
    assert!(t.runs.iter().any(|r| r.text == "two" && r.style.underline));
    assert!(t.runs.iter().any(|r| r.text.starts_with("one") && !r.style.underline));
    // The layout sees each paragraph's alignment.
    let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t);
    assert!(lay.lines[1].x0 > lay.lines[0].x0 + 20.0, "the centred line sits right of the left one");
    // No range: every paragraph (as before).
    s.execute("text.setStyle", &json!({"id": id, "justify": "right"})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Right; 3]);
    // Undo steps back through the ranged edits.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Center, Justify::Auto]);
    // Tabs per paragraph; a query reads the paragraph at `start`.
    s.execute("text.tabs.set", &json!({"ids": [id], "stops": [{"position": 40}], "start": 9, "end": 9})).unwrap();
    assert_eq!(s.execute("text.tabs.get", &json!({"ids": [id], "start": 9})).unwrap()["stops"].as_array().unwrap().len(), 1);
    assert_eq!(s.execute("text.tabs.get", &json!({"ids": [id]})).unwrap()["stops"], json!([]));
    // Junk ranges are errors, not panics.
    assert!(s.execute("text.setFormat", &json!({"ids": [id], "spaceAfter": 1, "start": -3})).is_err());
    assert!(s.execute("text.setStyle", &json!({"id": id, "justify": "left", "end": "x"})).is_err());
    s.execute("text.setFormat", &json!({"ids": [id], "spaceAfter": 1, "start": 1_000_000, "end": u64::MAX})).unwrap();
    assert_eq!(text(&s, id).para_at(2).space_after, 1.0, "past the end: the last paragraph");
}

#[test]
fn editing_splits_and_merges_paragraph_styles() {
    let mut s = session();
    let id = area(&mut s, "one\ntwo\nthree");
    s.execute("text.setStyle", &json!({"id": id, "justify": "center", "start": 5, "end": 5})).unwrap();
    // A break inside "two": both halves keep its style.
    s.execute("text.editRange", &json!({"id": id, "start": 5, "end": 5, "insert": "\n"})).unwrap();
    assert_eq!(text(&s, id).plain_text(), "one\nt\nwo\nthree");
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Center, Justify::Center, Justify::Auto]);
    // Deleting the break after "one": the first paragraph's style wins.
    s.execute("text.editRange", &json!({"id": id, "start": 3, "end": 4})).unwrap();
    assert_eq!(text(&s, id).plain_text(), "onet\nwo\nthree");
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Center, Justify::Auto]);
    // Pasting styled runs with breaks in "wo": the new paragraphs continue it.
    let runs = json!([{"text": "A\nB\n", "style": text(&s, id).runs[0].style}]);
    s.execute("text.editRange", &json!({"id": id, "start": 6, "end": 6, "runs": runs})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Center, Justify::Center, Justify::Center, Justify::Auto]);
    // Undo restores the styles with the text.
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Center, Justify::Center, Justify::Auto]);
    // Replacing all the text: every paragraph takes the first one's attributes.
    s.execute("text.setText", &json!({"ids": [id], "text": "x\ny"})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Auto, Justify::Auto]);
    assert!(text(&s, id).paras.is_empty());
}

#[test]
fn return_in_the_type_tool_continues_the_paragraph_it_splits() {
    let mut s = session();
    let v = ViewInfo::default();
    let id = s.execute("text.create", &json!({"x": 100, "y": 100, "text": "one\ntwo", "size": 20})).unwrap()["id"].as_u64().unwrap();
    s.execute("text.setStyle", &json!({"id": id, "justify": "right", "start": 0, "end": 0})).unwrap();
    s.execute("text.setStyle", &json!({"id": id, "justify": "center", "start": 5, "end": 5})).unwrap();
    s.select_tool("type", v).unwrap();
    // Click into the first line (right-aligned: it ends at the origin), then End and Return.
    let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &text(&s, id));
    let l0 = &lay.lines[0];
    let (x, y) = (100.0 + (l0.x0 + l0.x1) / 2.0, 100.0 + l0.baseline - 5.0);
    for k in [PointerKind::Down, PointerKind::Up] {
        s.pointer(&PointerEvent::new(k, x, y), v).unwrap();
    }
    s.tool_key(ToolKey::End, Mods::default(), v).unwrap();
    s.tool_key(ToolKey::Enter, Mods::default(), v).unwrap();
    s.tool_text("new", v).unwrap();
    s.tool_key(ToolKey::Escape, Mods::default(), v).unwrap();
    assert_eq!(text(&s, id).plain_text(), "one\nnew\ntwo");
    assert_eq!(justs(&s, id), [Justify::Right, Justify::Right, Justify::Center]);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Right, Justify::Center]);
}

#[test]
fn paragraph_style_applies_to_one_paragraph() {
    let mut s = session();
    let id = area(&mut s, "Title\nBody text");
    // The second paragraph keeps new type's attributes, whatever they are.
    let body = text(&s, id).para_at(1).clone();
    s.execute("paraStyle.new", &json!({"name": "Head", "attrs": {"justify": "Center", "space_after": 8.0}})).unwrap();
    s.execute("paraStyle.apply", &json!({"name": "Head", "id": id, "start": 2, "end": 2})).unwrap();
    let t = text(&s, id);
    assert_eq!(t.para_at(0).style_name.as_deref(), Some("Head"));
    assert_eq!((t.para_at(0).justify, t.para_at(0).space_after), (Justify::Center, 8.0));
    assert_eq!(t.para_at(1), &body, "new type's");
    let list = s.execute("paraStyle.list", &json!({})).unwrap();
    assert_eq!(list["styles"][1]["uses"], 1, "one paragraph uses it");
    assert_eq!(list["styles"][0]["uses"], 1, "the other is Normal");
    // Redefining updates that paragraph only; renaming follows it.
    s.execute("paraStyle.setAttrs", &json!({"name": "Head", "attrs": {"justify": "Right"}})).unwrap();
    assert_eq!(justs(&s, id), [Justify::Right, Justify::Auto]);
    s.execute("paraStyle.rename", &json!({"name": "Head", "to": "Heading"})).unwrap();
    assert_eq!(text(&s, id).para_at(0).style_name.as_deref(), Some("Heading"));
    // New from the selection reads the caret's paragraph.
    s.execute("paraStyle.new", &json!({"name": "From body", "id": id, "start": 8})).unwrap();
    let defs = &s.doc().unwrap().doc.para_styles;
    assert_eq!(defs.iter().find(|d| d.name == "From body").unwrap().attrs["justify"], "Auto");
}

#[test]
fn threaded_frames_keep_each_paragraphs_style() {
    let mut s = session();
    let story = "The quick brown fox jumps over the lazy dog and keeps running. ".repeat(3) + "\nSecond paragraph.\nThird.";
    let a =
        s.execute("text.create", &json!({"x": 10, "y": 10, "text": story, "area": {"width": 150, "height": 60}})).unwrap()["id"].as_u64().unwrap();
    let b = s.execute("shape.rectangle", &json!({"x": 200, "y": 10, "width": 150, "height": 200})).unwrap()["id"].as_u64().unwrap();
    s.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    s.execute("text.thread.create", &json!({})).unwrap();
    let joined = |s: &Session| format!("{}{}", text(s, a).plain_text(), text(s, b).plain_text());
    assert_eq!(joined(&s), story);
    // Centre the second paragraph (in frame b) and right-align the third.
    let tb = text(&s, b).plain_text();
    let second = tb.find("Second").unwrap();
    s.execute("text.setStyle", &json!({"id": b, "justify": "center", "start": second, "end": second})).unwrap();
    let third = text(&s, b).plain_text().find("Third").unwrap();
    s.execute("text.setStyle", &json!({"id": b, "justify": "right", "start": third, "end": third})).unwrap();
    assert_eq!(justs(&s, b), [Justify::Auto, Justify::Center, Justify::Right]);
    // Typing in the first frame re-flows the story: the styles move with their paragraphs.
    s.execute("text.editRange", &json!({"id": a, "start": 0, "end": 0, "insert": "More words at the start of the story. "})).unwrap();
    let (ja, jb) = (justs(&s, a), justs(&s, b));
    assert_eq!(ja, [Justify::Auto]);
    assert_eq!(jb, [Justify::Auto, Justify::Center, Justify::Right]);
    // A break in the first frame makes a new paragraph there; the later ones keep their styles.
    s.execute("text.editRange", &json!({"id": a, "start": 4, "end": 4, "insert": "\n"})).unwrap();
    let all: Vec<Justify> = [justs(&s, a), justs(&s, b)].concat();
    assert_eq!(all.iter().filter(|j| **j == Justify::Center).count(), 1);
    assert_eq!(all.last(), Some(&Justify::Right));
    // Restyling the paragraph that continues into frame b (its first) sticks after the re-flow.
    s.execute("text.setStyle", &json!({"id": b, "justify": "justifyAll", "start": 0, "end": 0})).unwrap();
    assert_eq!(justs(&s, b)[0], Justify::JustifyAll);
    assert_eq!(*justs(&s, a).last().unwrap(), Justify::JustifyAll, "the paragraph's part in frame a follows");
}

/// Upstream's paragraph attributes (direction, mojikumi, leading model) follow a range like the
/// others: only the touched paragraphs change, and the layout reads each paragraph's own.
#[test]
fn ranged_set_format_sets_direction_mojikumi_and_leading_model_per_paragraph() {
    use vectorcraft_doc::{LeadingModel, Mojikumi, ParaDirection};
    let mut s = session();
    let id = area(&mut s, "Hello.\nHello.");
    // A caret in the second paragraph (byte 9).
    s.execute(
        "text.setFormat",
        &json!({"ids": [id], "direction": "rightToLeft", "leadingModel": "emBoxTop", "mojikumi": "none", "start": 9, "end": 9}),
    )
    .unwrap();
    let t = text(&s, id);
    let (p0, p1) = (t.para_at(0).clone(), t.para_at(1).clone());
    assert_eq!((p0.direction, p0.leading_model, p0.mojikumi), (None, LeadingModel::RomanBaseline, Mojikumi::LineEndHalf));
    assert_eq!((p1.direction, p1.leading_model, p1.mojikumi), (Some(ParaDirection::RightToLeft), LeadingModel::EmBoxTop, Mojikumi::None));
    let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &t);
    assert!(!lay.lines[0].rtl && lay.lines[1].rtl, "each paragraph's direction");
    // Return in the right-to-left paragraph continues it.
    s.execute("text.editRange", &json!({"id": id, "start": 13, "end": 13, "insert": "\nX"})).unwrap();
    let t = text(&s, id);
    assert_eq!(t.para_at(2).direction, Some(ParaDirection::RightToLeft));
    // No range: every paragraph.
    s.execute("text.setFormat", &json!({"ids": [id], "direction": "auto"})).unwrap();
    assert!(text(&s, id).paragraph_styles().iter().all(|p| p.direction.is_none()));
}

/// Paragraph panel menu › Hyphenation…: `text.hyphenation` queries and sets the options per
/// paragraph, validates them, and the layout follows (one undo step).
#[test]
fn hyphenation_options_through_the_command() {
    let mut s = session();
    let id = area(&mut s, "one\ntwo");
    let q = s.execute("text.hyphenation", &json!({"ids": [id]})).unwrap();
    assert_eq!(q, json!({"hyphenate": false, "minWord": 6, "afterFirst": 3, "beforeLast": 3, "hyphenLimit": 0, "zone": 0.0, "capitalized": true}));
    s.execute(
        "text.hyphenation",
        &json!({"ids": [id], "hyphenate": true, "minWord": 8, "hyphenLimit": 2, "zone": 36, "capitalized": false, "start": 5, "end": 5}),
    )
    .unwrap();
    let t = text(&s, id);
    let ps = t.paragraph_styles();
    assert_eq!(ps[0].hyphenation, vectorcraft_doc::Hyphenation::default(), "only the paragraph at the caret");
    assert!(ps[1].hyphenate);
    assert_eq!((ps[1].hyphenation.min_word, ps[1].hyphenation.hyphen_limit, ps[1].hyphenation.zone), (8, 2, 36.0));
    assert!(!ps[1].hyphenation.capitalized);
    let q = s.execute("text.hyphenation", &json!({"ids": [id], "start": 5})).unwrap();
    assert_eq!(q["minWord"], json!(8));
    assert_eq!(q["capitalized"], json!(false));
    // Bad values are refused, nothing changes.
    for bad in [
        json!({"minWord": 0}),
        json!({"minWord": 26}),
        json!({"afterFirst": 2.5}),
        json!({"zone": -1}),
        json!({"zone": "far"}),
        json!({"capitalized": 1}),
        json!({"hyphenLimit": 99}),
    ] {
        let mut p = bad.clone();
        p["ids"] = json!([id]);
        assert!(s.execute("text.hyphenation", &p).is_err(), "{bad}");
    }
    assert_eq!(text(&s, id).paragraph_styles()[1].hyphenation.min_word, 8);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(text(&s, id).paragraph_styles()[1].hyphenation, vectorcraft_doc::Hyphenation::default());
    // The layout uses them: no word of this copy is longer than 20 letters.
    let copy = "Extraordinarily comprehensive typographical considerations notwithstanding everything.";
    let id =
        s.execute("text.create", &json!({"x": 10, "y": 10, "text": copy, "area": {"width": 120, "height": 500}})).unwrap()["id"].as_u64().unwrap();
    s.execute("text.hyphenation", &json!({"ids": [id], "hyphenate": true})).unwrap();
    let hyphens =
        |s: &Session| vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &text(s, id)).glyphs.iter().filter(|g| g.len == 0).count();
    assert!(hyphens(&s) > 0);
    s.execute("text.hyphenation", &json!({"ids": [id], "minWord": 20})).unwrap();
    assert_eq!(hyphens(&s), 0);
}

/// Paragraph panel menu › Justification…: `text.justification` queries and sets the options,
/// keeps Minimum ≤ Desired ≤ Maximum, and the layout follows.
#[test]
fn justification_options_through_the_command() {
    let mut s = session();
    let id = area(&mut s, "spacing in type");
    let q = s.execute("text.justification", &json!({"ids": [id]})).unwrap();
    assert_eq!(q["wordSpacingMin"], json!(80.0));
    assert_eq!(q["wordSpacingDesired"], json!(100.0));
    assert_eq!(q["wordSpacingMax"], json!(150.0));
    assert_eq!(q["letterSpacingMax"], json!(0.0));
    assert_eq!(q["glyphScalingDesired"], json!(100.0));
    assert_eq!(q["autoLeading"], json!(120.0));
    assert_eq!(q["singleWordJustification"], json!("fullJustify"));
    let width = |s: &Session| {
        let l = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), &text(s, id));
        l.lines.iter().map(|li| li.x1 - li.x0).fold(0.0, f64::max)
    };
    let before = width(&s);
    s.execute(
        "text.justification",
        &json!({"ids": [id], "letterSpacingDesired": 20, "glyphScalingMin": 90, "glyphScalingMax": 110, "autoLeading": 150, "singleWordJustification": "alignCenter"}),
    )
    .unwrap();
    let j = text(&s, id).para.justification.clone();
    assert_eq!(j.letter_spacing, vectorcraft_doc::SpacingRange::new(0.0, 20.0, 20.0), "a Desired above Maximum moves it");
    assert_eq!(j.glyph_scaling, vectorcraft_doc::SpacingRange::new(90.0, 100.0, 110.0));
    assert_eq!(j.auto_leading, 150.0);
    assert_eq!(j.single_word, vectorcraft_doc::SingleWordJustify::AlignCenter);
    assert!(width(&s) > before + 1.0, "letter spacing widens every line");
    for bad in [
        json!({"wordSpacingMax": 1001}),
        json!({"letterSpacingMin": -101}),
        json!({"glyphScalingDesired": 49}),
        json!({"autoLeading": "auto"}),
        json!({"singleWordJustification": "spread"}),
        json!({"wordSpacingDesired": null, "glyphScalingMax": 201}),
    ] {
        let mut p = bad.clone();
        p["ids"] = json!([id]);
        assert!(s.execute("text.justification", &p).is_err(), "{bad}");
    }
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(
        text(&s, id).para.justification,
        vectorcraft_doc::Justification { single_word: vectorcraft_doc::SingleWordJustify::FullJustify, ..Default::default() }
    );
    // The options survive a save and reopen of the native format.
    s.execute("text.justification", &json!({"ids": [id], "wordSpacingDesired": 120})).unwrap();
    s.execute("text.hyphenation", &json!({"ids": [id], "afterFirst": 4})).unwrap();
    let bytes = vectorcraft_format::save(&s.doc().unwrap().doc, false);
    let back = vectorcraft_format::load(&bytes).unwrap();
    let NodeKind::Text(t) = &back.node(NodeId(id)).unwrap().kind else { panic!("text") };
    assert_eq!(t.para.justification.word_spacing.desired, 120.0);
    assert_eq!(t.para.hyphenation.after_first, 4);
    assert_eq!(t.para.justification.single_word, vectorcraft_doc::SingleWordJustify::FullJustify, "new type keeps Full Justify");
}

#[test]
fn justification_rejects_contradictions_and_unknown_keys() {
    let mut s = session();
    let id = area(&mut s, "spacing in type");
    let before = text(&s, id).para.clone();
    for bad in [
        json!({"wordSpacingMin": 120, "wordSpacingDesired": 100}),
        json!({"wordSpacingDesired": 200, "wordSpacingMax": 150}),
        json!({"letterSpacingMin": 10, "letterSpacingMax": 5}),
        json!({"glyphScalingMin": 110, "glyphScalingDesired": 105, "glyphScalingMax": 120}),
        json!({"wordSpacingDesire": 100}),
        json!({"autoLeading": 150, "single": "alignLeft"}),
    ] {
        let mut p = bad.clone();
        p["ids"] = json!([id]);
        assert!(s.execute("text.justification", &p).is_err(), "{bad}");
    }
    for bad in [json!({"minWords": 5}), json!({"zone": 10, "hyphenateCaps": true})] {
        let mut p = bad.clone();
        p["ids"] = json!([id]);
        assert!(s.execute("text.hyphenation", &p).is_err(), "{bad}");
    }
    // A query with a misspelt option is an error too, not a silent query.
    assert!(s.execute("text.justification", &json!({"ids": [id], "autoleading": 150})).is_err());
    assert_eq!(text(&s, id).para, before, "nothing changed");
    // Values in order, and equal ones, are fine.
    s.execute("text.justification", &json!({"ids": [id], "wordSpacingMin": 90, "wordSpacingDesired": 90, "wordSpacingMax": 300})).unwrap();
    assert_eq!(text(&s, id).para.justification.word_spacing, vectorcraft_doc::SpacingRange::new(90.0, 90.0, 300.0));
}

#[test]
fn old_paragraphs_show_how_their_single_words_are_set() {
    let mut s = session();
    let id = area(&mut s, "spacing in type");
    // A paragraph from a file saved before the option.
    s.edit("test", |d, _| {
        if let Some(NodeKind::Text(t)) = d.node_mut(NodeId(id)).map(|n| &mut n.kind) {
            t.para.justification = vectorcraft_doc::Justification::default();
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(text(&s, id).para.justification.single_word, vectorcraft_doc::SingleWordJustify::Legacy);
    for (justify, shown) in [("justifyLeft", "alignLeft"), ("justifyCenter", "alignLeft"), ("justifyAll", "fullJustify")] {
        s.execute("text.setStyle", &json!({"id": id, "justify": justify})).unwrap();
        let q = s.execute("text.justification", &json!({"ids": [id]})).unwrap();
        assert_eq!(q["singleWordJustification"], json!(shown), "{justify}");
    }
    // Other options leave it as it was.
    s.execute("text.justification", &json!({"ids": [id], "autoLeading": 130})).unwrap();
    assert_eq!(text(&s, id).para.justification.single_word, vectorcraft_doc::SingleWordJustify::Legacy);
    assert!(s.execute("text.justification", &json!({"ids": [id], "singleWordJustification": "legacy"})).is_err(), "not a choice");
}

#[test]
fn paragraph_options_on_several_objects_undo_in_one_step() {
    let mut s = session();
    let a = area(&mut s, "first frame");
    let b = s.execute("text.create", &json!({"x": 400, "y": 10, "text": "second frame", "area": {"width": 200, "height": 200}})).unwrap()["id"]
        .as_u64()
        .unwrap();
    let (pa, pb) = (text(&s, a).para.clone(), text(&s, b).para.clone());
    let r = s
        .execute(
            "text.justification",
            &json!({"ids": [a, b], "wordSpacingDesired": 120, "autoLeading": 150, "singleWordJustification": "alignRight"}),
        )
        .unwrap();
    assert_eq!(r["ids"], json!([a, b]));
    for id in [a, b] {
        let j = text(&s, id).para.justification;
        assert_eq!((j.word_spacing.desired, j.auto_leading, j.single_word), (120.0, 150.0, vectorcraft_doc::SingleWordJustify::AlignRight));
    }
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!((text(&s, a).para, text(&s, b).para), (pa.clone(), pb.clone()), "one undo step restores both");
    s.execute("text.hyphenation", &json!({"ids": [a, b], "minWord": 8, "hyphenLimit": 2, "hyphenate": true})).unwrap();
    for id in [a, b] {
        let p = text(&s, id).para;
        assert!(p.hyphenate && p.hyphenation.min_word == 8 && p.hyphenation.hyphen_limit == 2);
    }
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!((text(&s, a).para, text(&s, b).para), (pa, pb), "one undo step restores both");
}

#[test]
fn leading_nudge_starts_from_the_paragraphs_auto_leading() {
    let mut s = session();
    let id = s.execute("text.create", &json!({"x": 10, "y": 10, "text": "one two\nthree four", "size": 10})).unwrap()["id"].as_u64().unwrap();
    s.execute("text.justification", &json!({"ids": [id], "autoLeading": 200})).unwrap();
    let inc = s.prefs.type_size_increment;
    // Alt+↑/↓ on the whole object: Auto leading is 20 pt (200 % of 10 pt), and the nudge goes on
    // from there.
    s.execute("type.step", &json!({"attribute": "leading", "id": id})).unwrap();
    let t = text(&s, id);
    assert!(t.runs.iter().all(|r| r.style.leading.is_some_and(|l| (l - (20.0 + inc)).abs() < 1e-9)), "{:?}", t.runs);
    // At the default 120 % it starts from 12 pt, as before.
    let id = s.execute("text.create", &json!({"x": 10, "y": 200, "text": "plain", "size": 10})).unwrap()["id"].as_u64().unwrap();
    s.execute("type.step", &json!({"attribute": "leading", "id": id})).unwrap();
    assert!(text(&s, id).runs.iter().all(|r| r.style.leading.is_some_and(|l| (l - (12.0 + inc)).abs() < 1e-9)));
}
