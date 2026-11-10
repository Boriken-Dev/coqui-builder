// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `non_defaults`: every setting that differs from its default, as `{setting, value,
//! default}` text; and `differences` against a second value.

use std::time::Duration;

use coqui_builder::{
    Datum, Describe, Expect, NonDefault, Reason, Shown, differences, non_defaults,
};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Limits {
    #[map(default = 16)]
    max_message: u32,
    #[map(default = true)]
    verify: bool,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Net {
    #[map(default = Limits { max_message: 16, verify: true })]
    limits: Limits,
    #[map(default = vec!["1.2".to_string(), "1.3".to_string()])]
    tls_versions: Vec<String>,
    #[map(default = Duration::from_secs(60))]
    idle: Duration,
    #[map(default = "wheel".to_string())]
    group: String,
    owner: Option<String>,
    #[schema(sensitive)]
    #[map(default)]
    secret: String,
}

fn defaults() -> Net {
    Net {
        limits: Limits {
            max_message: 16,
            verify: true,
        },
        tls_versions: vec!["1.2".into(), "1.3".into()],
        idle: Duration::from_secs(60),
        group: "wheel".into(),
        owner: None,
        secret: String::new(),
    }
}

fn one(net: &Net) -> NonDefault {
    let mut found = non_defaults(net);
    assert_eq!(found.len(), 1, "{found:?}");
    found.remove(0)
}

fn d(ty: &'static str, text: &str) -> Shown {
    Shown::One(Datum::new(ty, text))
}

fn items(ty: &'static str, texts: &[&str]) -> Shown {
    Shown::Items(texts.iter().map(|t| Datum::new(ty, *t)).collect())
}

#[test]
fn the_defaults_report_nothing() {
    assert!(non_defaults(&defaults()).is_empty());
}

#[test]
fn each_setting_that_differs_is_reported_as_a_typed_text() {
    let mut net = defaults();
    net.limits.max_message = 64;
    assert_eq!(
        one(&net),
        NonDefault::new("limits.max_message", d("u32", "64"), d("u32", "16"))
    );

    let mut net = defaults();
    net.limits.verify = false;
    assert_eq!(
        one(&net),
        NonDefault::new("limits.verify", d("bool", "false"), d("bool", "true"))
    );

    let mut net = defaults();
    net.tls_versions = vec!["1.3".into()];
    assert_eq!(
        one(&net),
        NonDefault::new(
            "tls_versions",
            items("String", &["1.3"]),
            items("String", &["1.2", "1.3"])
        )
    );

    let mut net = defaults();
    net.idle = Duration::from_secs(5);
    assert_eq!(
        one(&net),
        NonDefault::new("idle", d("Duration", "5s"), d("Duration", "60s"))
    );

    let mut net = defaults();
    net.group = "staff".into();
    assert_eq!(
        one(&net),
        NonDefault::new("group", d("String", "staff"), d("String", "wheel"))
    );

    let mut net = defaults();
    net.owner = Some("ana".into());
    assert_eq!(
        one(&net),
        NonDefault::new("owner", d("String", "ana"), Shown::Unset)
    );
}

#[test]
fn a_duration_equal_to_its_default_in_another_unit_is_no_difference() {
    let mut net = defaults();
    net.idle = Duration::from_millis(60_000);
    assert!(non_defaults(&net).is_empty());
}

#[test]
fn a_secret_that_changed_is_listed_and_never_shown() {
    let mut net = defaults();
    net.secret = "hunter2".into();
    let found = one(&net);
    assert_eq!(found.value.to_string(), "String(***)");
    assert_eq!(found.default.to_string(), "String(***)");
    assert!(!format!("{found:?}").contains("hunter2"));
}

#[test]
fn several_come_in_the_order_of_their_paths() {
    let mut net = defaults();
    net.owner = Some("a".into());
    net.group = "g".into();
    net.limits.max_message = 1;
    net.idle = Duration::ZERO;
    let keys: Vec<String> = non_defaults(&net).into_iter().map(|n| n.setting).collect();
    assert_eq!(keys, ["group", "idle", "limits.max_message", "owner"]);
}

#[test]
fn differences_compare_two_values_and_not_only_the_defaults() {
    let mut other = defaults();
    other.limits.max_message = 64;
    other.owner = Some("ana".into());
    let mut now = other.clone();
    now.limits.max_message = 128;
    now.owner = None;
    let report = differences(&Net::description(), &now, Some(&other));
    let found: Vec<(String, Expect)> = report
        .findings()
        .iter()
        .map(|f| (f.key().to_string(), f.expected().clone()))
        .collect();
    assert_eq!(
        found,
        [
            ("limits.max_message".to_string(), Expect::Equal("64".into())),
            ("owner".to_string(), Expect::Equal("ana".into())),
        ]
    );
    assert!(
        report
            .findings()
            .iter()
            .all(|f| !f.met() && f.reason() == Some(&Reason::Differs))
    );
    assert_eq!(report.findings()[1].value(), &Shown::Unset);
}

#[test]
fn a_value_equal_to_its_baseline_has_no_differences() {
    let mut other = defaults();
    other.limits.max_message = 64;
    let report = differences(&Net::description(), &other.clone(), Some(&other));
    assert!(report.is_empty());
}

#[test]
fn a_non_default_prints_on_one_line() {
    assert_eq!(
        NonDefault::new("idle", d("Duration", "5s"), d("Duration", "60s")).to_string(),
        "idle = Duration(5s) (default Duration(60s))"
    );
}
