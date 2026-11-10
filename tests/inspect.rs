// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Reading a key of a value without knowing its type.

use std::collections::BTreeMap;

use coqui_builder::{Datum, Describe, Entries, Inspect, KeyPath, Partial};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Peer {
    host: String,
    port: Option<u16>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Doc {
    name: String,
    peers: Vec<Peer>,
    ports: Vec<u16>,
    env: BTreeMap<String, String>,
    tls: Option<Peer>,
}

fn at(value: &dyn Inspect, key: &str) -> Option<Datum> {
    value.at(key.parse::<KeyPath>().unwrap().segments())
}

fn entries(value: &dyn Inspect, key: &str) -> Option<Entries> {
    value.entries(key.parse::<KeyPath>().unwrap().segments())
}

fn doc() -> Doc {
    Doc {
        name: "n".into(),
        peers: vec![
            Peer {
                host: "h0".into(),
                port: None,
            },
            Peer {
                host: "h1".into(),
                port: Some(80),
            },
        ],
        ports: vec![1, 2, 3],
        env: BTreeMap::from([("A".to_string(), "1".to_string())]),
        tls: None,
    }
}

#[test]
fn a_leaf_answers_its_datum_and_a_path_below_a_leaf_answers_nothing() {
    let d = doc();
    assert_eq!(at(&d, "name"), Some(Datum::new("String", "n")));
    assert_eq!(at(&d, "name.x"), None);
    assert_eq!(at(&d, "nope"), None);
    assert_eq!(entries(&d, "name"), None);
}

#[test]
fn a_list_answers_its_length_and_each_item_by_position() {
    let d = doc();
    assert_eq!(entries(&d, "peers"), Some(Entries::Len(2)));
    assert_eq!(entries(&d, "ports"), Some(Entries::Len(3)));
    assert_eq!(at(&d, "ports[2]"), Some(Datum::new("u16", "3")));
    assert_eq!(at(&d, "peers[1].host"), Some(Datum::new("String", "h1")));
    assert_eq!(at(&d, "peers[1].port"), Some(Datum::new("u16", "80")));
    assert_eq!(at(&d, "peers[2].host"), None);
    assert_eq!(at(&d, "ports"), None, "a list is never one datum");
}

#[test]
fn an_option_that_is_none_is_absent() {
    let d = doc();
    assert_eq!(at(&d, "peers[0].port"), None);
    assert_eq!(at(&d, "tls"), None);
    assert_eq!(entries(&d, "tls"), None);
}

#[test]
fn an_open_map_answers_its_keys_and_each_value() {
    let d = doc();
    assert_eq!(at(&d, "env.A"), Some(Datum::new("String", "1")));
    assert_eq!(at(&d, "env[\"A\"]"), Some(Datum::new("String", "1")));
    assert_eq!(at(&d, "env.B"), None);
    assert_eq!(entries(&d, "env"), Some(Entries::Keys(vec!["A".into()])));
}

#[test]
fn a_group_answers_the_keys_that_are_set() {
    let d = doc();
    let Some(Entries::Keys(keys)) = d.entries(&[]) else {
        panic!("keys")
    };
    assert_eq!(keys, ["name", "peers", "ports", "env"]);
    assert_eq!(
        entries(&d, "peers[0]"),
        Some(Entries::Keys(vec!["host".into()]))
    );
}

#[test]
fn a_partial_answers_the_keys_set_so_far() {
    let mut p = Partial::<Doc>::new();
    p.set("peers[0].host", "h").unwrap();
    p.set_items("ports", ["2", "3"]).unwrap();
    assert_eq!(at(&p, "peers[0].host"), Some(Datum::new("String", "h")));
    assert_eq!(at(&p, "peers[0].port"), None);
    assert_eq!(at(&p, "name"), None);
    assert_eq!(entries(&p, "ports"), Some(Entries::Len(2)));
    p.set("name", "n").unwrap();
    p.set("env.A", "1").unwrap();
    assert_eq!(p.build().unwrap().ports, [2, 3]);
}
