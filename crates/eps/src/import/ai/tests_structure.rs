//! The layer tree the reader builds, checked against the structure that was written: random
//! nestings of layers, sublayers, groups, compound paths, clipping groups, hidden and locked
//! objects and guides, written from scratch in the syntax `read` documents, come back with the same
//! counts, names, nesting and order (G9: #951, #868, #779). And a large file reads in bounded time
//! (#758).

use std::sync::Arc;

use proptest::prelude::*;
use vectorcraft_doc::{Document, Node, NodeKind};

use super::*;

/// What a file holds, as written.
#[derive(Clone, Debug)]
enum Item {
    /// A rectangle, filled (`f`) or stroked (`S`); a guide is painted `(N) *`.
    Path {
        hidden: bool,
        locked: bool,
        guide: bool,
    },
    Group {
        name: String,
        hidden: bool,
        locked: bool,
        items: Vec<Item>,
    },
    /// `*u` … `*U` of `paths` filled rectangles.
    Compound {
        name: String,
        paths: usize,
    },
    /// `q`, a clipping path, `items`, `Q`.
    Clip {
        name: String,
        items: Vec<Item>,
    },
    /// A sublayer.
    Layer(Box<Layer>),
}

#[derive(Clone, Debug)]
struct Layer {
    name: String,
    visible: bool,
    locked: bool,
    printable: bool,
    items: Vec<Item>,
}

/// Writes the editing data of a tree, each rectangle at a place of its own.
struct Writer {
    out: String,
    next: usize,
}

impl Writer {
    fn rect(&mut self) -> String {
        let i = self.next as f64;
        self.next += 1;
        let (x, y) = ((i % 20.0) * 10.0, (i / 20.0).floor() * 10.0);
        super::tests::rect_path(x, y, x + 8.0, y + 8.0)
    }

    /// The object before gets `name` (as `AIArtName`).
    fn name(&mut self, name: &str) {
        self.out.push_str(&format!("%_/ArtDictionary :\n%_({name}) /String (AIArtName) ,\n%_;\n"));
    }

    fn state(&mut self, hidden: bool, locked: bool) {
        self.out.push_str(&format!("{} A\n{} Xw\n", u8::from(locked), u8::from(hidden)));
    }

    fn item(&mut self, item: &Item) {
        match item {
            Item::Path { hidden, locked, guide } => {
                self.state(*hidden, *locked);
                let r = self.rect();
                if *guide {
                    self.out.push_str(&format!("{r}(N) *\n"));
                } else {
                    self.out.push_str(&format!("0 0 0 1 k\n{r}f\n"));
                }
            }
            Item::Group { name, hidden, locked, items } => {
                self.state(*hidden, *locked);
                self.out.push_str("0 Ae\nu\n");
                for it in items {
                    self.item(it);
                }
                self.out.push_str("U\n");
                self.name(name);
                // The transparency and style markers that follow a group.
                self.out.push_str("0 1 0 0 0 Xy\n0 0 Xd\n9 () XW\n");
            }
            Item::Compound { name, paths } => {
                self.state(false, false);
                self.out.push_str("0 Ae\n*u\n0 0 0 1 k\n");
                for _ in 0..*paths {
                    let r = self.rect();
                    self.out.push_str(&format!("{r}f\n"));
                }
                self.out.push_str("*U\n");
                self.name(name);
            }
            Item::Clip { name, items } => {
                self.state(false, false);
                let r = self.rect();
                self.out.push_str(&format!("0 Ae\nq\n{r}h\nW\nn\n"));
                for it in items {
                    self.item(it);
                }
                self.out.push_str("Q\n");
                self.name(name);
            }
            Item::Layer(l) => self.layer(l),
        }
    }

    fn layer(&mut self, l: &Layer) {
        self.out.push_str(&format!(
            "%AI5_BeginLayer\n{} 1 {} {} 0 0 1 2 79 128 255 0 50 0 Lb\n({}) Ln\n",
            u8::from(l.visible),
            u8::from(!l.locked),
            u8::from(l.printable),
            l.name
        ));
        for it in &l.items {
            self.item(it);
        }
        self.out.push_str("LB\n%AI5_EndLayer--\n");
    }
}

fn written(layers: &[Layer]) -> String {
    let mut w = Writer { out: String::new(), next: 0 };
    for l in layers {
        w.layer(l);
    }
    super::tests::stream(&w.out)
}

/// `nodes` are `items` as written, in order (`shown`: their layers show, so hidden objects are
/// hidden objects and not hidden layers' art).
fn check_items(nodes: &[Arc<Node>], items: &[Item], at: &str) {
    assert_eq!(nodes.len(), items.len(), "{at}: {} objects for {} written: {:?}", nodes.len(), items.len(), names(nodes));
    for (i, (n, it)) in nodes.iter().zip(items).enumerate() {
        let at = format!("{at}/{i}");
        match it {
            Item::Path { hidden, locked, guide } => {
                assert!(matches!(n.kind, NodeKind::Path { guide: g, clipping: false, .. } if g == *guide), "{at}: {:?}", n.kind);
                assert_eq!((n.visible, n.locked), (!hidden, *locked), "{at}: state");
            }
            Item::Group { name, hidden, locked, items } => {
                let NodeKind::Group { children, clip: false } = &n.kind else { panic!("{at}: not a group: {:?}", n.kind) };
                assert_eq!(n.name.as_deref(), Some(name.as_str()), "{at}: name");
                assert_eq!((n.visible, n.locked), (!hidden, *locked), "{at}: state");
                check_items(children, items, &at);
            }
            Item::Compound { name, paths } => {
                let NodeKind::Compound { children, .. } = &n.kind else { panic!("{at}: not a compound path: {:?}", n.kind) };
                assert_eq!((n.name.as_deref(), children.len()), (Some(name.as_str()), *paths), "{at}");
            }
            Item::Clip { name, items } => {
                let NodeKind::Group { children, clip: true } = &n.kind else { panic!("{at}: not a clipping group: {:?}", n.kind) };
                assert_eq!(n.name.as_deref(), Some(name.as_str()), "{at}: name");
                assert!(matches!(children.first().map(|c| &c.kind), Some(NodeKind::Path { clipping: true, .. })), "{at}: the clip first");
                check_items(children.get(1..).unwrap_or_default(), items, &at);
            }
            Item::Layer(l) => check_layer(n, l, &at),
        }
    }
}

fn check_layer(n: &Node, l: &Layer, at: &str) {
    let NodeKind::Layer { children, printable, .. } = &n.kind else { panic!("{at}: not a layer: {:?}", n.kind) };
    assert_eq!(n.name.as_deref(), Some(l.name.as_str()), "{at}: name");
    assert_eq!((n.visible, n.locked, *printable), (l.visible, l.locked, l.printable), "{at}: {}", l.name);
    check_items(children, &l.items, &format!("{at}:{}", l.name));
}

fn names(nodes: &[Arc<Node>]) -> Vec<String> {
    nodes.iter().map(|n| n.name.clone().unwrap_or_default()).collect()
}

/// The structure of `layers` (as written) is that of `doc`.
fn check(doc: &Document, layers: &[Layer]) {
    assert_eq!(doc.layers.len(), layers.len(), "top-level layers: {:?}", names(&doc.layers));
    for (i, (n, l)) in doc.layers.iter().zip(layers).enumerate() {
        check_layer(n, l, &i.to_string());
    }
}

fn arb_name() -> impl Strategy<Value = String> {
    "[A-Za-z0-9][A-Za-z0-9 _-]{0,11}".prop_map(|s| s.trim_end().to_string()).prop_filter("named", |s| !s.is_empty())
}

fn arb_item() -> impl Strategy<Value = Item> {
    let leaf = prop_oneof![
        3 => (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(hidden, locked, guide)| Item::Path { hidden, locked, guide }),
        1 => (arb_name(), 1usize..4).prop_map(|(name, paths)| Item::Compound { name, paths }),
    ];
    leaf.prop_recursive(4, 24, 4, |inner| {
        prop_oneof![
            2 => (arb_name(), any::<bool>(), any::<bool>(), prop::collection::vec(inner.clone(), 0..4))
                .prop_map(|(name, hidden, locked, items)| Item::Group { name, hidden, locked, items }),
            1 => (arb_name(), prop::collection::vec(inner.clone(), 0..3)).prop_map(|(name, items)| Item::Clip { name, items }),
            1 => arb_layer(inner).prop_map(|l| Item::Layer(Box::new(l))),
        ]
    })
}

fn arb_layer(items: impl Strategy<Value = Item> + 'static) -> impl Strategy<Value = Layer> {
    (arb_name(), any::<bool>(), any::<bool>(), any::<bool>(), prop::collection::vec(items, 0..4))
        .prop_map(|(name, visible, locked, printable, items)| Layer { name, visible, locked, printable, items })
}

fn arb_layers() -> impl Strategy<Value = Vec<Layer>> {
    prop::collection::vec(arb_layer(arb_item()), 1..5)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Whatever the nesting, the layers, groups, compound paths, clipping groups and guides come
    /// back as written: by the reader, and through the import of a `.ai` saved without its PDF
    /// part (nothing to compare the layers with).
    #[test]
    fn random_nestings_come_back_as_written(layers in arb_layers()) {
        let data = written(&layers);
        let s = match read(data.as_bytes()) {
            Ok(s) => s,
            Err(e) => panic!("{e}\n{data}"),
        };
        check(&s.doc, &layers);
        let (doc, notes) = crate::ai_alone(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
        check(&doc, &layers);
        prop_assert!(notes.is_empty(), "{notes:?}");
    }
}

/// Fourteen layers, each with sublayers and groups, come back as fourteen layers (#951 saw 109,
/// #868 a flat tree) with their hidden, locked and non-printing states (#779).
#[test]
fn fourteen_layers_with_sublayers_and_groups_stay_fourteen() {
    let path = |guide: bool| Item::Path { hidden: false, locked: false, guide };
    let group = |name: &str, items: Vec<Item>| Item::Group { name: name.into(), hidden: false, locked: false, items };
    let layers: Vec<Layer> = (1..=14)
        .map(|i| Layer {
            name: format!("Unit {i}"),
            visible: i % 5 != 0,
            locked: i % 7 == 0,
            printable: i != 14,
            items: vec![
                group("Figures", vec![path(false), group("Inner", vec![path(false), Item::Compound { name: "Ring".into(), paths: 2 }])]),
                Item::Layer(Box::new(Layer {
                    name: format!("Captions {i}"),
                    visible: true,
                    locked: false,
                    printable: true,
                    items: vec![Item::Clip { name: "Window".into(), items: vec![path(false)] }, path(true)],
                })),
                Item::Path { hidden: true, locked: true, guide: false },
            ],
        })
        .collect();
    let data = written(&layers);
    let s = read(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&s.doc, &layers);
    assert_eq!(s.doc.layers.len(), 14);
    let guides = count(&s.doc.layers, &|n| matches!(n.kind, NodeKind::Path { guide: true, .. }));
    assert_eq!(guides, 14, "a guide on each caption sublayer");
}

/// Operators the reader doesn't know, with their operands, are state it doesn't model: skipped,
/// the structure round them kept, a note naming them (not a loss). One met while a path is being
/// built could paint it, so that still stops the reader; so do unbalanced containers.
#[test]
fn unknown_state_operators_are_skipped_with_a_note() {
    let path = Item::Path { hidden: false, locked: false, guide: false };
    let layers = vec![Layer {
        name: "Art".into(),
        visible: true,
        locked: false,
        printable: true,
        items: vec![Item::Group { name: "G".into(), hidden: false, locked: false, items: vec![path.clone(), path.clone()] }, path.clone()],
    }];
    // The operators between the objects, inside the group: a name, numbers and a string, an array.
    let data = written(&layers).replace("U\n%_/ArtDictionary", "/Fancy X^\n%_1 2 (s) Xz\n%_[1 2] /Key X#\nU\n%_/ArtDictionary");
    assert!(data.contains("X^"), "{data}");
    let s = read(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&s.doc, &layers);
    assert_eq!(s.skipped.iter().cloned().collect::<Vec<_>>(), ["X#", "X^", "Xz"]);
    assert!(s.warnings.is_empty(), "the importer says whether the page comparison covers them: {:?}", s.warnings);
    // Without a page to compare with, what they set may be art the document lacks: a loss.
    let (doc, notes) = crate::ai_alone(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&doc, &layers);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].starts_with(super::read::OPERATORS_SKIPPED_UNCHECKED) && notes[0].contains("`X^`") && crate::is_loss(&notes[0]), "{notes:?}");
    // On a layer that doesn't show, that layer's art counts as unread (a loss), as before.
    let hidden = written(&[Layer { visible: false, ..layers[0].clone() }]).replace("U\n%_/ArtDictionary", "/Fancy X^\nU\n%_/ArtDictionary");
    let s = read(hidden.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    assert!(s.skipped.is_empty() && s.hidden_unread.contains("`X^`"), "{:?} {:?}", s.skipped, s.hidden_unread);
    // While a path is being built.
    let open = written(&layers).replace(" m\n", " m\n/Fancy X^\n");
    assert!(read(open.as_bytes()).unwrap_err().contains("`X^`"));
    // Unbalanced containers still stop it.
    assert!(read(written(&layers).replace("U\n%_/ArtDictionary", "U\nU\n%_/ArtDictionary").as_bytes()).is_err());
}

/// An object of a kind the reader doesn't read, `/Kind X!`, its data (`X#` entries, bare arrays,
/// on `%_` lines), `/End X!`, then its style markers: left out, the objects round it kept, a note
/// counting it by kind that counts as a loss (the file has art the document lacks). An object that
/// doesn't end is a damaged file.
#[test]
fn objects_of_kinds_it_doesnt_read_are_left_out_with_a_loss_note() {
    let path = Item::Path { hidden: false, locked: false, guide: false };
    let layers = vec![Layer {
        name: "Art".into(),
        visible: true,
        locked: false,
        printable: true,
        items: vec![Item::Group { name: "G".into(), hidden: false, locked: false, items: vec![path.clone(), path.clone()] }],
    }];
    let object = "/Mesh X!\n%_2 /Version X#\n%_/Cartesian /Type X#\n%_[2 2] /Size X#\n%_/Data X#\n%_[0 0] /P X#\n%_[0] /R X#\n%_/DeviceRGB /CS X#\n\
                  %_[1 2 3 4 5 6 1]\n%_[1 2 3 4 5 6 1]\n%_[0.5 0.5 0.5] /S X#\n%_/E X#\n/End X!\n0 0 Xd\n6 (Style) XW\n";
    // Between the group's two paths.
    let data = written(&layers).replacen("f\n0 A\n0 Xw\n", &format!("f\n{object}0 A\n0 Xw\n"), 1);
    assert!(data.contains("/Mesh X!"), "{data}");
    let s = read(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&s.doc, &layers);
    let note = s.warnings.iter().find(|w| w.starts_with(super::read::OBJECTS_LEFT_OUT)).unwrap_or_else(|| panic!("{:?}", s.warnings));
    assert!(note.ends_with("(Mesh: 1)"), "{note}");
    assert!(crate::is_loss(note));
    assert!(!s.warnings.iter().any(|w| w.starts_with(super::read::OPERATORS_SKIPPED)), "{:?}", s.warnings);
    // Two of them: both counted.
    let twice = data.replacen("/End X!\n", "/End X!\n/Mesh X!\n%_[1] /P X#\n/End X!\n", 1);
    let s = read(twice.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    assert!(s.warnings.iter().any(|w| w.ends_with("(Mesh: 2)")), "{:?}", s.warnings);
    // An object inside the object is its data: its end doesn't end the outer one, whose data
    // after it (bare arrays, a `/Document` dictionary) stays nothing: the artboards are the crop
    // marks', the second path is still read, one object counted.
    let nested = data.replacen(
        "%_/E X#\n",
        "%_/Inner X!\n%_[1 2] /P X#\n%_/End X!\n%_[1 2 3 4 5 6 1]\n%_[1 2 3 4 5 6 1]\n\
         %_/Document :\n%_/Dictionary :\n%_/Array :\n%_/Dictionary :\n%_0 50 /RealPointRelToROrigin\n%_ (PositionPoint1) ,\n\
         %_50 0 /RealPointRelToROrigin\n%_ (PositionPoint2) ,\n%_; ,\n%_; (ArtboardArray) ,\n%_;\n%_/E X#\n",
        1,
    );
    let s = read(nested.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&s.doc, &layers);
    assert!(s.warnings.iter().any(|w| w.ends_with("(Mesh: 1)")), "{:?}", s.warnings);
    assert_eq!(s.doc.artboards.len(), 1);
    assert_eq!(s.doc.artboards[0].rect, kurbo::Rect::new(0.0, 0.0, 200.0, 100.0), "the crop marks, not the dictionary in the object");
    // One that doesn't end swallows the layer's end: damaged.
    let e = read(data.replacen("/End X!\n", "", 1).as_bytes()).unwrap_err();
    assert!(e.contains("Mesh") || e.contains("inside a layer"), "{e}");
}

/// How many nodes under `nodes` (them included) satisfy `p`.
fn count(nodes: &[Arc<Node>], p: &dyn Fn(&Node) -> bool) -> usize {
    nodes.iter().map(|n| usize::from(p(n)) + n.children().map_or(0, |c| count(c, p))).sum()
}

/// A file of about 50 000 objects in layers, sublayers, groups and clipping groups reads whole,
/// with its structure (#758: nothing in the reader grows with the square of the file; it read and
/// imported in 0.6 s in the optimized test profile when this was written, but timings under CI's
/// load are noise, so the test doesn't assert one).
#[test]
fn fifty_thousand_objects_in_nested_layers_read_whole() {
    let path = Item::Path { hidden: false, locked: false, guide: false };
    let layers: Vec<Layer> = (0..50)
        .map(|i| Layer {
            name: format!("Layer {i}"),
            visible: true,
            locked: false,
            printable: true,
            items: (0..10)
                .map(|j| {
                    let inner: Vec<Item> = (0..5)
                        .map(|_| Item::Group { name: "G".into(), hidden: false, locked: false, items: vec![path.clone(); 18] })
                        .chain(std::iter::once(Item::Clip { name: "C".into(), items: vec![path.clone(); 8] }))
                        .collect();
                    if j % 2 == 0 {
                        Item::Group { name: format!("Group {j}"), hidden: false, locked: false, items: inner }
                    } else {
                        Item::Layer(Box::new(Layer { name: format!("Sub {j}"), visible: true, locked: false, printable: true, items: inner }))
                    }
                })
                .collect(),
        })
        .collect();
    let data = written(&layers);
    let s = read(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&s.doc, &layers);
    let paths = count(&s.doc.layers, &|n| matches!(n.kind, NodeKind::Path { .. }));
    assert!(paths > 49_000, "{paths}");
    // Through the import too (the text slots, the commented copies and the notes).
    let (doc, notes) = crate::ai_alone(data.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    check(&doc, &layers);
    assert!(notes.is_empty(), "{notes:?}");
}
