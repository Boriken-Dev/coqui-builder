// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Profiles: each expectation, gating, expansion from a type or a value, merging, and checks
//! composed with closures and slices.

use std::collections::BTreeMap;
use std::time::Duration;

use coqui_builder::{
    Check, Checks, Datum, Describe, Expect, Finding, Inspect, KeyPath, Profile, Reason, Report,
    Shown,
};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Limits {
    #[builder(stricter = "lower")]
    #[map(default = 16)]
    per_source: u32,
    #[builder(stricter = "higher")]
    #[map(default = 3)]
    attempts: u32,
    #[map(default = 5)]
    plain: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            per_source: 16,
            attempts: 3,
            plain: 5,
        }
    }
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct TlsGroup {
    ca: String,
    #[map(default)]
    verify: bool,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Auth {
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
struct Policy {
    #[map(default)]
    limits: Limits,
    #[map(default = vec!["1.2".to_string(), "1.3".to_string()])]
    tls_versions: Vec<String>,
    #[map(default)]
    env: BTreeMap<String, String>,
    tls: Option<TlsGroup>,
    #[builder(stricter = "lower")]
    #[map(default = Some(Duration::from_secs(30)), none = "never")]
    timeout: Option<Duration>,
    #[map(default = Auth::KeyFile { path: "k".into() })]
    auth: Auth,
    #[map(default = "x".to_string())]
    name: String,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            limits: Limits::default(),
            tls_versions: vec!["1.2".into(), "1.3".into()],
            env: BTreeMap::new(),
            tls: None,
            timeout: Some(Duration::from_secs(30)),
            auth: Auth::KeyFile { path: "k".into() },
            name: "x".into(),
        }
    }
}

fn check(profile: &Profile, policy: &Policy) -> Report {
    profile.verify(policy)
}

fn only(report: &Report) -> &Finding {
    assert_eq!(report.len(), 1, "{report:?}");
    &report.findings()[0]
}

fn profile(entries: &[(&str, Expect)]) -> Profile {
    let mut out = Profile::new();
    for (key, expect) in entries {
        out = out.expecting(key, expect.clone()).unwrap();
    }
    out
}

#[test]
fn equal_is_met_by_the_same_value_and_not_by_another_or_by_nothing() {
    let p = Policy::default();
    let met = check(
        &profile(&[("limits.attempts", Expect::Equal("3".into()))]),
        &p,
    );
    assert!(only(&met).met());
    let differs = check(
        &profile(&[("limits.attempts", Expect::Equal("4".into()))]),
        &p,
    );
    assert_eq!(only(&differs).reason(), Some(&Reason::Differs));
    assert_eq!(only(&differs).value(), &Shown::One(Datum::new("u32", "3")));
    let missing = check(&profile(&[("tls.ca", Expect::Equal("x".into()))]), &p);
    assert_eq!(only(&missing).reason(), Some(&Reason::Missing));
    assert_eq!(only(&missing).value(), &Shown::Unset);
}

#[test]
fn within_holds_both_bounds_inclusive() {
    let p = Policy::default();
    let within = |min: Option<i32>, max: Option<i32>| Expect::Within {
        min: min.map(|n| n.to_string()),
        max: max.map(|n| n.to_string()),
    };
    assert!(
        only(&check(
            &profile(&[("limits.attempts", within(Some(3), Some(3)))]),
            &p
        ))
        .met()
    );
    let below = check(&profile(&[("limits.attempts", within(Some(4), None))]), &p);
    assert_eq!(only(&below).reason(), Some(&Reason::Below));
    let above = check(&profile(&[("limits.attempts", within(None, Some(2)))]), &p);
    assert_eq!(only(&above).reason(), Some(&Reason::Above));
    let text = check(&profile(&[("name", within(Some(1), None))]), &p);
    assert_eq!(only(&text).reason(), Some(&Reason::NoOrdering));
    let duration = Expect::Within {
        min: Some("10s".into()),
        max: Some("1m".into()),
    };
    assert!(only(&check(&profile(&[("timeout", duration)]), &p)).met());
}

#[test]
fn one_of_is_met_by_a_listed_value() {
    let p = Policy::default();
    let list = Expect::OneOf(vec!["x".into(), "y".into()]);
    assert!(only(&check(&profile(&[("name", list)]), &p)).met());
    let other = Expect::OneOf(vec!["a".into()]);
    let report = check(&profile(&[("name", other)]), &p);
    assert_eq!(only(&report).reason(), Some(&Reason::NotOneOf));
}

#[test]
fn as_strict_as_follows_the_order_the_setting_declares() {
    let p = Policy::default();
    // attempts: higher is stricter.
    let ok = check(
        &profile(&[("limits.attempts", Expect::AsStrictAs("2".into()))]),
        &p,
    );
    assert!(only(&ok).met());
    let looser = check(
        &profile(&[("limits.attempts", Expect::AsStrictAs("4".into()))]),
        &p,
    );
    assert_eq!(only(&looser).reason(), Some(&Reason::Looser));
    // per_source: lower is stricter.
    let ok = check(
        &profile(&[("limits.per_source", Expect::AsStrictAs("32".into()))]),
        &p,
    );
    assert!(only(&ok).met());
    let looser = check(
        &profile(&[("limits.per_source", Expect::AsStrictAs("8".into()))]),
        &p,
    );
    assert_eq!(only(&looser).reason(), Some(&Reason::Looser));
}

#[test]
fn no_stricter_than_follows_the_same_order() {
    let p = Policy::default();
    let ok = check(
        &profile(&[("limits.attempts", Expect::NoStricterThan("3".into()))]),
        &p,
    );
    assert!(only(&ok).met());
    let stricter = check(
        &profile(&[("limits.attempts", Expect::NoStricterThan("2".into()))]),
        &p,
    );
    assert_eq!(only(&stricter).reason(), Some(&Reason::Above));
    let stricter = check(
        &profile(&[("limits.per_source", Expect::NoStricterThan("32".into()))]),
        &p,
    );
    assert_eq!(only(&stricter).reason(), Some(&Reason::Below));
}

#[test]
fn a_setting_with_no_order_cannot_be_compared_for_strictness() {
    let p = Policy::default();
    let report = check(
        &profile(&[("limits.plain", Expect::AsStrictAs("1".into()))]),
        &p,
    );
    assert_eq!(only(&report).reason(), Some(&Reason::NoOrdering));
    let report = check(
        &profile(&[("limits.plain", Expect::NoStricterThan("1".into()))]),
        &p,
    );
    assert_eq!(only(&report).reason(), Some(&Reason::NoOrdering));
    let report = check(&profile(&[("name", Expect::AsStrictAs("a".into()))]), &p);
    assert_eq!(only(&report).reason(), Some(&Reason::NoOrdering));
}

#[test]
fn none_ranks_loosest_along_the_order() {
    let never = Policy {
        timeout: None,
        ..Policy::default()
    };
    let sixty = "60s";
    let looser = check(
        &profile(&[("timeout", Expect::AsStrictAs(sixty.into()))]),
        &never,
    );
    assert_eq!(only(&looser).reason(), Some(&Reason::Looser));
    let fine = check(
        &profile(&[("timeout", Expect::NoStricterThan(sixty.into()))]),
        &never,
    );
    assert!(only(&fine).met());
    let thirty = Policy::default();
    let ok = check(
        &profile(&[("timeout", Expect::AsStrictAs(sixty.into()))]),
        &thirty,
    );
    assert!(only(&ok).met());
}

#[test]
fn subset_of_asks_that_every_item_is_allowed() {
    let p = Policy::default();
    let wide = Expect::SubsetOf(vec!["1.1".into(), "1.2".into(), "1.3".into()]);
    assert!(only(&check(&profile(&[("tls_versions", wide)]), &p)).met());
    let narrow = Expect::SubsetOf(vec!["1.3".into()]);
    let report = check(&profile(&[("tls_versions", narrow)]), &p);
    assert_eq!(only(&report).reason(), Some(&Reason::NotSubset));
}

#[test]
fn required_and_absent_judge_a_leaf_and_an_optional_group() {
    let mut p = Policy::default();
    assert!(only(&check(&profile(&[("name", Expect::Required)]), &p)).met());
    assert_eq!(
        only(&check(&profile(&[("tls", Expect::Required)]), &p)).reason(),
        Some(&Reason::Missing)
    );
    assert!(only(&check(&profile(&[("tls", Expect::Absent)]), &p)).met());
    p.tls = Some(TlsGroup {
        ca: "ca".into(),
        verify: true,
    });
    assert!(only(&check(&profile(&[("tls", Expect::Required)]), &p)).met());
    assert_eq!(
        only(&check(&profile(&[("tls", Expect::Absent)]), &p)).reason(),
        Some(&Reason::Present)
    );
}

#[test]
fn a_key_the_type_does_not_have_is_a_finding() {
    let report = check(
        &profile(&[("nope.x", Expect::Required)]),
        &Policy::default(),
    );
    assert_eq!(only(&report).reason(), Some(&Reason::UnknownKey));
    assert!(!only(&report).met());
}

#[test]
fn any_expects_nothing() {
    let report = check(&profile(&[("name", Expect::Any)]), &Policy::default());
    assert!(report.is_empty());
}

#[test]
fn an_expectation_on_a_missing_optional_group_speaks_for_the_leaves_below_it() {
    let p = Policy::default();
    let both = profile(&[
        ("tls", Expect::Required),
        ("tls.ca", Expect::Equal("x".into())),
    ]);
    let report = check(&both, &p);
    assert_eq!(only(&report).key().to_string(), "tls");
    assert_eq!(only(&report).reason(), Some(&Reason::Missing));

    let present = Policy {
        tls: Some(TlsGroup {
            ca: "y".into(),
            verify: false,
        }),
        ..Policy::default()
    };
    let report = check(&both, &present);
    assert_eq!(report.len(), 2);
    assert!(report.findings()[0].met());
    assert_eq!(report.findings()[1].reason(), Some(&Reason::Differs));

    let absent = profile(&[
        ("tls", Expect::Absent),
        ("tls.ca", Expect::Equal("x".into())),
    ]);
    assert_eq!(check(&absent, &present).len(), 1);
}

#[test]
fn a_leaf_under_an_optional_group_with_no_expectation_of_its_own_is_judged_absent() {
    let report = check(
        &profile(&[("tls.ca", Expect::Equal("x".into()))]),
        &Policy::default(),
    );
    assert_eq!(only(&report).reason(), Some(&Reason::Missing));
}

#[test]
fn an_expectation_on_a_variant_gates_the_fields_of_the_other_arms() {
    let p = Policy {
        auth: Auth::Password {
            user: "u".into(),
            password: "pw".into(),
        },
        ..Policy::default()
    };
    let gated = profile(&[
        ("auth", Expect::Arms(vec!["KeyFile".into()])),
        ("auth.path", Expect::Equal("k".into())),
        ("auth.user", Expect::Equal("u".into())),
    ]);
    let report = check(&gated, &p);
    assert_eq!(report.len(), 2);
    assert_eq!(report.findings()[0].reason(), Some(&Reason::ArmNotAllowed));
    assert_eq!(
        report.findings()[0].value(),
        &Shown::One(Datum::new("String", "Password"))
    );
    assert_eq!(report.findings()[1].key().to_string(), "auth.user");
    assert!(report.findings()[1].met());
}

#[test]
fn the_arm_can_be_expected_by_name_at_the_tag() {
    let p = Policy::default();
    let by_name = profile(&[("auth.kind", Expect::Equal("KeyFile".into()))]);
    assert!(only(&check(&by_name, &p)).met());
    let wrong = profile(&[("auth.kind", Expect::Equal("Password".into()))]);
    assert_eq!(only(&check(&wrong, &p)).reason(), Some(&Reason::Differs));
}

#[test]
fn arms_on_something_that_is_not_a_variant_is_a_finding() {
    let report = check(
        &profile(&[("name", Expect::Arms(vec!["x".into()]))]),
        &Policy::default(),
    );
    assert!(matches!(only(&report).reason(), Some(Reason::Custom(_))));
}

#[test]
fn an_open_map_is_judged_one_entry_at_a_time() {
    let mut p = Policy::default();
    p.env.insert("A".into(), "1".into());
    p.env.insert("a.b".into(), "2".into());
    let report = check(
        &profile(&[
            ("env.A", Expect::Equal("1".into())),
            ("env.B", Expect::Required),
            ("env[\"a.b\"]", Expect::Equal("3".into())),
            ("env.C", Expect::Absent),
        ]),
        &p,
    );
    let by_key: BTreeMap<String, (bool, Option<Reason>)> = report
        .findings()
        .iter()
        .map(|f| (f.key().to_string(), (f.met(), f.reason().cloned())))
        .collect();
    assert_eq!(by_key["env.A"], (true, None));
    assert_eq!(by_key["env.B"], (false, Some(Reason::Missing)));
    assert_eq!(by_key["env.C"], (true, None));
    assert_eq!(by_key["env[\"a.b\"]"], (false, Some(Reason::Differs)));
}

#[test]
fn a_list_of_leaves_is_expected_whole_in_order() {
    let p = Policy::default();
    let same = Expect::ItemsEqual(vec!["1.2".into(), "1.3".into()]);
    assert!(only(&check(&profile(&[("tls_versions", same)]), &p)).met());
    let reordered = Expect::ItemsEqual(vec!["1.3".into(), "1.2".into()]);
    let report = check(&profile(&[("tls_versions", reordered)]), &p);
    assert_eq!(only(&report).reason(), Some(&Reason::Differs));
    assert_eq!(
        only(&report).value().to_string(),
        "[String(1.2), String(1.3)]"
    );
}

#[test]
fn an_expectation_a_leaf_cannot_hold_is_a_finding_and_is_refused_when_bound() {
    let p = Policy::default();
    let bad = profile(&[("limits.attempts", Expect::Equal("many".into()))]);
    let report = check(&bad, &p);
    assert!(matches!(only(&report).reason(), Some(Reason::Custom(_))));
    let error = bad.bind::<Policy>().unwrap_err();
    assert_eq!(error.key().to_string(), "limits.attempts");
    let good = profile(&[("limits.attempts", Expect::Equal("4".into()))]);
    assert!(good.bind::<Policy>().is_ok());
    let unordered = profile(&[(
        "name",
        Expect::Within {
            min: Some("a".into()),
            max: None,
        },
    )]);
    assert!(unordered.bind::<Policy>().is_err());
}

#[test]
fn a_duration_expectation_is_read_by_the_durations_own_parser() {
    let p = Policy::default();
    for spelling in ["30", "30s", "0.5m", "30000ms"] {
        let report = check(&profile(&[("timeout", Expect::Equal(spelling.into()))]), &p);
        assert!(only(&report).met(), "{spelling}");
    }
}

#[test]
fn the_defaults_of_a_type_are_a_profile_that_its_default_value_meets() {
    let defaults = Profile::defaults::<Policy>();
    let report = check(&defaults, &Policy::default());
    assert!(!report.is_empty());
    assert!(report.is_met(), "{:?}", report.unmet().collect::<Vec<_>>());
    let keys: Vec<String> = defaults
        .entries()
        .iter()
        .map(|(k, _)| k.to_string())
        .collect();
    assert!(keys.contains(&"limits.per_source".to_string()));
    assert!(keys.contains(&"auth.kind".to_string()));
    assert!(keys.contains(&"auth.path".to_string()));
    assert!(!keys.contains(&"auth.user".to_string()));
    assert!(keys.contains(&"tls".to_string()));
    assert!(!keys.contains(&"tls.ca".to_string()));
    assert_eq!(defaults.get(&"tls".parse().unwrap()), Some(&Expect::Absent));
    assert_eq!(
        defaults.get(&"tls_versions".parse().unwrap()),
        Some(&Expect::ItemsEqual(vec!["1.2".into(), "1.3".into()]))
    );
}

#[test]
fn a_changed_value_does_not_meet_the_profile_of_the_defaults() {
    let defaults = Profile::defaults::<Policy>();
    let mut changed = Policy::default();
    changed.limits.attempts = 9;
    changed.env.insert("A".into(), "1".into());
    changed.tls = Some(TlsGroup {
        ca: "c".into(),
        verify: false,
    });
    let report = check(&defaults, &changed);
    let unmet: Vec<String> = report.unmet().map(|f| f.key().to_string()).collect();
    assert!(unmet.contains(&"limits.attempts".to_string()), "{unmet:?}");
    assert!(unmet.contains(&"tls".to_string()), "{unmet:?}");
}

#[test]
fn a_profile_made_against_a_value_expects_that_value() {
    let base = Policy {
        tls: Some(TlsGroup {
            ca: "c".into(),
            verify: true,
        }),
        auth: Auth::Password {
            user: "u".into(),
            password: "pw".into(),
        },
        ..Policy::default()
    };
    let profile = Profile::against(&base);
    assert!(check(&profile, &base).is_met());
    assert_eq!(
        profile.get(&"tls".parse().unwrap()),
        Some(&Expect::Required)
    );
    assert_eq!(
        profile.get(&"tls.ca".parse().unwrap()),
        Some(&Expect::Equal("c".into()))
    );
    assert_eq!(
        profile.get(&"auth.user".parse().unwrap()),
        Some(&Expect::Equal("u".into()))
    );
    let mut other = base.clone();
    other.auth = Auth::KeyFile { path: "p".into() };
    let report = check(&profile, &other);
    assert!(!report.is_met());
}

#[test]
fn merging_takes_the_later_per_key_keeps_the_rest_and_any_clears() {
    let base = Profile::defaults::<Policy>();
    let later = profile(&[
        ("limits.per_source", Expect::NoStricterThan("32".into())),
        ("limits.attempts", Expect::Any),
        ("extra", Expect::Required),
    ]);
    let merged = base.merge(&later);
    let get = |key: &str| merged.get(&key.parse::<KeyPath>().unwrap()).cloned();
    assert_eq!(
        get("limits.per_source"),
        Some(Expect::NoStricterThan("32".into()))
    );
    assert_eq!(get("limits.attempts"), None);
    assert_eq!(get("limits.plain"), Some(Expect::Equal("5".into())));
    assert_eq!(get("extra"), Some(Expect::Required));
    assert_eq!(merged.len(), base.len());
    assert_eq!(base.merge(&Profile::new()), base);
}

#[test]
fn a_profile_replaces_and_removes_entries() {
    let mut p = Profile::new();
    let key: KeyPath = "name".parse().unwrap();
    assert_eq!(p.insert(key.clone(), Expect::Required), None);
    assert_eq!(
        p.insert(key.clone(), Expect::Absent),
        Some(Expect::Required)
    );
    assert_eq!(p.len(), 1);
    assert_eq!(p.remove(&key), Some(Expect::Absent));
    assert!(p.is_empty());
    assert!(Profile::new().expecting("a..b", Expect::Any).is_err());
}

#[test]
fn a_closure_a_slice_and_a_list_of_boxes_are_checks_too() {
    let p = Policy::default();
    let closure = |policy: &Policy, report: &mut Report| {
        let attempts: KeyPath = "limits.attempts".parse().unwrap();
        let value = policy.at(attempts.segments());
        report.push(Finding::new(
            "limits.attempts".parse().unwrap(),
            value.as_ref().is_some_and(|d| d.text() == "3"),
            value.map_or(Shown::Unset, Shown::One),
            Expect::Equal("3".into()),
            None,
        ));
    };
    assert!(closure.verify(&p).is_met());
    let profile = profile(&[("name", Expect::Required)]);
    let mut composed = Checks::<Policy>::new();
    composed.push(profile.clone());
    composed.push(closure);
    assert_eq!(composed.len(), 2);
    let report = composed.verify(&p);
    assert_eq!(report.len(), 2);
    let slice = [profile.clone(), profile];
    assert_eq!(slice.as_slice().verify(&p).len(), 2);
}

#[test]
fn a_report_of_a_profile_hides_a_secret_in_both_sides() {
    let p = Policy {
        auth: Auth::Password {
            user: "u".into(),
            password: "pw".into(),
        },
        ..Policy::default()
    };
    let profile = profile(&[("auth.password", Expect::Equal("expected-secret".into()))]);
    let report = check(&profile, &p);
    let finding = only(&report);
    assert_eq!(finding.value().to_string(), "String(***)");
    assert_eq!(finding.expected(), &Expect::Equal("***".into()));
    assert!(!format!("{report:?}").contains("expected-secret"));
    assert!(!format!("{report:?}").contains("\"pw\""));
}

#[test]
fn a_profile_made_from_a_type_or_a_value_never_copies_a_secret() {
    let p = Policy {
        auth: Auth::Password {
            user: "u".into(),
            password: "hunter2".into(),
        },
        ..Policy::default()
    };
    let against = Profile::against(&p);
    assert_eq!(
        against.get(&"auth.password".parse().unwrap()),
        Some(&Expect::Required)
    );
    assert!(!format!("{against:?}").contains("hunter2"));
    let defaults = Profile::defaults::<Policy>();
    assert!(!format!("{defaults:?}").contains("hunter2"));
    // The secret is judged present, not equal to a copy of itself.
    assert!(check(&against, &p).is_met());
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Vault {
    #[schema(sensitive)]
    #[map(default)]
    tokens: BTreeMap<String, String>,
    #[schema(sensitive)]
    inner: Inner,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Inner {
    key: String,
}

#[test]
fn everything_below_a_sensitive_map_or_group_is_a_secret_too() {
    let vault = Vault {
        tokens: BTreeMap::from([("ci".to_string(), "hunter2".to_string())]),
        inner: Inner {
            key: "hunter3".into(),
        },
    };
    let against = Profile::against(&vault);
    assert_eq!(
        against.get(&"tokens.ci".parse().unwrap()),
        Some(&Expect::Required)
    );
    assert_eq!(
        against.get(&"inner.key".parse().unwrap()),
        Some(&Expect::Required)
    );
    let shown = format!("{against:?}");
    assert!(!shown.contains("hunter2") && !shown.contains("hunter3"));
}

#[test]
fn arms_must_name_arms_of_the_variant_they_are_bound_to() {
    let bad = profile(&[("auth", Expect::Arms(vec!["Pasword".into()]))]);
    assert!(bad.bind::<Policy>().is_err());
    let not_a_variant = profile(&[("name", Expect::Arms(vec!["KeyFile".into()]))]);
    assert!(not_a_variant.bind::<Policy>().is_err());
    let good = profile(&[("auth", Expect::Arms(vec!["KeyFile".into()]))]);
    assert!(good.bind::<Policy>().is_ok());
}
