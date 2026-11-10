// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `#[derive(Leaf)]`: a unit-only enum is its variant names; any other type is its `Display`
//! and `FromStr`.

use core::fmt;
use core::str::FromStr;

use coqui_builder::describe::{Kind, LeafError, assert_round_trip};
use coqui_builder::{Describe, Leaf, Partial};

#[derive(Leaf, Debug, Clone, Copy, PartialEq)]
#[leaf(name = "Level")]
enum Level {
    #[schema(label = "Quiet")]
    Low,
    #[map(rename = "mid")]
    Middle,
    /// As loud as it goes.
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Leaf)]
#[leaf(ordered)]
struct Mhz(u32);

impl fmt::Display for Mhz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}MHz", self.0)
    }
}

impl FromStr for Mhz {
    type Err = ();

    fn from_str(text: &str) -> Result<Mhz, ()> {
        text.strip_suffix("MHz")
            .and_then(|n| n.parse().ok())
            .map(Mhz)
            .ok_or(())
    }
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Radio {
    #[map(default = Level::Middle)]
    level: Level,
    clock: Mhz,
}

#[test]
fn a_unit_enum_reads_and_writes_its_names() {
    assert_eq!(Level::TYPE, "Level");
    assert_eq!(Level::Middle.to_text(), "mid");
    assert_eq!(Level::from_text("High"), Ok(Level::High));
    assert!(Level::from_text("mid ").is_err());
    assert!(matches!(
        Level::from_text("Medium"),
        Err(LeafError::UnknownName)
    ));
    assert_round_trip(&[Level::Low, Level::Middle, Level::High]);
}

#[test]
fn the_labels_of_a_unit_enum_come_from_label_and_doc_comments() {
    let Kind::Leaf(info) = Level::description().kind().clone() else {
        panic!("a leaf")
    };
    assert_eq!(
        info.labels(),
        &[("Low", "Quiet"), ("High", "As loud as it goes.")]
    );
    assert_eq!(info.label("Low"), Some("Quiet"));
    assert_eq!(info.label("mid"), None);
    assert_eq!(info.label("nothing"), None);
}

#[test]
fn a_leaf_with_no_labelled_text_and_a_converted_leaf_have_none() {
    let Kind::Leaf(info) = Mhz::description().kind().clone() else {
        panic!("a leaf")
    };
    assert!(info.labels().is_empty());
    let d = Radio::description();
    let Kind::Group(group) = d.kind() else {
        panic!("a group")
    };
    let Kind::Leaf(level) = group.field("level").unwrap().description.kind() else {
        panic!("a leaf")
    };
    assert_eq!(level.label("Low"), Some("Quiet"));
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Speed {
    #[schema(label = "Whatever the line allows")]
    Auto,
    Fixed(u16),
}

#[test]
fn an_untagged_enums_unit_variant_keeps_its_label() {
    let Kind::Leaf(info) = Speed::description().kind().clone() else {
        panic!("a leaf")
    };
    assert_eq!(info.labels(), &[("Auto", "Whatever the line allows")]);
    assert_eq!(info.choices(), None);
}

#[test]
fn any_other_type_uses_display_and_from_str() {
    assert_eq!(Mhz::TYPE, "Mhz");
    assert_eq!(Mhz(80).to_text(), "80MHz");
    assert_eq!(Mhz::from_text("80MHz"), Ok(Mhz(80)));
    assert_eq!(
        Mhz::from_text("80"),
        Err(LeafError::Invalid { type_name: "Mhz" })
    );
    const { assert!(<Mhz as Leaf>::ORDERED) };
    const { assert!(!<Level as Leaf>::ORDERED) };
    assert_eq!(
        Mhz::order(&Mhz(1), &Mhz(2)),
        Some(core::cmp::Ordering::Less)
    );
    assert_round_trip(&[Mhz(0), Mhz(u32::MAX)]);
}

#[test]
fn a_derived_leaf_is_a_field_like_any_other() {
    let d = Radio::description();
    let Kind::Group(group) = d.kind() else {
        panic!("a group")
    };
    assert_eq!(
        group.field("level").unwrap().default.as_deref(),
        Some("mid")
    );
    let Kind::Leaf(info) = group.field("level").unwrap().description.kind() else {
        panic!("a leaf")
    };
    assert_eq!(info.choices(), Some(&["Low", "mid", "High"][..]));
    let mut p = Partial::<Radio>::new();
    p.set("clock", "433MHz").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Radio {
            level: Level::Middle,
            clock: Mhz(433)
        }
    );
    let mut p = Partial::<Radio>::new();
    assert!(p.set("level", "loud").is_err());
    assert!(p.set("clock", "433").is_err());
}

#[derive(Leaf, Debug, Clone, Copy, PartialEq, PartialOrd)]
#[leaf(ordered)]
enum ClientAuth {
    None,
    Optional,
    Required,
}

#[test]
fn an_ordered_unit_enum_orders_by_declaration() {
    use core::cmp::Ordering;
    const { assert!(<ClientAuth as Leaf>::ORDERED) };
    assert_eq!(
        ClientAuth::order(&ClientAuth::None, &ClientAuth::Required),
        Some(Ordering::Less)
    );
    assert_eq!(ClientAuth::from_text("Optional"), Ok(ClientAuth::Optional));
    assert!(ClientAuth::choices().is_some());
}
