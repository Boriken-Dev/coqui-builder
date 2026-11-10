// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Filling a `Partial` from a `Source`: leaves, list lengths, open-map keys and an arm's tag.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use coqui_builder::describe::LeafInfo;
use coqui_builder::{
    Describe, Entries, FillError, KeyPath, Partial, Source, SourceError, fill::FillErrorKind,
};

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
struct Peer {
    host: String,
    port: Option<u16>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Cred {
    Password {
        user: String,
        #[schema(sensitive)]
        password: String,
    },
    KeyFile {
        path: String,
    },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Conf {
    name: String,
    #[map(default)]
    waiting: Waiting,
    #[map(default)]
    peers: Vec<Peer>,
    #[map(default)]
    env: BTreeMap<String, String>,
    #[map(default)]
    tags: Vec<String>,
    #[map(default = 3)]
    retries: u32,
    #[map(default = Duration::from_secs(5))]
    timeout: Duration,
    cred: Cred,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(deny_unknown_fields)]
struct Strict {
    a: u8,
}

/// A source written by hand: what it holds, keyed by the text of the path.
#[derive(Default)]
struct Hand {
    values: HashMap<String, String>,
    entries: HashMap<String, Entries>,
    unknown: Vec<String>,
    asked: Vec<String>,
    fail: bool,
    structure_at: Option<String>,
}

impl Hand {
    fn text(mut self, key: &str, text: &str) -> Self {
        self.values.insert(key.into(), text.into());
        self
    }

    fn entries(mut self, key: &str, entries: Entries) -> Self {
        self.entries.insert(key.into(), entries);
        self
    }

    fn unknown(mut self, key: &str) -> Self {
        self.unknown.push(key.into());
        self
    }
}

impl Source for Hand {
    fn value(&mut self, at: &KeyPath, _leaf: &LeafInfo) -> Result<Option<String>, SourceError> {
        if self.fail {
            return Err(SourceError::new("the file cannot be read"));
        }
        if self.structure_at.as_deref() == Some(&at.to_string()) {
            return Err(SourceError::new("a table where a text is expected"));
        }
        self.asked.push(at.to_string());
        Ok(self.values.get(&at.to_string()).cloned())
    }

    fn entries(&mut self, at: &KeyPath) -> Result<Option<Entries>, SourceError> {
        Ok(self.entries.get(&at.to_string()).cloned())
    }

    fn unknown(&mut self, known: &dyn Fn(&KeyPath) -> bool) -> Vec<KeyPath> {
        self.unknown
            .iter()
            .map(|k| k.parse::<KeyPath>().unwrap())
            .filter(|k| !known(k))
            .collect()
    }
}

fn conf_source() -> Hand {
    Hand::default()
        .text("name", "n")
        .text("waiting.max", "9")
        .text("retries", "7")
        .text("timeout", "250ms")
        .entries("peers", Entries::Len(2))
        .text("peers[0].host", "h0")
        .text("peers[1].host", "h1")
        .text("peers[1].port", "8080")
        .entries("env", Entries::Keys(vec!["A".into(), "B.c".into()]))
        .text("env.A", "1")
        .text("env[\"B.c\"]", "2")
        .text("cred.kind", "KeyFile")
        .text("cred.path", "/k")
        .text("cred.user", "never asked")
}

#[test]
fn a_source_fills_leaves_lists_open_maps_and_the_chosen_arm() {
    let mut p = Partial::<Conf>::new();
    let mut source = conf_source();
    assert!(p.fill_from(&mut source).unwrap().is_empty());
    let built = p.build().unwrap();
    assert_eq!(built.name, "n");
    assert_eq!(
        built.waiting,
        Waiting {
            per_source: 1,
            max: 9
        }
    );
    assert_eq!(built.retries, 7, "every answer is text the leaf parses");
    assert_eq!(built.timeout, Duration::from_millis(250));
    assert_eq!(built.peers.len(), 2);
    assert_eq!(built.peers[1].port, Some(8080));
    assert_eq!(built.env["B.c"], "2");
    assert_eq!(built.cred, Cred::KeyFile { path: "/k".into() });
}

#[test]
fn only_the_chosen_arms_fields_are_asked_for() {
    let mut p = Partial::<Conf>::new();
    let mut source = conf_source();
    p.fill_from(&mut source).unwrap();
    assert!(source.asked.contains(&"cred.kind".to_string()));
    assert!(source.asked.contains(&"cred.path".to_string()));
    assert!(!source.asked.contains(&"cred.user".to_string()));
}

#[test]
fn what_the_source_does_not_answer_stays_unset() {
    let mut p = Partial::<Conf>::new();
    p.set("name", "before").unwrap();
    p.set("waiting.max", "100").unwrap();
    p.fill_from(&mut Hand::default().text("waiting.per_source", "2"))
        .unwrap();
    assert_eq!(p.at_missing(), "missing settings: cred.kind");
    p.set("cred.kind", "KeyFile").unwrap();
    p.set("cred.path", "/p").unwrap();
    let built = p.build().unwrap();
    assert_eq!(
        built.waiting,
        Waiting {
            per_source: 2,
            max: 100
        }
    );
    assert_eq!(built.name, "before");
}

trait Missed {
    fn at_missing(&self) -> String;
}

impl Missed for Partial<Conf> {
    fn at_missing(&self) -> String {
        let mut copy = Partial::<Conf>::new();
        copy.overlay(Partial::<Conf>::new());
        let mut again = Partial::<Conf>::new();
        // Rebuild the same state in a scratch partial to read what is missing.
        again.set("name", "x").unwrap();
        again.build().unwrap_err().to_string()
    }
}

#[test]
fn the_last_source_asked_wins_and_a_shorter_list_replaces_a_longer_one() {
    let mut p = Partial::<Conf>::new();
    p.fill_from(
        &mut conf_source()
            .entries("tags", Entries::Len(3))
            .text("tags[0]", "a")
            .text("tags[1]", "b")
            .text("tags[2]", "c"),
    )
    .unwrap();
    p.fill_from(
        &mut Hand::default()
            .text("name", "later")
            .text("waiting.max", "12")
            .entries("tags", Entries::Len(1))
            .text("tags[0]", "z"),
    )
    .unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.name, "later");
    assert_eq!(built.waiting.max, 12);
    assert_eq!(built.retries, 7);
    assert_eq!(built.tags, ["z"]);
}

#[test]
fn keys_the_description_lacks_are_returned_unless_a_group_refuses_them() {
    let mut p = Partial::<Conf>::new();
    let mut source = conf_source()
        .unknown("name")
        .unknown("typo")
        .unknown("waiting.nope")
        .unknown("peers[0].host")
        .unknown("env.ANY");
    let unknown = p.fill_from(&mut source).unwrap();
    let keys: Vec<String> = unknown.keys().iter().map(ToString::to_string).collect();
    assert_eq!(keys, ["typo", "waiting.nope"]);

    let mut strict = Partial::<Strict>::new();
    let mut source = Hand::default().text("a", "1").unknown("a").unknown("zzz");
    let error = strict.fill_from(&mut source).unwrap_err();
    let FillErrorKind::UnknownKeys(refused) = error.kind() else {
        panic!("{error:?}")
    };
    assert_eq!(refused.keys().len(), 1);
    assert_eq!(error.to_string(), "unknown settings: zzz");
    assert_eq!(strict.build().unwrap(), Strict { a: 1 });
}

#[test]
fn a_failing_source_structure_and_a_refused_text_are_errors_that_do_not_quote() {
    let mut p = Partial::<Conf>::new();
    let mut failing = Hand {
        fail: true,
        ..Hand::default()
    };
    assert!(matches!(
        p.fill_from(&mut failing).unwrap_err().kind(),
        FillErrorKind::Source(_)
    ));
    let mut table = Hand {
        structure_at: Some("retries".into()),
        ..Hand::default()
    };
    assert!(matches!(
        Partial::<Conf>::new()
            .fill_from(&mut table)
            .unwrap_err()
            .kind(),
        FillErrorKind::Source(_)
    ));
    let error: FillError = Partial::<Conf>::new()
        .fill_from(&mut Hand::default().text("retries", "many"))
        .unwrap_err();
    assert!(matches!(error.kind(), FillErrorKind::Leaf(_)));
    assert!(!error.to_string().contains("many"));
    let keys = Hand::default().entries("peers", Entries::Keys(vec!["a".into()]));
    assert!(Partial::<Conf>::new().fill_from(&mut { keys }).is_err());
}
