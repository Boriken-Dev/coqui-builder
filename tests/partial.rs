// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Partial`: the order keys are set in, what text sets, what a refused key leaves alone.

use std::net::Ipv4Addr;
use std::time::Duration;

use coqui_builder::{Datum, Describe, Inspect, KeyPath, Missing, Partial};

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Mode {
    Fast { level: u8 },
    Slow { delay: Duration },
}

#[derive(Describe, Debug, Clone, PartialEq)]
enum Colour {
    Red,
    #[map(rename = "dark-blue")]
    DarkBlue,
    #[map(other)]
    Other(String),
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Port {
    Auto,
    Number(u16),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Conf {
    flag: bool,
    count: i32,
    ratio: f64,
    name: String,
    delay: Duration,
    addr: Ipv4Addr,
    colour: Colour,
    port: Port,
    mode: Mode,
}

fn key(text: &str) -> Vec<coqui_builder::Segment> {
    text.parse::<KeyPath>().unwrap().segments().to_vec()
}

fn complete() -> Partial<Conf> {
    let mut p = Partial::<Conf>::new();
    for (k, v) in [
        ("flag", "on"),
        ("count", "-4"),
        ("ratio", "0.5"),
        ("name", "n"),
        ("delay", "1.5"),
        ("addr", "10.1.2.3"),
        ("colour", "dark-blue"),
        ("port", "8080"),
        ("mode.kind", "Slow"),
        ("mode.delay", "2s"),
    ] {
        p.set(k, v).unwrap();
    }
    p
}

#[test]
fn text_sets_every_kind_of_leaf() {
    assert_eq!(
        complete().build().unwrap(),
        Conf {
            flag: true,
            count: -4,
            ratio: 0.5,
            name: "n".into(),
            delay: Duration::new(1, 500_000_000),
            addr: Ipv4Addr::new(10, 1, 2, 3),
            colour: Colour::DarkBlue,
            port: Port::Number(8080),
            mode: Mode::Slow {
                delay: Duration::from_secs(2)
            },
        }
    );
}

#[test]
fn a_choice_reads_its_names_and_other_keeps_a_name_it_does_not_know() {
    let mut p = complete();
    p.set("colour", "Red").unwrap();
    assert_eq!(p.build().unwrap().colour, Colour::Red);
    let mut p = complete();
    p.set("colour", "teal").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.colour, Colour::Other("teal".into()));
    assert_eq!(built.at(&key("colour")), Some(Datum::new("Colour", "teal")));
}

#[test]
fn a_choice_without_other_refuses_a_name_it_does_not_have() {
    #[derive(Describe, Debug, PartialEq)]
    enum Closed {
        A,
    }
    let mut p = Partial::<Closed>::new();
    assert!(p.set_at(&[], "B").is_err());
    p.set_at(&[], "A").unwrap();
    assert_eq!(p.build().unwrap(), Closed::A);
    let info = match <Closed as Describe>::description().kind() {
        coqui_builder::describe::Kind::Leaf(info) => *info,
        other => panic!("{other:?}"),
    };
    assert_eq!(info.choices(), Some(&["A"][..]));
}

#[test]
fn an_untagged_union_reads_its_names_then_its_held_leaves_in_order() {
    let mut p = complete();
    p.set("port", "Auto").unwrap();
    assert_eq!(p.build().unwrap().port, Port::Auto);
    let mut p = complete();
    p.set("port", "443").unwrap();
    assert_eq!(p.build().unwrap().port, Port::Number(443));
    let mut p = complete();
    assert!(p.set("port", "Nope").is_err());
    let built = complete().build().unwrap();
    assert_eq!(built.at(&key("port")), Some(Datum::new("Port", "8080")));
    let auto = Conf {
        port: Port::Auto,
        ..built
    };
    assert_eq!(auto.at(&key("port")), Some(Datum::new("Port", "Auto")));
}

#[test]
fn a_key_a_leaf_refuses_keeps_what_it_held() {
    let mut p = complete();
    p.set("count", "9").unwrap();
    assert!(p.set("count", "nine").is_err());
    assert!(p.set("count", "1.5").is_err());
    assert_eq!(p.build().unwrap().count, 9);
}

#[test]
fn the_last_write_of_a_key_wins_whatever_spelling_it_came_in() {
    let mut p = complete();
    p.set("delay", "3").unwrap();
    p.set("delay", "4000ms").unwrap();
    p.set("delay", "5s").unwrap();
    assert_eq!(p.build().unwrap().delay, Duration::from_secs(5));
}

#[test]
fn a_missing_key_is_named_by_its_path_and_the_error_lists_them_all() {
    let mut p = Partial::<Conf>::new();
    p.set("mode.kind", "Fast").unwrap();
    let missing: Missing = p.build().unwrap_err();
    let keys: Vec<String> = missing.keys().iter().map(ToString::to_string).collect();
    assert_eq!(
        keys,
        [
            "flag",
            "count",
            "ratio",
            "name",
            "delay",
            "addr",
            "colour",
            "port",
            "mode.level"
        ]
    );
    assert!(
        missing
            .to_string()
            .starts_with("missing settings: flag, count")
    );
}

#[test]
fn an_arm_overlaid_replaces_the_tag_and_keeps_the_fields_it_does_not_set() {
    let mut base = complete();
    let mut later = Partial::<Conf>::new();
    later.set("mode.kind", "Fast").unwrap();
    later.set("mode.level", "7").unwrap();
    base.overlay(later);
    assert_eq!(base.build().unwrap().mode, Mode::Fast { level: 7 });
}

#[test]
fn a_field_of_an_arm_set_before_the_tag_is_answered() {
    let mut p = Partial::<Mode>::new();
    p.set("level", "3").unwrap();
    assert_eq!(p.at(&key("level")), Some(Datum::new("u8", "3")));
    p.set("kind", "Fast").unwrap();
    assert_eq!(p.build().unwrap(), Mode::Fast { level: 3 });
}

#[test]
fn a_partial_is_a_default_and_prints_without_a_value() {
    let p = Partial::<Conf>::default();
    assert!(format!("{p:?}").starts_with("Partial"));
    assert!(p.description().notes().description.is_none());
    assert!(p.build().is_err());
}

#[test]
fn a_value_overlaid_on_a_partial_made_from_it_changes_nothing() {
    let built = complete().build().unwrap();
    let mut p = Partial::from(built.clone());
    p.overlay(Partial::from(built.clone()));
    assert_eq!(p.build().unwrap(), built);
}

#[test]
fn a_secret_is_not_printed_by_a_partial_that_holds_it() {
    #[derive(Describe, Debug)]
    struct S {
        #[schema(sensitive)]
        token: String,
    }
    let mut p = Partial::<S>::new();
    p.set("token", "hunter2").unwrap();
    assert!(!format!("{p:?}").contains("hunter2"));
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Pool {
    hosts: Vec<Member>,
    labels: std::collections::BTreeMap<String, Member>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Member {
    name: String,
}

#[test]
fn an_empty_list_of_groups_and_an_empty_map_count_as_set_when_overlaid() {
    let mut p = Partial::<Pool>::new();
    p.overlay(Partial::from(Pool {
        hosts: Vec::new(),
        labels: std::collections::BTreeMap::new(),
    }));
    let built = p.build().unwrap();
    assert!(built.hosts.is_empty() && built.labels.is_empty());
}

#[test]
fn an_empty_list_of_groups_replaces_an_earlier_one_and_an_empty_map_keeps_its_entries() {
    let mut base = Partial::<Pool>::new();
    base.set("hosts[0].name", "a").unwrap();
    base.set("labels.x.name", "b").unwrap();
    base.overlay(Partial::from(Pool {
        hosts: Vec::new(),
        labels: std::collections::BTreeMap::new(),
    }));
    let built = base.build().unwrap();
    assert!(built.hosts.is_empty());
    assert_eq!(built.labels["x"].name, "b");
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Pace {
    Fast {
        #[map(default = 1)]
        level: u8,
    },
    Slow {
        #[map(default = 9)]
        level: u8,
    },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Run {
    #[map(default = Pace::Fast { level: 1 })]
    pace: Pace,
}

#[test]
fn a_shared_key_reads_its_default_from_the_chosen_arm() {
    let key: Vec<coqui_builder::Segment> =
        "pace.level".parse::<KeyPath>().unwrap().segments().to_vec();
    let mut p = Partial::<Run>::new();
    assert_eq!(p.at(&key).unwrap().text(), "1");
    p.set("pace.kind", "Slow").unwrap();
    assert_eq!(p.at(&key).unwrap().text(), "9");
    assert_eq!(p.build().unwrap().pace, Pace::Slow { level: 9 });
}
