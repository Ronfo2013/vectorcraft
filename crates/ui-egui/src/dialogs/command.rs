//! The generic parameter dialog (`ui.paramDialog`): edits a command's parameters (`__command`,
//! headed `__label`) and runs it on OK, with the fixed ones in `__params` (not shown) added.

use serde_json::Value;

use super::{DialogSpec, form};
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |d| tl!(&d.str("__label")).to_string(),
    body: |app, ui, d| {
        let command = d.str("__command");
        let lengths = lengths(&command);
        form::param_fields(ui, d, &|k| lengths.contains(&k), &|k| choices(&command, k), &|k| rank(&command, k), app.session.general_unit());
        false
    },
    confirm,
    ..DialogSpec::FORM
};

/// Where parameter `key` of `command` comes in its dialog (fields of equal rank by name): the
/// Hyphenation and Justification dialogs keep the reference app's field order.
fn rank(command: &str, key: &str) -> usize {
    const HYPHENATION: [&str; 7] = ["hyphenate", "minWord", "afterFirst", "beforeLast", "hyphenLimit", "zone", "capitalized"];
    const JUSTIFICATION: [&str; 11] = [
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
    let order: &[&str] = match command {
        "text.hyphenation" => &HYPHENATION,
        "text.justification" => &JUSTIFICATION,
        _ => &[],
    };
    order.iter().position(|k| *k == key).unwrap_or(0)
}

/// The parameters of the commands this dialog edits that are distances.
fn lengths(command: &str) -> &'static [&'static str] {
    match command {
        "graph.create" => &["width", "height"],
        "shape.flare" => &["diameter", "pathLength"],
        "perspective.grid.set" => &["cell", "distance"],
        "object.repeat.options" => &["radius", "hSpacing", "vSpacing"],
        "text.areaOptions" => &["width", "height", "gutter", "inset", "firstBaselineMin"],
        "type.pathOptions" => &["spacing"],
        "text.hyphenation" => &["zone"],
        _ => &[],
    }
}

/// The parameters of the commands this dialog edits that pick one of some values.
fn choices(command: &str, key: &str) -> Option<form::Choices> {
    match (command, key) {
        ("type.pathOptions", "effect") => Some(crate::menus::PATH_EFFECTS),
        ("type.pathOptions", "alignToPath") => Some(PATH_ALIGN),
        ("text.areaOptions", "fit") => Some(AREA_FIT),
        ("text.areaOptions", "firstBaseline") => Some(FIRST_BASELINE),
        ("text.areaOptions", "verticalAlign") => Some(VERTICAL_ALIGN),
        ("graph.setType", "valueAxis") => Some(GRAPH_VALUE_AXIS),
        ("graph.setType", "tickLength" | "rightTickLength" | "categoryTickLength") => Some(GRAPH_TICK_LENGTH),
        ("text.justification", "singleWordJustification") => Some(SINGLE_WORD),
        _ => None,
    }
}

/// Type on a Path Options › Align to Path.
const PATH_ALIGN: form::Choices = &[("Ascender", "ascender"), ("Descender", "descender"), ("Center", "center"), ("Baseline", "baseline")];

/// Area Type Options › Fit.
const AREA_FIT: form::Choices = &[("None", "none"), ("Auto Size", "autoHeight"), ("Shrink Text to Fit", "shrinkText")];

/// Area Type Options › First Baseline.
const FIRST_BASELINE: form::Choices =
    &[("Ascent", "ascent"), ("Cap Height", "capHeight"), ("x Height", "xHeight"), ("Leading", "leading"), ("Fixed", "fixed")];

/// Area Type Options › Align (vertical alignment of the lines in each row/column).
const VERTICAL_ALIGN: form::Choices = &[("Top", "top"), ("Center", "center"), ("Bottom", "bottom"), ("Justify", "justify")];

/// Graph Type › Value Axis (series picked on both axes show none, and OK leaves each where it is).
const GRAPH_VALUE_AXIS: form::Choices = &[("On Left Side", "left"), ("On Right Side", "right"), ("On Both Sides", "both")];

/// Graph Type › Tick Marks › Length.
const GRAPH_TICK_LENGTH: form::Choices = &[("None", "none"), ("Short", "short"), ("Full Width", "full")];

/// Justification › Single Word Justification.
const SINGLE_WORD: form::Choices =
    &[("Full Justify", "fullJustify"), ("Align Left", "alignLeft"), ("Align Center", "alignCenter"), ("Align Right", "alignRight")];

/// Closes before running, so a dialog the command opens stays open.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let cmd = d.str("__command");
    let mut params = form::params(d);
    if let (Some(o), Some(Value::Object(fixed))) = (params.as_object_mut(), d.fields.get("__params")) {
        o.extend(fixed.clone());
    }
    app.ui.dialog = None;
    // A tool's click-to-size shape (Flare) goes on the active perspective plane while the grid shows.
    let at = |x: &str, y: &str| Some(vectorcraft_geom::Point::new(params.get(x)?.as_f64()?, params.get(y)?.as_f64()?));
    if let Some((c, p)) =
        at("cx", "cy").or_else(|| at("x", "y")).and_then(|pt| vectorcraft_engine::perspective_click(&app.session, &cmd, &params, pt))
    {
        return app.run(&c, p);
    }
    app.run(&cmd, params)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_doc::{NodeKind, VerticalAlign};
    use vectorcraft_engine::Session;

    use super::*;

    #[test]
    fn area_type_options_choices_match_the_params_doc() {
        let doc = vectorcraft_engine::find_command("text.areaOptions").unwrap().params;
        for key in ["firstBaseline", "verticalAlign"] {
            let values: Vec<&str> = choices("text.areaOptions", key).unwrap().iter().map(|(_, v)| *v).collect();
            assert!(doc.contains(&format!("{key}?: {}", values.join("|"))), "{key}: {values:?}");
        }
        assert!(choices("text.areaOptions", "firstBaselineMin").is_none(), "a distance, not a choice");
        assert!(choices("text.areaOptions", "inset").is_none());
    }

    #[test]
    fn area_type_options_dialog_sets_the_vertical_alignment() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 400, "height": 400})).unwrap();
        let id = app.run("text.create", json!({"x": 20, "y": 20, "text": "Hello", "area": {"width": 200, "height": 200}})).unwrap()["id"]
            .as_u64()
            .unwrap();
        app.run("select.set", json!({"ids": [id]})).unwrap();
        crate::menus::invoke(&mut app, "text.areaOptions", json!({}));
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("verticalAlign"), "top");
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| crate::dialogs::show(&mut app, ui.ctx()));
        out.textures_delta.clear();
        app.ui.dialog.as_mut().unwrap().fields.insert("verticalAlign".into(), json!("center"));
        crate::dialogs::confirm(&mut app).unwrap();
        let NodeKind::Text(t) = &app.session.doc().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().kind else { panic!("text") };
        assert_eq!(t.area.vertical_align, VerticalAlign::Center);
    }

    /// Graph Type › Value Axis is a dropdown of the values `graph.setType` takes, and OK puts the
    /// value axis on the side picked.
    #[test]
    fn graph_type_value_axis_is_a_dropdown_of_its_sides() {
        let doc = vectorcraft_engine::find_command("graph.setType").unwrap().params;
        let values: Vec<&str> = choices("graph.setType", "valueAxis").unwrap().iter().map(|(_, v)| *v).collect();
        assert!(doc.contains(&format!("valueAxis?: {}", values.join("|"))), "{values:?}");
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 400, "height": 400})).unwrap();
        app.run("graph.create", json!({"type": "column", "x": 0, "y": 0, "width": 200, "height": 150})).unwrap();
        crate::menus::graph_dialog(&mut app, "graph.setType").unwrap();
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("valueAxis"), "left");
        app.ui.dialog.as_mut().unwrap().fields.insert("valueAxis".into(), json!("both"));
        crate::dialogs::confirm(&mut app).unwrap();
        assert_eq!(app.session.execute("graph.setType", &json!({})).unwrap()["valueAxis"], "both");
    }

    /// Paragraph panel menu › Hyphenation… and Justification… open on the paragraph's values and
    /// OK sets them; Single Word Justification is a dropdown of the command's values.
    #[test]
    fn hyphenation_and_justification_dialogs_set_the_paragraph_options() {
        let doc = vectorcraft_engine::find_command("text.justification").unwrap().params;
        let values: Vec<&str> = choices("text.justification", "singleWordJustification").unwrap().iter().map(|(_, v)| *v).collect();
        assert!(doc.contains(&format!("singleWordJustification?: {}", values.join("|"))), "{values:?}");
        assert_eq!(lengths("text.hyphenation"), ["zone"]);
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 400, "height": 400})).unwrap();
        let id = app.run("text.create", json!({"x": 20, "y": 20, "text": "Hello", "area": {"width": 200, "height": 200}})).unwrap()["id"]
            .as_u64()
            .unwrap();
        app.run("select.set", json!({"ids": [id]})).unwrap();
        crate::menus::invoke(&mut app, "text.hyphenation", json!({}));
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("__command"), "text.hyphenation");
        assert_eq!(app.ui.dialog.as_ref().unwrap().fields["minWord"], json!(6));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| crate::dialogs::show(&mut app, ui.ctx()));
        out.textures_delta.clear();
        app.ui.dialog.as_mut().unwrap().fields.insert("minWord".into(), json!(9));
        crate::dialogs::confirm(&mut app).unwrap();
        crate::panels::paragraph::options_dialog(&mut app, "text.justification").unwrap();
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("singleWordJustification"), "fullJustify");
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| crate::dialogs::show(&mut app, ui.ctx()));
        out.textures_delta.clear();
        app.ui.dialog.as_mut().unwrap().fields.insert("singleWordJustification".into(), json!("alignRight"));
        crate::dialogs::confirm(&mut app).unwrap();
        let NodeKind::Text(t) = &app.session.doc().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().kind else { panic!("text") };
        assert_eq!(t.para.hyphenation.min_word, 9);
        assert_eq!(t.para.justification.single_word, vectorcraft_doc::SingleWordJustify::AlignRight);
    }
}
