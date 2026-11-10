// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Deserialize` for `Partial`: TOML and JSON read into a partly filled settings type, which
//! a builder then takes.

use std::collections::BTreeMap;
use std::time::Duration;

use coqui_builder::{Builder, Describe, Partial};

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Credential {
    Password {
        user: String,
        #[schema(sensitive)]
        password: String,
    },
    Token {
        #[schema(sensitive)]
        token: String,
    },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Group {
    name: String,
    #[map(default)]
    size: u32,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Waiting {
    #[map(default = 10)]
    total: u32,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct CacheOptions {
    #[map(default = Duration::from_secs(60))]
    ttl: Duration,
    #[map(default = 1024)]
    size: u32,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Cache {
    Enabled(bool),
    Tuned(CacheOptions),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Settings {
    #[map(default = Some(Duration::from_secs(30)), none = "never")]
    read_timeout: Option<Duration>,
    credential: Credential,
    #[map(default)]
    groups: Vec<Group>,
    #[map(default)]
    labels: BTreeMap<String, String>,
    #[map(default = Cache::Enabled(true))]
    cache: Cache,
    #[map(default = Waiting { total: 10 })]
    waiting: Waiting,
    #[map(default)]
    ports: Vec<u16>,
    #[schema(sensitive)]
    #[map(default)]
    pin: u16,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(deny_unknown_fields)]
struct Strict {
    #[map(default)]
    name: String,
}

fn toml_of(text: &str) -> Settings {
    toml::from_str::<Partial<Settings>>(text)
        .unwrap()
        .build()
        .unwrap()
}

fn json_of(text: &str) -> Settings {
    serde_json::from_str::<Partial<Settings>>(text)
        .unwrap()
        .build()
        .unwrap()
}

const PASSWORD: &str = r#"
read_timeout = "250ms"
ports = [80, 443]

[credential]
kind = "Password"
user = "ana"
password = "hunter2"

[[groups]]
name = "a"
size = 3

[[groups]]
name = "b"

[labels]
env = "prod"
"a.b" = "dotted"

[cache]
ttl = "5s"

[waiting]
total = 4
"#;

fn expected() -> Settings {
    Settings {
        read_timeout: Some(Duration::from_millis(250)),
        credential: Credential::Password {
            user: "ana".into(),
            password: "hunter2".into(),
        },
        groups: vec![
            Group {
                name: "a".into(),
                size: 3,
            },
            Group {
                name: "b".into(),
                size: 0,
            },
        ],
        labels: BTreeMap::from([
            ("a.b".into(), "dotted".into()),
            ("env".into(), "prod".into()),
        ]),
        cache: Cache::Tuned(CacheOptions {
            ttl: Duration::from_secs(5),
            size: 1024,
        }),
        waiting: Waiting { total: 4 },
        ports: vec![80, 443],
        pin: 0,
    }
}

#[test]
fn toml_reads_a_tagged_enum_lists_a_map_and_a_table_arm() {
    assert_eq!(toml_of(PASSWORD), expected());
}

#[test]
fn a_tag_written_after_the_keys_of_its_arm_is_read() {
    let tag_last = r#"
[credential]
token = "abc"
kind = "Token"
"#;
    assert_eq!(
        toml_of(tag_last).credential,
        Credential::Token {
            token: "abc".into()
        }
    );
}

#[test]
fn json_reads_the_same_settings() {
    let json = r#"{
        "read_timeout": "250ms",
        "ports": [80, 443],
        "credential": {"user": "ana", "password": "hunter2", "kind": "Password"},
        "groups": [{"name": "a", "size": 3}, {"name": "b"}],
        "labels": {"env": "prod", "a.b": "dotted"},
        "cache": {"ttl": "5s"},
        "waiting": {"total": 4}
    }"#;
    assert_eq!(json_of(json), expected());
}

#[test]
fn a_duration_is_read_from_its_text_a_number_or_the_none_text() {
    let with = |value: &str| {
        format!("read_timeout = {value}\n[credential]\nkind = \"Token\"\ntoken = \"t\"\n")
    };
    assert_eq!(toml_of(&with("\"never\"")).read_timeout, None);
    assert_eq!(
        toml_of(&with("30")).read_timeout,
        Some(Duration::from_secs(30))
    );
    assert_eq!(
        toml_of(&with("0.25")).read_timeout,
        Some(Duration::from_millis(250))
    );
    assert_eq!(
        toml_of(&with("\"250ms\"")).read_timeout,
        Some(Duration::from_millis(250))
    );
    let absent = toml_of("[credential]\nkind = \"Token\"\ntoken = \"t\"\n");
    assert_eq!(absent.read_timeout, Some(Duration::from_secs(30)));
}

#[test]
fn a_json_null_is_a_key_that_is_not_given() {
    let s = json_of(
        r#"{"read_timeout": null, "credential": {"kind": "Token", "token": "t"}, "pin": 7}"#,
    );
    assert_eq!(s.read_timeout, Some(Duration::from_secs(30)));
    assert_eq!(s.pin, 7);
}

#[test]
fn an_untagged_enum_is_a_boolean_or_a_table() {
    let with = |cache: &str| format!("{cache}\n[credential]\nkind = \"Token\"\ntoken = \"t\"\n");
    assert_eq!(toml_of(&with("cache = true")).cache, Cache::Enabled(true));
    assert_eq!(toml_of(&with("cache = false")).cache, Cache::Enabled(false));
    assert_eq!(
        toml_of(&with("cache = { size = 8 }")).cache,
        Cache::Tuned(CacheOptions {
            ttl: Duration::from_secs(60),
            size: 8
        })
    );
    let both = json_of(r#"{"cache": false, "credential": {"kind": "Token", "token": "t"}}"#);
    assert_eq!(both.cache, Cache::Enabled(false));
}

#[test]
fn unknown_keys_are_skipped_and_refused_under_deny_unknown_fields() {
    let s = toml_of(
        "typo = 1\n[credential]\nkind = \"Token\"\ntoken = \"t\"\nextra = 2\n[waiting]\nbogus = 3\n",
    );
    assert_eq!(s.waiting.total, 10);

    assert!(
        toml::from_str::<Partial<Strict>>("name = \"a\"")
            .unwrap()
            .build()
            .is_ok()
    );
    let error = toml::from_str::<Partial<Strict>>("name = \"a\"\ntypo = 1")
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown settings: typo"), "{error}");
    let error = serde_json::from_str::<Partial<Strict>>(r#"{"nmae": "secret-value"}"#)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("nmae") && !error.contains("secret-value"),
        "{error}"
    );
}

#[test]
fn a_structure_where_a_leaf_is_expected_is_refused() {
    let error = toml::from_str::<Partial<Settings>>("[waiting]\ntotal = { a = 1 }")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("expected a single value at waiting.total"),
        "{error}"
    );
    let error = serde_json::from_str::<Partial<Settings>>(r#"{"waiting": {"total": [1, 2]}}"#)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("expected a single value at waiting.total"),
        "{error}"
    );
}

#[test]
fn a_root_that_is_not_a_table_is_refused() {
    let error = serde_json::from_str::<Partial<Settings>>("[1]")
        .unwrap_err()
        .to_string();
    assert!(error.contains("expected a table"), "{error}");
    let error = serde_json::from_str::<Partial<Settings>>("\"text\"")
        .unwrap_err()
        .to_string();
    assert!(error.contains("expected a table"), "{error}");
}

#[test]
fn an_error_at_a_sensitive_key_does_not_quote_the_value() {
    for text in ["hunter2-not-a-port", "99999999"] {
        let json =
            format!(r#"{{"pin": "{text}", "credential": {{"kind": "Token", "token": "t"}}}}"#);
        let error = serde_json::from_str::<Partial<Settings>>(&json)
            .unwrap_err()
            .to_string();
        assert!(error.contains("pin"), "{error}");
        assert!(!error.contains(text), "{error}");
    }
    let error = toml::from_str::<Partial<Settings>>("pin = { hunter2 = 1 }")
        .unwrap_err()
        .to_string();
    assert!(!error.contains("hunter2"), "{error}");
    let error = serde_json::from_str::<Partial<Settings>>(r#"{"credential": {"kind": "hunter2"}}"#)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("hunter2"), "{error}");
}

#[test]
fn a_later_file_lays_over_an_earlier_one_in_a_builder() {
    #[derive(Debug, PartialEq)]
    struct App {
        settings: Settings,
        name: String,
    }

    #[derive(Builder)]
    #[builder(build = App)]
    struct AppParts {
        settings: Settings,
        #[builder(input)]
        name: String,
    }

    impl From<AppParts> for App {
        fn from(parts: AppParts) -> App {
            App {
                settings: parts.settings,
                name: parts.name,
            }
        }
    }

    let first = toml::from_str::<Partial<Settings>>(PASSWORD).unwrap();
    let second =
        serde_json::from_str::<Partial<Settings>>(r#"{"waiting": {"total": 99}}"#).unwrap();
    let built = AppBuilder::from(first)
        .fill(second)
        .name("x")
        .build()
        .unwrap();
    let mut want = expected();
    want.waiting.total = 99;
    assert_eq!(built.settings, want);
    assert_eq!(built.name, "x");

    let missing = AppBuilder::from(toml::from_str::<Partial<Settings>>("pin = 1").unwrap())
        .name("x")
        .build()
        .unwrap_err();
    assert!(!missing.keys().is_empty());
}

#[test]
fn a_list_is_replaced_whole() {
    let mut partial = toml::from_str::<Partial<Settings>>("ports = [1, 2, 3]").unwrap();
    partial.overlay(toml::from_str::<Partial<Settings>>("ports = [9]").unwrap());
    let s = {
        partial
            .set("credential.kind", "Token")
            .and_then(|()| partial.set("credential.token", "t"))
            .unwrap();
        partial.build().unwrap()
    };
    assert_eq!(s.ports, vec![9]);
}

fn refused(text: &str) -> String {
    serde_json::from_str::<Partial<Settings>>(text)
        .unwrap_err()
        .to_string()
}

#[test]
fn a_value_of_the_wrong_shape_is_refused_where_it_sits() {
    let token = r#""credential": {"kind": "Token", "token": "t"}"#;
    let error = refused(&format!(r#"{{{token}, "waiting": 4}}"#));
    assert!(error.contains("expected a table at waiting"), "{error}");
    let error = refused(&format!(r#"{{{token}, "ports": 80}}"#));
    assert!(error.contains("expected a list at ports"), "{error}");
    let error = refused(&format!(r#"{{{token}, "cache": [1]}}"#));
    assert!(
        error.contains("expected a value or a table at cache"),
        "{error}"
    );
    let error = refused(&format!(r#"{{{token}, "labels": {{"a": {{"b": 1}}}}}}"#));
    assert!(
        error.contains("expected a single value at labels.a"),
        "{error}"
    );
    let error = refused(&format!(r#"{{{token}, "groups": [{{"name": "a"}}, 5]}}"#));
    assert!(error.contains("expected a table at groups[1]"), "{error}");
    let error = serde_json::from_str::<Partial<Cache>>("[1]")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("expected a value or a table at the root"),
        "{error}"
    );
    assert!(serde_json::from_str::<Partial<Waiting>>(r#"{"total": 5}"#).is_ok());
}

#[test]
fn a_repeated_key_keeps_the_last_value_and_an_empty_string_is_a_value() {
    let s = json_of(
        r#"{"credential": {"kind": "Password", "user": "", "password": "x"}, "pin": 1, "pin": 2}"#,
    );
    assert_eq!(s.pin, 2);
    assert_eq!(
        s.credential,
        Credential::Password {
            user: String::new(),
            password: "x".into()
        }
    );
}

#[test]
fn a_table_of_positions_stands_for_a_list_and_is_not_unknown() {
    let s = toml_of("ports = { 0 = 5, 1 = 6 }\n[credential]\nkind = \"Token\"\ntoken = \"t\"\n");
    assert_eq!(s.ports, vec![5, 6]);
}

#[test]
fn unknown_keys_are_refused_below_a_nested_strict_group() {
    #[derive(Describe, Debug, PartialEq)]
    struct Outer {
        #[map(default)]
        inner: Inner,
    }
    #[derive(Describe, Debug, PartialEq, Default)]
    #[map(deny_unknown_fields)]
    struct Inner {
        #[map(default)]
        a: u8,
    }
    let error = serde_json::from_str::<Partial<Outer>>(r#"{"inner": {"a": 1, "b": 2}}"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("inner.b"), "{error}");
}
