// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Environment variables as a source: names, indices, open-map keys and unknown names.
//! Variables are passed in, never set.

use std::collections::BTreeMap;
use std::time::Duration;

use coqui_builder::{Describe, Env, FillError, KeyPath, Partial};

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
#[map(rename_all = "kebab-case")]
struct Conf {
    name: String,
    #[map(default)]
    waiting: Waiting,
    #[map(default)]
    peers: Vec<Peer>,
    #[map(default)]
    env: BTreeMap<String, String>,
    #[map(default)]
    view_only: bool,
    #[map(default = Duration::from_secs(5))]
    idle_timeout: Duration,
}

fn env(vars: &[(&str, &str)]) -> Env {
    Env::from_vars(
        "APP",
        vars.iter().map(|(k, v)| (k.to_string(), v.to_string())),
    )
}

#[test]
fn a_path_maps_to_an_upper_case_name_with_double_underscores_between_segments() {
    let e = env(&[]);
    let name = |text: &str| e.name(&text.parse::<KeyPath>().unwrap());
    assert_eq!(name("name"), "APP_NAME");
    assert_eq!(name("waiting.per_source"), "APP_WAITING__PER_SOURCE");
    assert_eq!(name("peers[0].host"), "APP_PEERS__0__HOST");
    assert_eq!(name("env.path"), "APP_ENV__PATH");
    assert_eq!(name("view-only"), "APP_VIEW_ONLY");
}

#[test]
fn variables_fill_leaves_lists_open_maps_and_hyphenated_keys() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[
        ("APP_NAME", "n"),
        ("APP_WAITING__PER_SOURCE", "3"),
        ("APP_PEERS__0__HOST", "h0"),
        ("APP_PEERS__1__HOST", "h1"),
        ("APP_PEERS__1__PORT", "8080"),
        ("APP_ENV__PATH", "/bin"),
        ("APP_ENV__HOME", "/root"),
        ("APP_VIEW_ONLY", "yes"),
        ("APP_IDLE_TIMEOUT", "0.5"),
        ("OTHER_NAME", "ignored"),
    ]);
    let unknown = p.fill_from(&mut source).unwrap();
    assert!(unknown.is_empty(), "{unknown:?}");
    let built = p.build().unwrap();
    assert_eq!(built.name, "n");
    assert_eq!(
        built.waiting,
        Waiting {
            per_source: 3,
            max: 4
        }
    );
    assert_eq!(built.peers.len(), 2);
    assert_eq!(built.peers[1].port, Some(8080));
    assert_eq!(built.env["PATH"], "/bin");
    assert_eq!(built.env["HOME"], "/root");
    assert!(built.view_only);
    assert_eq!(built.idle_timeout, Duration::from_millis(500));
}

#[test]
fn an_empty_variable_is_unset() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[("APP_NAME", "n"), ("APP_WAITING__MAX", "")]);
    p.fill_from(&mut source).unwrap();
    assert_eq!(p.build().unwrap().waiting.max, 4);
}

#[test]
fn a_name_the_description_lacks_is_unknown() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[
        ("APP_NAME", "n"),
        ("APP_TYPO", "x"),
        ("APP_WAITING__NOPE", "x"),
        ("APP_ENV__ANYTHING", "x"),
        ("APP_PEERS__0__HOST", "h"),
    ]);
    let unknown = p.fill_from(&mut source).unwrap();
    let keys: Vec<String> = unknown.keys().iter().map(ToString::to_string).collect();
    assert_eq!(keys, ["typo", "waiting.nope"]);
}

#[test]
fn a_value_the_leaf_refuses_is_an_error_that_does_not_quote_it() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[("APP_WAITING__MAX", "plenty")]);
    let error = p.fill_from(&mut source).unwrap_err();
    assert!(matches!(error, FillError { .. }));
    assert!(!error.to_string().contains("plenty"));
    assert!(error.to_string().starts_with("waiting.max"));
}

#[test]
fn only_variables_of_the_prefix_are_read() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[("APPLE_NAME", "no"), ("APP", "no"), ("APP_NAME", "yes")]);
    p.fill_from(&mut source).unwrap();
    assert_eq!(p.build().unwrap().name, "yes");
}

#[test]
fn the_process_environment_is_read_once_for_the_prefix() {
    // No variable is set here: a prefix nobody uses reads none, and fills nothing.
    let mut source = Env::new("COQUI_BUILDER_TEST_PREFIX_NOBODY_SETS_THIS");
    let mut p = Partial::<Conf>::new();
    assert!(p.fill_from(&mut source).unwrap().is_empty());
    assert!(p.build().is_err());
}

#[test]
fn an_open_map_key_in_mixed_case_is_found_as_written() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[
        ("APP_NAME", "n"),
        ("APP_ENV__Path", "/bin"),
        ("app_env__Home", "/h"),
    ]);
    let unknown = p.fill_from(&mut source).unwrap();
    assert!(unknown.is_empty(), "{unknown:?}");
    let built = p.build().unwrap();
    assert_eq!(built.env["Path"], "/bin");
    assert!(
        !built.env.contains_key("Home"),
        "the prefix is matched as written"
    );
}

#[test]
fn a_position_too_large_to_hold_is_ignored_and_never_allocates() {
    let mut p = Partial::<Conf>::new();
    let mut source = env(&[
        ("APP_NAME", "n"),
        ("APP_PEERS__0__HOST", "h"),
        ("APP_PEERS__4000000000__HOST", "far"),
        ("APP_PEERS__99999999999999999999999__HOST", "farther"),
    ]);
    p.fill_from(&mut source).unwrap();
    assert_eq!(p.build().unwrap().peers.len(), 1);
}

#[test]
fn debug_names_the_prefix_and_never_a_value() {
    let source = env(&[
        ("APP_NAME", "hunter2"),
        ("APP_ENV__team", "blue"),
        ("OTHER", "x"),
    ]);
    let shown = format!("{source:?}");
    assert!(shown.contains("APP_") && shown.contains('2'));
    assert!(!shown.contains("hunter2") && !shown.contains("blue"));
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Hosts {
    peers: Vec<Peer>,
    #[map(default)]
    env: BTreeMap<String, String>,
}

#[test]
fn a_source_answering_an_empty_list_of_groups_sets_it() {
    struct Empty;
    impl coqui_builder::Source for Empty {
        fn value(
            &mut self,
            _: &KeyPath,
            _: &coqui_builder::describe::LeafInfo,
        ) -> Result<Option<String>, coqui_builder::SourceError> {
            Ok(None)
        }
        fn entries(
            &mut self,
            at: &KeyPath,
        ) -> Result<Option<coqui_builder::Entries>, coqui_builder::SourceError> {
            Ok((at.to_string() == "peers").then_some(coqui_builder::Entries::Len(0)))
        }
        fn unknown(&mut self, _: &dyn Fn(&KeyPath) -> bool) -> Vec<KeyPath> {
            Vec::new()
        }
    }
    let mut p = Partial::<Hosts>::new();
    p.fill_from(&mut Empty).unwrap();
    assert!(p.build().unwrap().peers.is_empty());
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(deny_unknown_fields)]
struct Strict {
    #[map(rename = "maxConn")]
    max_conn: u32,
    #[map(default)]
    ports: BTreeMap<String, u32>,
    #[map(default)]
    hosts: Vec<String>,
}

#[test]
fn a_variable_read_by_a_renamed_key_is_not_unknown() {
    let mut p = Partial::<Strict>::new();
    let mut source = env(&[("APP_MAXCONN", "5")]);
    assert!(p.fill_from(&mut source).unwrap().is_empty());
    assert_eq!(p.build().unwrap().max_conn, 5);
    let mut p = Partial::<Strict>::new();
    let mut source = env(&[("APP_MAXCONN", "5"), ("APP_TYPO", "1")]);
    assert!(matches!(
        p.fill_from(&mut source).unwrap_err().kind(),
        coqui_builder::fill::FillErrorKind::UnknownKeys(_)
    ));
}

#[test]
fn an_open_map_whose_keys_are_digits_is_a_map() {
    let mut p = Partial::<Strict>::new();
    let mut source = env(&[("APP_MAXCONN", "1"), ("APP_PORTS__80", "5")]);
    p.fill_from(&mut source).unwrap();
    assert_eq!(p.build().unwrap().ports["80"], 5);
}

#[test]
fn a_list_with_a_missing_position_is_refused() {
    let mut p = Partial::<Strict>::new();
    let mut source = env(&[
        ("APP_MAXCONN", "1"),
        ("APP_HOSTS__0", "a"),
        ("APP_HOSTS__2", "c"),
    ]);
    assert!(p.fill_from(&mut source).is_err());
    let mut p = Partial::<Strict>::new();
    let mut source = env(&[("APP_MAXCONN", "1"), ("APP_HOSTS__1", "b")]);
    assert!(p.fill_from(&mut source).is_err());
    let mut p = Partial::<Strict>::new();
    let mut source = env(&[
        ("APP_MAXCONN", "1"),
        ("APP_HOSTS__0", "a"),
        ("APP_HOSTS__1", "b"),
    ]);
    p.fill_from(&mut source).unwrap();
    assert_eq!(p.build().unwrap().hosts, ["a", "b"]);
}
