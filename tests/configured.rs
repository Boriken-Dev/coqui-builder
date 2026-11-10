// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Configured`: a builder and the value it built answer the same questions about their
//! settings, and a profile checks either.

use coqui_builder::{
    Builder, Check, Checks, Configured, Describe, Expect, Finding, KeyPath, Profile, Report,
    Segment, Shown,
};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Retry {
    #[map(default = 3)]
    attempts: u8,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct PoolSettings {
    #[map(default = 4)]
    size: u32,
    label: Option<String>,
    retry: Retry,
    #[schema(sensitive)]
    token: Option<String>,
}

#[derive(Debug, PartialEq, Configured)]
struct Pool {
    #[configured]
    settings: PoolSettings,
    tag: String,
}

#[derive(Builder)]
#[builder(build = Pool)]
struct PoolParts {
    settings: PoolSettings,
    #[builder(input)]
    tag: String,
}

impl From<PoolParts> for Pool {
    fn from(parts: PoolParts) -> Pool {
        Pool {
            settings: parts.settings,
            tag: parts.tag,
        }
    }
}

fn setters(builder: PoolBuilder) -> PoolBuilder {
    use PoolSettingsSetters;
    builder
        .size(8u32)
        .label("main".to_owned())
        .retry(Retry { attempts: 5 })
}

#[test]
fn the_non_defaults_of_a_builder_and_of_what_it_built_are_equal() {
    let builder = setters(Pool::builder()).tag("t");
    let seen = builder.non_defaults();
    assert_eq!(
        seen.iter().map(|n| n.setting.as_str()).collect::<Vec<_>>(),
        ["label", "retry.attempts", "size"]
    );
    assert_eq!(seen[2].value.to_string(), "u32(8)");
    assert_eq!(seen[2].default.to_string(), "u32(4)");
    let pool = builder.build().unwrap();
    assert_eq!(pool.non_defaults(), seen);
    assert_eq!(<Pool as Configured>::non_defaults(&pool), seen);
}

#[test]
fn a_fresh_builder_has_no_non_defaults_and_the_trait_method_agrees() {
    let builder = Pool::builder();
    assert!(builder.non_defaults().is_empty());
    assert_eq!(Configured::non_defaults(&builder), builder.non_defaults());
}

fn drift<C: Configured>(subject: &C) -> usize {
    subject.non_defaults().len()
}

#[test]
fn a_function_over_configured_takes_a_builder_and_a_built_value_alike() {
    let builder = setters(Pool::builder()).tag("t");
    assert_eq!(drift(&builder), 3);
    assert_eq!(drift(&builder.build().unwrap()), 3);
}

#[test]
fn the_configured_view_is_the_partial_with_defaults_for_unset_keys() {
    let builder = Pool::builder();
    let view = builder.configured();
    let at = |text: &str| {
        view.at(text.parse::<KeyPath>().unwrap().segments())
            .map(|d| d.to_string())
    };
    assert_eq!(at("size").as_deref(), Some("u32(4)"));
    assert_eq!(at("retry.attempts").as_deref(), Some("u8(3)"));
    assert_eq!(at("label"), None);
}

#[test]
fn a_profile_of_the_defaults_is_met_by_a_fresh_builder_and_missed_after_a_setter() {
    let profile = Profile::defaults::<PoolSettings>();
    let mut report = Report::new(&PoolSettings::description());
    profile.check(&Pool::builder(), &mut report);
    assert!(report.is_met(), "{:?}", report.unmet().collect::<Vec<_>>());

    let moved = setters(Pool::builder());
    let mut report = Report::new(&PoolSettings::description());
    profile.check(&moved, &mut report);
    let keys: Vec<String> = report.unmet().map(|f| f.key().to_string()).collect();
    assert_eq!(keys, ["size", "label", "retry.attempts"]);
}

#[test]
fn a_profile_checks_the_built_value_as_it_checks_the_builder() {
    let profile = Profile::against(&PoolSettings {
        size: 8,
        label: Some("main".into()),
        retry: Retry { attempts: 5 },
        token: None,
    });
    let builder = setters(Pool::builder()).tag("t");
    let mut report = Report::new(&PoolSettings::description());
    profile.check(&builder, &mut report);
    assert!(report.is_met());
    let pool = builder.build().unwrap();
    let mut report = Report::new(&PoolSettings::description());
    profile.check(&pool, &mut report);
    assert!(report.is_met());
}

#[test]
fn a_secret_in_a_builder_is_hidden_in_what_a_profile_reports() {
    use PoolSettingsSetters;
    let profile = Profile::new()
        .expecting("token", Expect::Equal("other".into()))
        .unwrap();
    let builder = Pool::builder().token("hunter2".to_owned());
    let mut report = Report::new(&PoolSettings::description());
    profile.check(&builder, &mut report);
    let finding = report.findings().first().unwrap();
    assert!(!finding.met());
    let Shown::One(datum) = finding.value() else {
        panic!("a value")
    };
    assert!(datum.is_sensitive());
    assert!(!datum.to_string().contains("hunter2"));
}

#[test]
fn a_closure_check_and_checks_run_on_a_builder() {
    let builder = setters(Pool::builder());
    let closure = |subject: &PoolBuilder, report: &mut Report| {
        let size = subject.configured().at(&[Segment::Key("size".into())]);
        report.push(Finding::new(
            "size".parse::<KeyPath>().unwrap(),
            size.as_ref().is_some_and(|d| d.text() == "8"),
            size.map_or(Shown::Unset, Shown::One),
            Expect::Equal("8".into()),
            None,
        ));
    };
    let mut report = Report::new(&PoolSettings::description());
    closure.check(&builder, &mut report);
    assert!(report.is_met());

    let mut checks: Checks<PoolBuilder> = Checks::new();
    checks.push(Profile::defaults::<PoolSettings>());
    checks.push(closure);
    let mut report = Report::new(&PoolSettings::description());
    checks.check(&builder, &mut report);
    assert_eq!(checks.len(), 2);
    assert!(!report.is_met(), "the profile of the defaults is missed");
    assert!(
        report.findings().iter().any(|f| f.met()),
        "the closure's finding is met"
    );
}
