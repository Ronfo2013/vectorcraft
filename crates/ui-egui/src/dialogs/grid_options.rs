//! Rectangular Grid Tool Options and Polar Grid Tool Options (double-click the tool, or click with
//! it), as the reference app documents them
//! (<https://helpx.adobe.com/illustrator/using/drawing-simple-lines-shapes.html>, "Draw
//! rectangular grids" and "Draw circular (polar) grids"): the tool's Default Size, its dividers
//! with their Skew, Use Outside Rectangle As Frame or Create Compound Path From Ellipses, and Fill
//! Grid. They are the tool's options (`tool.setOption {tool, values}`), which draw the next grid
//! dragged out, last across tool switches and are saved with the preferences; opened by a click,
//! OK then draws a grid of the Default Size from the click (its top-left corner) with them, along
//! General › Constrain Angle (on the active perspective plane while the grid shows).
//!
//! Fields: `width`, `height` (points); `rows`, `columns`, `rowSkew`, `columnSkew` (rectangular)
//! or `concentric`, `radial`, `concentricSkew`, `radialSkew` (polar), skews in percent; `frame`
//! or `compound`; `fill`; and `x`, `y` where a click opened it.

use serde_json::{Map, Value, json};
use vectorcraft_geom::shapes::MAX_GRID_SKEW;
use vectorcraft_tools::draw2::MAX_GRID_SIZE;

use super::document_setup::check;
use super::{DialogResult, DialogSpec, form, run_and_close};
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

/// The dialog kinds of the Rectangular and Polar Grid Tool Options.
pub const RECTANGULAR: &str = "rectangularGridOptions";
pub const POLAR: &str = "polarGridOptions";

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |d| heading(&d.kind).into(), body, confirm, min_width: 360.0, ..DialogSpec::FORM };

/// Width of the label column and of the fields.
const LABEL_W: f32 = 70.0;
const FIELD_W: f32 = 80.0;
/// The most dividers a grid takes either way.
const MAX_DIVIDERS: f64 = 999.0;

fn heading(kind: &str) -> &'static str {
    if kind == POLAR { tl!("Polar Grid Tool Options") } else { tl!("Rectangular Grid Tool Options") }
}

/// The grid tool a dialog kind sets the options of.
fn tool(kind: &str) -> &'static str {
    if kind == POLAR { "polarGrid" } else { "rectangularGrid" }
}

/// The option keys of grid tool `tool`: (count, skew) of the two divider kinds, then the
/// checkboxes.
fn keys(tool: &str) -> ([(&'static str, &'static str); 2], [&'static str; 2]) {
    if tool == "polarGrid" {
        ([("concentric", "concentricSkew"), ("radial", "radialSkew")], ["compound", "fill"])
    } else {
        ([("rows", "rowSkew"), ("columns", "columnSkew")], ["frame", "fill"])
    }
}

/// Open grid tool `tool`'s options with its values; `at`: where a click opened them, to draw a
/// grid there on OK. None for another tool.
pub fn open(app: &mut VectorcraftApp, tool: &str, at: Option<vectorcraft_geom::Point>) -> Option<Value> {
    let kind = match tool {
        "rectangularGrid" => RECTANGULAR,
        "polarGrid" => POLAR,
        _ => return None,
    };
    let mut fields = match app.session.tool_options_of(tool) {
        Value::Object(o) => o,
        _ => Map::new(),
    };
    if let Some(p) = at {
        fields.insert("x".into(), json!(p.x));
        fields.insert("y".into(), json!(p.y));
    }
    app.ui.dialog = Some(Dialog { kind: kind.into(), fields });
    Some(json!({ "dialog": kind }))
}

/// A row of a divider count and its skew, `ends` naming the sides a negative and a positive skew
/// weight the dividers towards.
fn dividers(ui: &mut egui::Ui, d: &mut Dialog, (count, skew): (&str, &str), ends: (&str, &str)) {
    widgets::label_row(ui, tl!("Number:"), LABEL_W, |ui| {
        if let Some(v) = widgets::range_field(ui, ("grid", count), d.f64(count, 5.0), 0.0..=MAX_DIVIDERS, "", 0, FIELD_W) {
            d.fields.insert(count.into(), json!(v as u64));
        }
    });
    widgets::label_row(ui, tl!("Skew:"), LABEL_W, |ui| {
        if let Some(v) = widgets::range_field(ui, ("grid", skew), d.f64(skew, 0.0), -MAX_GRID_SKEW..=MAX_GRID_SKEW, "%", 0, FIELD_W) {
            d.fields.insert(skew.into(), json!(v));
        }
        ui.label(format!("{} ↔ {}", tl!(ends.0), tl!(ends.1)));
    });
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let unit = app.session.general_unit();
    let polar = d.kind == POLAR;
    widgets::subheader(ui, tl!("Default Size"));
    for (key, label) in [("width", tl!("Width:")), ("height", tl!("Height:"))] {
        widgets::label_row(ui, label, LABEL_W, |ui| {
            if form::length(ui, d, key, unit, FIELD_W) {
                let v = d.f64(key, 100.0).clamp(0.0, MAX_GRID_SIZE);
                d.fields.insert(key.into(), json!(v));
            }
        });
    }
    let ([first, second], [option, fill]) = keys(tool(&d.kind));
    if polar {
        widgets::subheader(ui, tl!("Concentric Dividers"));
        dividers(ui, d, first, (tl!("In"), tl!("Out")));
        widgets::subheader(ui, tl!("Radial Dividers"));
        dividers(ui, d, second, (tl!("Bottom"), tl!("Top")));
        check(ui, d, option, tl!("Create Compound Path From Ellipses"));
    } else {
        widgets::subheader(ui, tl!("Horizontal Dividers"));
        dividers(ui, d, first, (tl!("Bottom"), tl!("Top")));
        widgets::subheader(ui, tl!("Vertical Dividers"));
        dividers(ui, d, second, (tl!("Left"), tl!("Right")));
        check(ui, d, option, tl!("Use Outside Rectangle As Frame"));
    }
    check(ui, d, fill, tl!("Fill Grid"));
    false
}

/// OK: set the tool's options; opened by a click, draw a grid of the Default Size there with them.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> DialogResult {
    let tool = tool(&d.kind);
    let ([first, second], [option, fill]) = keys(tool);
    let mut values = Map::new();
    for (count, skew) in [first, second] {
        values.insert(count.into(), json!(d.f64(count, 5.0).clamp(0.0, MAX_DIVIDERS) as u64));
        values.insert(skew.into(), json!(d.f64(skew, 0.0).clamp(-MAX_GRID_SKEW, MAX_GRID_SKEW)));
    }
    for key in [option, fill] {
        values.insert(key.into(), json!(d.bool(key)));
    }
    for key in ["width", "height"] {
        values.insert(key.into(), json!(d.f64(key, 100.0).clamp(0.0, MAX_GRID_SIZE)));
    }
    let at = d.fields.get("x").and_then(Value::as_f64).zip(d.fields.get("y").and_then(Value::as_f64));
    let options = run_and_close(app, "tool.setOption", json!({ "tool": tool, "values": values }))?;
    let Some((x, y)) = at else { return Ok(options) };
    let mut params = json!({ "x": x, "y": y });
    for (key, v) in &values {
        params[key.as_str()] = options.get(key).cloned().unwrap_or_else(|| v.clone());
    }
    let id = format!("shape.{tool}");
    match vectorcraft_engine::perspective_click(&app.session, &id, &params, vectorcraft_geom::Point::new(x, y)) {
        Some((id, p)) => app.run(&id, p),
        None => {
            let params = vectorcraft_engine::constrain_click(&app.session, &id, params);
            app.run(&id, params)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;
    use vectorcraft_engine::doc::NodeKind;

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 600})).unwrap();
        app
    }

    /// The labels the open dialog paints.
    fn painted(app: &mut VectorcraftApp) -> String {
        crate::tests_labels::painted_text(app, |app, ui| super::super::show(app, ui.ctx()))
    }

    /// The newest top-level object's paths and its bounding-box angle.
    fn newest(app: &VectorcraftApp) -> (Vec<vectorcraft_geom::PathData>, f64, Vec<vectorcraft_engine::doc::Node>) {
        let doc = &app.session.doc().unwrap().doc;
        let g = doc.layers[0].children().unwrap().last().unwrap();
        let NodeKind::Group { children, .. } = &g.kind else { panic!("a grid is a group") };
        let paths = children.iter().filter_map(|c| c.path_data().cloned()).collect();
        (paths, g.bbox_angle, children.iter().map(|c| (**c).clone()).collect())
    }

    #[test]
    fn the_toolbar_opens_the_options_and_ok_keeps_them_for_the_next_drag() {
        let mut app = app();
        app.run("tool.options", json!({"tool": "rectangularGrid"})).unwrap();
        let text = painted(&mut app);
        for label in
            ["Rectangular Grid Tool Options", "Default Size", "Width:", "Horizontal Dividers", "Vertical Dividers", "Skew:", "Use Outside Rectangle As Frame", "Fill Grid"]
        {
            assert!(text.contains(label), "{label} in {text}");
        }
        let d = app.ui.dialog.as_mut().unwrap();
        assert_eq!((d.f64("width", 0.0), d.f64("rows", 0.0), d.bool("frame"), d.bool("fill")), (100.0, 5.0, true, false));
        for (k, v) in [("rows", json!(2)), ("columns", json!(1)), ("frame", json!(false)), ("rowSkew", json!(900))] {
            d.fields.insert(k.into(), v);
        }
        let objects = |app: &VectorcraftApp| app.session.doc().unwrap().doc.layers[0].children().map_or(0, |c| c.len());
        super::super::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.is_none());
        assert_eq!(objects(&app), 0, "OK from the toolbar draws nothing");
        let o = app.session.tool_options_of("rectangularGrid");
        assert_eq!((o["rows"].as_u64(), o["frame"].as_bool(), o["rowSkew"].as_f64()), (Some(2), Some(false), Some(MAX_GRID_SKEW)));
        // A drag draws with them: two horizontal and one vertical divider, four outside lines.
        app.select_tool("rectangularGrid");
        let view = app.view_info();
        for (kind, x, y) in [(vectorcraft_tools::PointerKind::Down, 100.0, 100.0), (vectorcraft_tools::PointerKind::Drag, 200.0, 200.0), (vectorcraft_tools::PointerKind::Up, 200.0, 200.0)] {
            crate::canvas::dispatch(&mut app, &vectorcraft_tools::PointerEvent::new(kind, x, y), view);
        }
        let (paths, ..) = newest(&app);
        assert_eq!(paths.len(), 2 + 1 + 4);
        assert!(paths.iter().all(|p| !p.is_closed()));
    }

    /// A click opens the options with the tool's values and draws a grid of the Default Size
    /// from it: along General › Constrain Angle, its box turned with it; the polar grid's
    /// ellipses as one compound path filled even-odd with the current fill.
    #[test]
    fn a_click_draws_a_grid_of_the_default_size_there() {
        let mut app = app();
        crate::dialogs::open_tool_dialog(&mut app, "rectangularGrid", json!({"x": 300, "y": 300}));
        let d = app.ui.dialog.as_mut().unwrap();
        assert_eq!((d.kind.as_str(), d.f64("x", 0.0)), (RECTANGULAR, 300.0));
        d.fields.insert("width".into(), json!(60));
        d.fields.insert("height".into(), json!(40));
        super::super::confirm(&mut app).unwrap();
        let (paths, angle, _) = newest(&app);
        let b = paths.iter().filter_map(|p| p.bounds()).reduce(|a, b| a.union(b)).unwrap();
        assert_eq!((b.x0, b.y0, b.width(), b.height(), angle), (300.0, 300.0, 60.0, 40.0, 0.0));
        assert_eq!(app.session.tool_options_of("rectangularGrid")["width"], json!(60.0));

        app.run("prefs.set", json!({"key": "constrainAngle", "value": 90})).unwrap();
        crate::dialogs::open_tool_dialog(&mut app, "polarGrid", json!({"x": 100, "y": 100}));
        let d = app.ui.dialog.as_mut().unwrap();
        assert_eq!(d.kind, POLAR);
        let text = painted(&mut app);
        for label in ["Polar Grid Tool Options", "Concentric Dividers", "Radial Dividers", "Create Compound Path From Ellipses"] {
            assert!(text.contains(label), "{label} in {text}");
        }
        let d = app.ui.dialog.as_mut().unwrap();
        for (k, v) in [("width", json!(80)), ("height", json!(40)), ("compound", json!(true)), ("fill", json!(true)), ("concentric", json!(2))] {
            d.fields.insert(k.into(), v);
        }
        super::super::confirm(&mut app).unwrap();
        let (paths, angle, nodes) = newest(&app);
        // Along the turned axes from the click: 80 wide up the page, 40 to the right of it.
        let b = paths.iter().filter_map(|p| p.bounds()).reduce(|a, b| a.union(b)).unwrap();
        assert!((b.x0 - 100.0).abs() < 1e-9 && (b.y0 - 20.0).abs() < 1e-9 && (b.width() - 40.0).abs() < 1e-9 && (b.height() - 80.0).abs() < 1e-9, "{b:?}");
        assert_eq!(angle, 90.0);
        assert_eq!(paths[0].subpaths.len(), 3, "the ellipses as one compound path");
        assert!(matches!(nodes[0].kind, NodeKind::Path { rule: vectorcraft_geom::FillRule::EvenOdd, .. }));
        assert!(!nodes[0].appearance.fill_paint().is_none(), "Fill Grid");
    }
}
