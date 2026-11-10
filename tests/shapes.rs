// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Every shape the derive takes: what it describes, and what a `Partial` builds from it.

use std::collections::{BTreeMap, HashMap};
use std::net::Ipv4Addr;

use coqui_builder::describe::{Field, Kind, Stricter};
use coqui_builder::{Datum, Describe, Description, Entries, Inspect, KeyPath, Partial, Segment};

fn keys(description: &Description) -> Vec<String> {
    match description.kind() {
        Kind::Group(group) => group.fields.iter().map(|f| f.key.clone()).collect(),
        other => panic!("not a group: {other:?}"),
    }
}

fn field<'a>(description: &'a Description, key: &str) -> &'a Field {
    match description.kind() {
        Kind::Group(group) => group.field(key).unwrap_or_else(|| panic!("no field {key}")),
        other => panic!("not a group: {other:?}"),
    }
}

fn paths(missing: &coqui_builder::Missing) -> Vec<String> {
    missing.keys().iter().map(ToString::to_string).collect()
}

fn path(text: &str) -> Vec<Segment> {
    text.parse::<KeyPath>().unwrap().segments().to_vec()
}

fn ty(description: &Description) -> &'static str {
    match description.kind() {
        Kind::Leaf(info) => info.ty(),
        other => panic!("not a leaf: {other:?}"),
    }
}

/// How long a source may wait.
#[derive(Describe, Debug, Clone, PartialEq)]
struct Waiting {
    /// Per source.
    #[map(default = 1)]
    per_source: u32,
    #[map(default = 4)]
    max: u32,
}

impl Default for Waiting {
    fn default() -> Self {
        Waiting {
            per_source: 1,
            max: 4,
        }
    }
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Peer {
    host: String,
    port: Option<u16>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Tls {
    ca: String,
    #[map(default)]
    verify: bool,
}

/// The settings of a thing.
#[derive(Describe, Debug, Clone, PartialEq)]
struct Settings {
    name: String,
    #[map(default)]
    waiting: Waiting,
    #[map(default)]
    tags: Vec<String>,
    #[map(default)]
    peers: Vec<Peer>,
    #[map(default)]
    env: BTreeMap<String, String>,
    tls: Option<Tls>,
}

#[test]
fn a_struct_is_a_group_of_its_fields_in_order() {
    let d = Settings::description();
    assert_eq!(keys(&d), ["name", "waiting", "tags", "peers", "env", "tls"]);
    assert_eq!(
        d.notes().description.as_deref(),
        Some("The settings of a thing.")
    );
    assert!(field(&d, "name").required);
    assert!(!field(&d, "waiting").required);
    assert!(field(&d, "tls").optional && !field(&d, "tls").required);
    assert!(matches!(
        field(&d, "tags").description.kind(),
        Kind::List(_)
    ));
    assert!(matches!(field(&d, "env").description.kind(), Kind::Map(_)));
    assert_eq!(ty(&field(&d, "name").description), "String");
}

#[test]
fn a_nested_group_carries_the_default_texts_of_its_leaves() {
    let d = Settings::description();
    let waiting = &field(&d, "waiting").description;
    assert_eq!(field(waiting, "per_source").default.as_deref(), Some("1"));
    assert_eq!(field(waiting, "max").default.as_deref(), Some("4"));
    assert!(!field(waiting, "per_source").required);
    assert_eq!(
        field(waiting, "per_source")
            .description
            .notes()
            .description
            .as_deref(),
        Some("Per source.")
    );
    assert_eq!(field(&d, "tags").default_items, Some(vec![]));
}

#[test]
fn building_with_nothing_set_lists_every_required_key() {
    let missing = Partial::<Settings>::new().build().unwrap_err();
    assert_eq!(paths(&missing), ["name"]);
    assert_eq!(missing.to_string(), "missing settings: name");
}

#[test]
fn building_lists_every_missing_key_at_once_and_not_just_the_first() {
    let mut p = Partial::<Settings>::new();
    p.set("name", "n").unwrap();
    p.set("tls.verify", "true").unwrap();
    p.set("peers[1].port", "80").unwrap();
    let missing = p.build().unwrap_err();
    assert_eq!(
        paths(&missing),
        ["peers[0].host", "peers[1].host", "tls.ca"]
    );
}

#[test]
fn a_partial_sets_leaves_lists_maps_and_optional_groups_by_path() {
    let mut p = Partial::<Settings>::new();
    p.set("name", "n").unwrap();
    p.set("waiting.per_source", "3").unwrap();
    p.set_items("tags", ["a", "b"]).unwrap();
    p.set("peers[0].host", "h0").unwrap();
    p.set("peers[1].host", "h1").unwrap();
    p.set("peers[1].port", "8080").unwrap();
    p.set("env.PATH", "/bin").unwrap();
    p.set("env[\"a.b\"]", "1").unwrap();
    p.set("tls.ca", "ca.pem").unwrap();
    let built = p.build().unwrap();
    assert_eq!(
        built.waiting,
        Waiting {
            per_source: 3,
            max: 4
        }
    );
    assert_eq!(built.tags, ["a", "b"]);
    assert_eq!(built.peers.len(), 2);
    assert_eq!(built.peers[1].port, Some(8080));
    assert_eq!(built.peers[0].port, None);
    assert_eq!(built.env["PATH"], "/bin");
    assert_eq!(built.env["a.b"], "1");
    assert_eq!(
        built.tls,
        Some(Tls {
            ca: "ca.pem".into(),
            verify: false
        })
    );
}

#[test]
fn an_unknown_key_and_a_refused_text_are_named_by_path_and_never_quoted() {
    let mut p = Partial::<Settings>::new();
    assert_eq!(
        p.set("nope", "1").unwrap_err().to_string(),
        "nope: no such setting"
    );
    assert_eq!(
        p.set("waiting.nope", "1").unwrap_err().to_string(),
        "waiting.nope: no such setting"
    );
    let wrong = p.set("waiting.max", "many").unwrap_err();
    assert_eq!(wrong.at().to_string(), "waiting.max");
    assert!(!wrong.to_string().contains("many"));
    assert!(p.set("peers.host", "x").is_err());
    assert!(p.set("name..x", "x").is_err());
    assert!(p.set_items("name", ["x"]).is_err());
}

#[test]
fn a_value_becomes_a_partial_with_every_key_set() {
    let value = Settings {
        name: "n".into(),
        waiting: Waiting {
            per_source: 9,
            max: 9,
        },
        tags: vec!["t".into()],
        peers: vec![Peer {
            host: "h".into(),
            port: None,
        }],
        env: BTreeMap::new(),
        tls: None,
    };
    assert_eq!(Partial::from(value.clone()).build().unwrap(), value);
}

// ---- review findings ----

#[test]
fn a_position_far_past_the_end_is_refused_and_a_failed_set_adds_nothing() {
    let mut p = Partial::<Settings>::new();
    p.set("name", "n").unwrap();
    assert!(p.set("tags[18446744073709551615]", "x").is_err());
    assert!(p.set("tags[1000000000000]", "x").is_err());
    assert!(p.set("peers[0].port", "not a port").is_err());
    assert!(p.set("env.A.x", "1").is_err());
    let built = p.build().unwrap();
    assert!(built.tags.is_empty() && built.peers.is_empty() && built.env.is_empty());
}

#[test]
fn clearing_an_optional_group_forgets_what_was_set_below_it() {
    let mut p = Partial::<Settings>::new();
    p.set("name", "n").unwrap();
    p.set("tls.ca", "ca").unwrap();
    p.clear("tls").unwrap();
    assert_eq!(p.build().unwrap().tls, None);
}

#[test]
fn clearing_a_position_and_a_map_entry_forgets_them() {
    let mut p = Partial::<Settings>::new();
    p.set("name", "n").unwrap();
    p.set_items("tags", ["a", "b", "c"]).unwrap();
    p.set("env.A", "1").unwrap();
    p.set("env.B", "2").unwrap();
    p.clear("tags[1]").unwrap();
    p.clear("env.A").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.tags, ["a", "c"]);
    assert_eq!(built.env.len(), 1);
}

#[test]
fn a_later_partial_overlays_only_the_keys_it_set_and_replaces_a_list_whole() {
    let mut base = Partial::<Settings>::new();
    base.set("name", "base").unwrap();
    base.set("waiting.max", "10").unwrap();
    base.set_items("tags", ["a", "b"]).unwrap();
    base.set("env.A", "1").unwrap();
    let mut later = Partial::<Settings>::new();
    later.set("waiting.per_source", "2").unwrap();
    later.set_items("tags", Vec::<&str>::new()).unwrap();
    later.set("env.B", "2").unwrap();
    later.set("tls.ca", "ca").unwrap();
    base.overlay(later);
    let built = base.build().unwrap();
    assert_eq!(built.name, "base");
    assert_eq!(
        built.waiting,
        Waiting {
            per_source: 2,
            max: 10
        }
    );
    assert!(built.tags.is_empty(), "an empty list replaces the list");
    assert_eq!(built.env.len(), 2);
    assert_eq!(built.tls.map(|t| t.ca), Some("ca".into()));
}

#[test]
fn a_partial_answers_what_was_set_else_the_default_and_nothing_below_an_absent_group() {
    let mut p = Partial::<Settings>::new();
    p.set("waiting.max", "10").unwrap();
    assert_eq!(p.at(&path("waiting.max")), Some(Datum::new("u32", "10")));
    assert_eq!(
        p.at(&path("waiting.per_source")),
        Some(Datum::new("u32", "1"))
    );
    assert_eq!(p.at(&path("name")), None);
    assert_eq!(p.at(&path("tls.verify")), None);
    assert_eq!(p.entries(&path("tags")), Some(Entries::Len(0)));
    p.set("tls.ca", "ca").unwrap();
    assert_eq!(p.at(&path("tls.verify")), Some(Datum::new("bool", "false")));
}

// ---- generics, flatten, skip, rename ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Pair<T> {
    first: T,
    #[map(default = 7)]
    second: u8,
}

#[test]
fn a_generic_struct_is_described_through_its_parameters() {
    assert_eq!(keys(&Pair::<u16>::description()), ["first", "second"]);
    let mut p = Partial::<Pair<u16>>::new();
    p.set("first", "5").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Pair {
            first: 5,
            second: 7
        }
    );
    assert_eq!(
        ty(&field(&Pair::<String>::description(), "first").description),
        "String"
    );
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Inner {
    x: u8,
    #[map(default = 2)]
    y: u8,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Outer {
    a: u8,
    #[map(flatten)]
    inner: Inner,
}

#[test]
fn a_flattened_struct_sits_beside_the_others() {
    assert_eq!(keys(&Outer::description()), ["a", "x", "y"]);
    let mut p = Partial::<Outer>::new();
    p.set("a", "1").unwrap();
    p.set("x", "2").unwrap();
    let built = p.build().unwrap();
    assert_eq!(
        built,
        Outer {
            a: 1,
            inner: Inner { x: 2, y: 2 }
        }
    );
    assert_eq!(Partial::<Outer>::new().build().unwrap_err().keys().len(), 2);
    assert!(Partial::<Outer>::new().set("z", "1").is_err());
    assert_eq!(built.at(&path("x")), Some(Datum::new("u8", "2")));
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Skipping {
    a: u8,
    #[map(skip)]
    cache: Vec<u8>,
}

#[test]
fn a_skipped_field_is_no_key_and_is_built_from_default() {
    assert_eq!(keys(&Skipping::description()), ["a"]);
    let mut p = Partial::<Skipping>::new();
    p.set("a", "1").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Skipping {
            a: 1,
            cache: vec![]
        }
    );
    assert!(Partial::<Skipping>::new().set("cache", "1").is_err());
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(rename_all = "kebab-case")]
struct Kebab {
    view_only: bool,
    #[map(rename = "Max")]
    max_len: u32,
}

#[test]
fn rename_all_and_rename_set_the_keys() {
    assert_eq!(keys(&Kebab::description()), ["view-only", "Max"]);
    let mut p = Partial::<Kebab>::new();
    p.set("view-only", "yes").unwrap();
    p.set("Max", "3").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Kebab {
            view_only: true,
            max_len: 3
        }
    );
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(deny_unknown_fields)]
struct Strict {
    a: u8,
}

#[test]
fn deny_unknown_fields_is_a_flag_of_the_group() {
    let Kind::Group(group) = Strict::description().kind().clone() else {
        panic!("a group")
    };
    assert!(group.deny_unknown_fields);
    let Kind::Group(group) = Waiting::description().kind().clone() else {
        panic!("a group")
    };
    assert!(!group.deny_unknown_fields);
}

// ---- newtypes ----

#[derive(Describe, Debug, Clone, Copy, PartialEq)]
struct Port(u16);

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(transparent)]
struct Label {
    text: String,
    #[map(skip)]
    cached: u8,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Endpoint {
    port: Port,
    #[map(default = Label { text: "x".into(), cached: 0 })]
    label: Label,
}

#[test]
fn a_newtype_and_a_transparent_struct_are_their_field() {
    let d = Endpoint::description();
    assert_eq!(ty(&field(&d, "port").description), "u16");
    assert_eq!(ty(&field(&d, "label").description), "String");
    assert_eq!(field(&d, "label").default.as_deref(), Some("x"));
    let mut p = Partial::<Endpoint>::new();
    p.set("port", "80").unwrap();
    p.set("label", "y").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Endpoint {
            port: Port(80),
            label: Label {
                text: "y".into(),
                cached: 0
            }
        }
    );
    let built = Endpoint {
        port: Port(1),
        label: Label {
            text: "z".into(),
            cached: 9,
        },
    };
    assert_eq!(built.at(&path("port")), Some(Datum::new("u16", "1")));
    assert_eq!(built.at(&path("label")), Some(Datum::new("String", "z")));
}

// ---- conditional fields ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Conditional {
    #[cfg(not(any()))]
    kept: u8,
    #[cfg(any())]
    gone: u8,
}

#[test]
fn a_field_behind_a_cfg_is_a_key_when_the_cfg_holds_and_none_otherwise() {
    assert_eq!(keys(&Conditional::description()), ["kept"]);
}

// ---- text and conversion modules ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Text {
    #[map(as_text)]
    addr: Ipv4Addr,
    #[map(as_text, default)]
    addrs: Vec<Ipv4Addr>,
    #[map(as_text)]
    maybe: Option<Ipv4Addr>,
    #[map(as_text)]
    maybe_list: Option<Vec<Ipv4Addr>>,
}

#[test]
fn as_text_converts_each_item_through_display_and_from_str() {
    let d = Text::description();
    assert_eq!(ty(&field(&d, "addr").description), "Ipv4Addr");
    assert!(matches!(
        field(&d, "addrs").description.kind(),
        Kind::List(_)
    ));
    assert!(field(&d, "maybe").optional && field(&d, "maybe_list").optional);
    assert_eq!(field(&d, "addrs").default_items, Some(vec![]));
    let mut p = Partial::<Text>::new();
    p.set("addr", "10.0.0.1").unwrap();
    p.set("addrs[0]", "10.0.0.2").unwrap();
    p.set("addrs[1]", "10.0.0.3").unwrap();
    p.set_items("maybe_list", ["1.1.1.1"]).unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.addr, Ipv4Addr::new(10, 0, 0, 1));
    assert_eq!(built.addrs.len(), 2);
    assert_eq!(built.maybe, None);
    assert_eq!(built.maybe_list, Some(vec![Ipv4Addr::new(1, 1, 1, 1)]));
    assert_eq!(built.entries(&path("addrs")), Some(Entries::Len(2)));
    assert_eq!(
        built.at(&path("addrs[1]")),
        Some(Datum::new("Ipv4Addr", "10.0.0.3"))
    );
    assert!(
        Partial::<Text>::new()
            .set("addr", "not an address")
            .is_err()
    );
}

mod percent {
    use coqui_builder::describe::LeafError;

    pub const TYPE: &str = "Percent";

    pub fn to_text(value: &u8) -> String {
        format!("{value}%")
    }

    pub fn from_text(text: &str) -> Result<u8, LeafError> {
        text.trim_end_matches('%')
            .parse()
            .map_err(|_| LeafError::Invalid { type_name: TYPE })
    }
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Converted {
    #[map(with = percent, default = 50)]
    amount: u8,
}

#[test]
fn with_names_a_module_of_text_converters() {
    let d = Converted::description();
    assert_eq!(field(&d, "amount").default.as_deref(), Some("50%"));
    assert_eq!(ty(&field(&d, "amount").description), "Percent");
    assert_eq!(
        Partial::<Converted>::new().build().unwrap(),
        Converted { amount: 50 }
    );
    let mut p = Partial::<Converted>::new();
    p.set("amount", "75%").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built, Converted { amount: 75 });
    assert_eq!(
        built.at(&path("amount")),
        Some(Datum::new("Percent", "75%"))
    );
    assert!(Partial::<Converted>::new().set("amount", "lots").is_err());
}

// ---- annotations ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Annotated {
    /// Ignored for the stated one.
    #[schema(
        title = "Name",
        description = "Stated.",
        section = "General",
        order = 3,
        advanced,
        read_only,
        deprecated,
        min_length = 1,
        max_length = 5,
        pattern = "^[a-z]+$",
        format = "hostname",
        examples("a".to_string(), "b".to_string())
    )]
    name: String,
    #[schema(sensitive, multiple_of = 5)]
    code: u32,
    #[schema(min_items = 1, max_items = 2)]
    list: Vec<u8>,
    #[builder(stricter = "lower")]
    timeout: u32,
    #[builder(stricter = "higher", setter = "attempt_count")]
    attempts: u32,
}

#[test]
fn schema_and_builder_attributes_become_notes() {
    let d = Annotated::description();
    let name = field(&d, "name").description.notes();
    assert_eq!(name.title.as_deref(), Some("Name"));
    assert_eq!(name.description.as_deref(), Some("Stated."));
    assert_eq!(name.section.as_deref(), Some("General"));
    assert_eq!(name.order, Some(3));
    assert!(name.advanced && name.read_only && name.deprecated);
    assert_eq!((name.min_length, name.max_length), (Some(1), Some(5)));
    assert_eq!(name.pattern.as_deref(), Some("^[a-z]+$"));
    assert_eq!(name.format.as_deref(), Some("hostname"));
    assert_eq!(name.examples, ["a", "b"]);
    assert!(field(&d, "code").description.notes().sensitive);
    assert_eq!(field(&d, "code").description.notes().multiple_of, Some(5.0));
    assert_eq!(
        field(&d, "timeout").description.notes().stricter,
        Some(Stricter::Lower)
    );
    assert_eq!(
        field(&d, "attempts").description.notes().stricter,
        Some(Stricter::Higher)
    );
}

#[test]
fn a_bound_the_description_states_is_checked_when_a_key_is_set() {
    let mut p = Partial::<Annotated>::new();
    assert!(p.set("name", "").is_err());
    assert!(p.set("name", "toolong").is_err());
    assert!(p.set("name", "fine").is_ok());
    assert!(p.set("code", "7").is_err());
    assert!(p.set("code", "10").is_ok());
    assert!(p.set_items("list", Vec::<&str>::new()).is_err());
    assert!(p.set_items("list", ["1", "2", "3"]).is_err());
    assert!(p.set_items("list", ["1"]).is_ok());
}

// ---- maps ----

#[derive(Describe, Debug, Clone, PartialEq, Default)]
struct Maps {
    #[map(default)]
    sorted: BTreeMap<String, u8>,
    #[map(default)]
    hashed: HashMap<String, Peer>,
}

#[test]
fn open_maps_of_leaves_and_of_groups_are_filled_key_by_key() {
    let mut p = Partial::<Maps>::new();
    p.set("sorted.a", "1").unwrap();
    p.set("hashed.x.host", "h").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.sorted["a"], 1);
    assert_eq!(built.hashed["x"].host, "h");
    assert_eq!(
        built.entries(&path("sorted")),
        Some(Entries::Keys(vec!["a".into()]))
    );
    let mut p = Partial::<Maps>::new();
    p.set("hashed.x.port", "1").unwrap();
    assert_eq!(paths(&p.build().unwrap_err()), ["hashed.x.host"]);
    assert!(Partial::<Maps>::new().set("sorted", "1").is_err());
}
