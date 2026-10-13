//! Reading [`NodeKind`] without buffering it.
//!
//! `NodeKind` is written internally tagged (`{"type": "group", "children": [...]}`). serde's derived
//! reader for that form first copies the whole object into an in-memory tree, because the tag may
//! come anywhere, and then reads the variant from the copy. A container's kind holds its children,
//! so every level of nesting copied the art below it again: half the time opening a 50,000-path
//! document went there (G15). The writer puts the tag first, so here the tag is read and the rest of
//! the object goes straight to the variant. Objects whose tag isn't first (hand-written JSON) are
//! read through a `serde_json::Value`, as before.
//!
//! [`NodeKindWire`] is the same enum externally tagged: a remote derive, so the compiler checks that
//! it lists exactly `NodeKind`'s variants and fields. Its field attributes must match `NodeKind`'s.

use std::fmt;
use std::sync::Arc;

use serde::de::value::MapAccessDeserializer;
use serde::de::{self, DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer, MapAccess, VariantAccess, Visitor};
use serde::{Deserialize, forward_to_deserialize_any};
use vectorcraft_geom::{Affine, FillRule, PathData};

use crate::live::{BlendSpec, EnvelopeKind, GradientMesh};
use crate::node::{ImageObject, LayerColor, LiveShape, Node, NodeKind};
use crate::pattern::RepeatSpec;
use crate::text::TextObject;

const TAG: &str = "type";

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(remote = "NodeKind", rename_all = "lowercase")]
enum NodeKindWire {
    Layer {
        color: LayerColor,
        #[serde(default)]
        template: bool,
        #[serde(default = "yes")]
        printable: bool,
        children: Vec<Arc<Node>>,
        #[serde(default)]
        clip: bool,
        #[serde(default = "yes")]
        preview: bool,
        #[serde(default)]
        dim_images: Option<u8>,
    },
    Group {
        children: Vec<Arc<Node>>,
        #[serde(default)]
        clip: bool,
    },
    Path {
        path: PathData,
        #[serde(default)]
        rule: FillRule,
        #[serde(default)]
        live: Option<LiveShape>,
        #[serde(default)]
        clipping: bool,
        #[serde(default)]
        guide: bool,
    },
    Compound {
        children: Vec<Arc<Node>>,
        #[serde(default)]
        rule: FillRule,
    },
    Text(Box<TextObject>),
    Image(ImageObject),
    SymbolInstance {
        symbol: String,
        xf: Affine,
    },
    Blend {
        children: Vec<Arc<Node>>,
        #[serde(default)]
        spec: BlendSpec,
    },
    Envelope {
        content: Vec<Arc<Node>>,
        kind: EnvelopeKind,
        #[serde(default = "crate::live::default_fidelity")]
        fidelity: f64,
        #[serde(default)]
        editing: bool,
        #[serde(default)]
        options: crate::live::EnvelopeOptions,
        #[serde(default)]
        frame: Affine,
    },
    Mesh(GradientMesh),
    Repeat(RepeatSpec),
    PlacedDocument(Box<crate::placed_document::PlacedDocument>),
}

impl<'de> Deserialize<'de> for NodeKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(KindVisitor)
    }
}

struct KindVisitor;

impl<'de> Visitor<'de> for KindVisitor {
    type Value = NodeKind;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an object kind (a map with a `type`)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NodeKind, A::Error> {
        let first: Option<String> = map.next_key()?;
        if first.as_deref() == Some(TAG) {
            let tag: String = map.next_value()?;
            return NodeKindWire::deserialize(TaggedRest { tag, map });
        }
        // The tag isn't first: gather the object, then read it externally tagged.
        let mut rest = serde_json::Map::new();
        if let Some(k) = first {
            rest.insert(k, map.next_value()?);
        }
        while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
            rest.insert(k, v);
        }
        let tag = match rest.remove(TAG) {
            Some(serde_json::Value::String(t)) => t,
            Some(_) => return Err(de::Error::custom("`type` must be a string")),
            None => return Err(de::Error::missing_field(TAG)),
        };
        let mut outer = serde_json::Map::new();
        outer.insert(tag, serde_json::Value::Object(rest));
        NodeKindWire::deserialize(serde_json::Value::Object(outer)).map_err(de::Error::custom)
    }
}

/// The rest of a map whose tag was read: an enum whose variant is the tag and whose content is the
/// rest of the map.
struct TaggedRest<A> {
    tag: String,
    map: A,
}

impl<'de, A: MapAccess<'de>> Deserializer<'de> for TaggedRest<A> {
    type Error = A::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, A::Error> {
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option
        unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, A: MapAccess<'de>> EnumAccess<'de> for TaggedRest<A> {
    type Error = A::Error;
    type Variant = Rest<A>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Rest<A>), A::Error> {
        let v = seed.deserialize(self.tag.into_deserializer())?;
        Ok((v, Rest(self.map)))
    }
}

struct Rest<A>(A);

impl<'de, A: MapAccess<'de>> VariantAccess<'de> for Rest<A> {
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), A::Error> {
        Err(de::Error::custom("unexpected unit variant"))
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, A::Error> {
        seed.deserialize(MapAccessDeserializer::new(self.0))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, A::Error> {
        Err(de::Error::custom("unexpected tuple variant"))
    }

    fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value, A::Error> {
        visitor.visit_map(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(v: serde_json::Value) -> NodeKind {
        let s = v.to_string();
        let streamed: NodeKind = serde_json::from_str(&s).unwrap();
        let from_value: NodeKind = serde_json::from_value(v).unwrap();
        assert_eq!(streamed, from_value);
        streamed
    }

    #[test]
    fn tag_anywhere_in_the_object() {
        let first = read(json!({"type": "group", "children": [], "clip": true}));
        let last = read(json!({"children": [], "clip": true, "type": "group"}));
        assert_eq!(first, NodeKind::Group { children: vec![], clip: true });
        assert_eq!(first, last);
    }

    #[test]
    fn defaults_and_newtype_variants_round_trip() {
        let kinds = [
            NodeKind::Group {
                children: vec![Arc::new(Node::path(crate::NodeId(7), PathData::default(), crate::Appearance::default_art()))],
                clip: false,
            },
            NodeKind::Path { path: PathData::default(), rule: FillRule::EvenOdd, live: None, clipping: true, guide: false },
            NodeKind::SymbolInstance { symbol: "s".into(), xf: Affine::translate((1.0, 2.0)) },
        ];
        for k in kinds {
            assert_eq!(read(serde_json::to_value(&k).unwrap()), k);
        }
        // A layer written with only its required fields gets the defaults.
        let layer = read(json!({"type": "layer", "color": {"Preset": 1}, "children": []}));
        assert!(matches!(layer, NodeKind::Layer { printable: true, preview: true, template: false, clip: false, dim_images: None, .. }));
    }

    #[test]
    fn bad_tags_are_errors() {
        assert!(serde_json::from_str::<NodeKind>(r#"{"type": "nope"}"#).is_err());
        assert!(serde_json::from_str::<NodeKind>(r#"{"children": []}"#).is_err());
        assert!(serde_json::from_str::<NodeKind>(r#"{"children": [], "type": 3}"#).is_err());
        assert!(serde_json::from_str::<NodeKind>(r#"[]"#).is_err());
    }

    #[test]
    fn deep_nesting_reads() {
        // Nested groups (to serde_json's depth limit): each level used to copy everything below it.
        let mut v = json!({"type": "group", "children": []});
        for i in 0..40 {
            v = json!({"type": "group", "children": [{"id": i, "kind": v}]});
        }
        let mut k = read(v);
        let mut depth = 0;
        while let NodeKind::Group { children, .. } = k {
            let Some(c) = children.first() else { break };
            k = c.kind.clone();
            depth += 1;
        }
        assert_eq!(depth, 40);
    }
}
