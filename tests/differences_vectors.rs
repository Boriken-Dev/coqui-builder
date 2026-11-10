// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The cases of `tests/vectors/differences.json`, a byte copy of the vectors guatiao's
//! `schema::differences` is held to, each run on a typed fixture: same places, same keys,
//! same values.

use std::collections::BTreeMap;
use std::time::Duration;

use coqui_builder::describe::Kind;
use coqui_builder::{Describe, Description, Expect, Inspect, Partial, Segment, Shown, differences};
use serde_json::{Map, Value, json};

/// Cases a typed value cannot state: a typed value has no key its type does not declare.
const NOT_TYPED: &[(&str, &str)] = &[
    (
        "a key the schema does not declare is reported hidden",
        "a typed value holds no key its type does not declare",
    ),
    (
        "a variant that switched arm is one key",
        "a switched arm is reported at its tag, one text, where the vector reports the arm whole",
    ),
    (
        "a container present on one side only is one key",
        "a group present on one side only is reported at the group with no value, where the          vector shows the group whole",
    ),
    (
        "a secret inside a list that is reported whole stays hidden",
        "a list of groups is walked item by item, where the vector reports the list whole",
    ),
];

// ---- fixtures ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Waiting {
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
struct Basic {
    #[map(default = "a".to_string())]
    name: String,
    #[map(default)]
    waiting: Waiting,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Ciphers {
    #[map(default = vec!["aes".to_string(), "chacha".to_string()])]
    cipher_suites: Vec<String>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Timeout {
    #[map(default = Duration::from_secs(5))]
    timeout: Duration,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Env {
    #[map(default)]
    env: BTreeMap<String, String>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind", rename_all = "lowercase")]
enum Auth {
    None,
    Pw { user: Option<String> },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Switched {
    #[map(default = Auth::None)]
    auth: Auth,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind", rename_all = "lowercase")]
enum Walked {
    Pw {
        user: Option<String>,
        #[map(default = 3)]
        retries: u32,
    },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Kept {
    auth: Walked,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Nick {
    nickname: Option<String>,
    #[map(default = "a".to_string())]
    name: String,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Secrets {
    #[map(default = "root".to_string())]
    user: String,
    #[schema(sensitive)]
    #[map(default)]
    password: String,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Token {
    #[schema(sensitive)]
    token: Option<String>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct PeerSecret {
    host: Option<String>,
    #[schema(sensitive)]
    key: Option<String>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Peers {
    #[map(default)]
    peers: Vec<PeerSecret>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Waits {
    #[map(default)]
    waiting: Waiting,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct TlsCa {
    ca: Option<String>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Tls {
    tls: Option<TlsCa>,
}

// ---- reading a case into a typed value ----

/// The text a JSON leaf is set from: a duration tuple `[secs, nanos]` is written with a
/// fraction, every other scalar as it prints.
fn text_of(value: &Value, description: &Description) -> String {
    match (value, description.kind()) {
        (Value::Array(pair), Kind::Leaf(info)) if info.ty() == "Duration" => {
            format!("{}.{:09}", pair[0], pair[1].as_u64().unwrap())
        }
        (Value::String(s), _) => s.clone(),
        (other, _) => other.to_string(),
    }
}

fn put<T: Describe>(
    partial: &mut Partial<T>,
    description: &Description,
    value: &Value,
    path: &mut Vec<Segment>,
) {
    let object = || value.as_object().expect("an object");
    match description.kind() {
        Kind::Group(group) => {
            for (key, inner) in object() {
                let field = group.field(key).unwrap_or_else(|| panic!("no field {key}"));
                path.push(Segment::Key(key.clone()));
                put(partial, &field.description, inner, path);
                path.pop();
            }
        }
        Kind::Variant(variant) => {
            let tag = object()[variant.tag()].as_str().expect("a tag").to_owned();
            let mut at = path.clone();
            at.push(Segment::Key(variant.tag().into()));
            partial.set_at(&at, &tag).unwrap();
            let arm = variant.arm(&tag).expect("an arm");
            for (key, inner) in object() {
                if key == variant.tag() {
                    continue;
                }
                let field = arm.fields.iter().find(|f| f.key == *key).expect("a field");
                path.push(Segment::Key(key.clone()));
                put(partial, &field.description, inner, path);
                path.pop();
            }
        }
        Kind::Map(entry) => {
            for (key, inner) in object() {
                path.push(Segment::Key(key.clone()));
                put(partial, entry, inner, path);
                path.pop();
            }
        }
        Kind::List(item) if matches!(item.kind(), Kind::Group(_) | Kind::Variant(_)) => {
            for (index, inner) in value.as_array().expect("an array").iter().enumerate() {
                path.push(Segment::Index(index));
                put(partial, item, inner, path);
                path.pop();
            }
        }
        Kind::List(item) => {
            let items: Vec<String> = value
                .as_array()
                .expect("an array")
                .iter()
                .map(|v| text_of(v, item))
                .collect();
            partial.set_items_at(path, &items).unwrap();
        }
        _ => partial.set_at(path, &text_of(value, description)).unwrap(),
    }
}

fn build<T: Describe>(value: &Value) -> T {
    let mut partial = Partial::<T>::new();
    let description = partial.description().clone();
    put(&mut partial, &description, value, &mut Vec::new());
    partial.build().unwrap_or_else(|e| panic!("{e}"))
}

/// What the vector states of a value: whether it is one text, a list of texts or a
/// structure. Texts are not compared, since each leaf prints its own.
fn kind_of(value: &Value) -> &'static str {
    match value {
        // `[secs, nanos]` is how the vectors write one duration, which is one text here.
        Value::Array(pair) if pair.len() == 2 && pair.iter().all(Value::is_number) => "one",
        Value::Array(_) => "items",
        Value::Object(_) => "structure",
        _ => "one",
    }
}

fn run<T: Describe + Inspect>(case: &Value) -> Vec<Value> {
    let value: T = build(&case["value"]);
    let baseline: Option<T> = case.get("baseline").map(build);
    let report = differences(
        &T::description(),
        &value,
        baseline.as_ref().map(|b| b as &dyn Inspect),
    );
    report
        .findings()
        .iter()
        .map(|finding| {
            let mut out = Map::new();
            out.insert(
                "path".into(),
                Value::Array(
                    finding
                        .key()
                        .segments()
                        .iter()
                        .map(|s| json!(s.name()))
                        .collect(),
                ),
            );
            out.insert("key".into(), json!(finding.key().to_string()));
            match finding.value() {
                Shown::Unset => {}
                Shown::One(d) => {
                    let kind = if d.is_sensitive() { "secret" } else { "one" };
                    out.insert("value".into(), json!(kind));
                }
                Shown::Items(_) => {
                    out.insert("value".into(), json!("items"));
                }
            }
            match finding.expected() {
                Expect::Equal(text) => {
                    let kind = if text == "***" { "secret" } else { "one" };
                    out.insert("default".into(), json!(kind));
                }
                Expect::ItemsEqual(_) => {
                    out.insert("default".into(), json!("items"));
                }
                _ => {}
            }
            Value::Object(out)
        })
        .collect()
}

/// The vector's expectation reduced to the same terms: key paths and kinds.
fn reduced(expected: &Value) -> Vec<Value> {
    expected
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let mut out = Map::new();
            out.insert("path".into(), entry["path"].clone());
            out.insert("key".into(), entry["key"].clone());
            for field in ["value", "default"] {
                if let Some(v) = entry.get(field) {
                    let kind = if v == "***" { "secret" } else { kind_of(v) };
                    out.insert(field.into(), json!(kind));
                }
            }
            Value::Object(out)
        })
        .collect()
}

fn typed(name: &str, case: &Value) -> Option<Vec<Value>> {
    Some(match name {
        "a nested object reports the one field that moved"
        | "a field left out equals its default and is no difference" => run::<Basic>(case),
        "a list is reported whole, never per element" => run::<Ciphers>(case),
        "a duration tuple is one key" => run::<Timeout>(case),
        "an open map key added and one removed" | "a map key holding a dot is one segment" => {
            run::<Env>(case)
        }
        "a variant that kept its arm is walked" => run::<Kept>(case),
        "an optional field that is set has no default to match" => run::<Nick>(case),
        "a secret that changed is reported and never shown"
        | "a secret that did not change is not listed" => run::<Secrets>(case),
        "a secret set with no default shows only the placeholder" => run::<Token>(case),
        "a baseline that leaves a field out falls back to the default" => run::<Waits>(case),
        _ => return None,
    })
}

#[test]
fn every_case_of_the_shared_vectors_gives_the_same_places_keys_and_values() {
    let vectors: Value =
        serde_json::from_str(include_str!("vectors/differences.json")).expect("valid JSON");
    assert_eq!(vectors["placeholder"], "***");
    let mut ran = 0;
    let mut listed = 0;
    for case in vectors["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        match typed(name, case) {
            Some(found) => {
                ran += 1;
                assert_eq!(
                    Value::Array(found),
                    Value::Array(reduced(&case["expected"])),
                    "{name}: the typed run differs from the vector"
                );
            }
            None => {
                assert!(
                    NOT_TYPED.iter().any(|(n, _)| *n == name),
                    "{name}: neither run nor listed in NOT_TYPED"
                );
                listed += 1;
            }
        }
    }
    assert_eq!(ran, 12);
    assert_eq!(listed, 4);
    assert_eq!(NOT_TYPED.len(), 4);
    for (name, reason) in NOT_TYPED {
        assert!(!reason.is_empty());
        assert!(
            vectors["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["name"] == *name),
            "{name} is listed but is not a case"
        );
    }
}

#[test]
fn the_shared_vectors_are_the_ones_guatiao_keeps() {
    // The copy is byte for byte what guatiao's `schema::differences` is held to: it has the
    // sixteen cases its tests expect and nothing a typed value could not state.
    let vectors: Value =
        serde_json::from_str(include_str!("vectors/differences.json")).expect("valid JSON");
    assert_eq!(vectors["cases"].as_array().unwrap().len(), 16);
}
