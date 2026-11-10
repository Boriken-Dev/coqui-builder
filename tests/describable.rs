// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Expect` and `Profile` are settings types, written by hand: each is held to a derived twin
//! of the same shape, so a profile can be read from a file or a form like any other type.

use std::collections::BTreeMap;

use coqui_builder::{
    Check, Datum, Describe, Expect, Inspect, KeyPath, Partial, Profile, Reason, Segment,
};

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "expect", rename_all = "snake_case")]
enum ExpectTwin {
    Equal {
        value: String,
    },
    Within {
        min: Option<String>,
        max: Option<String>,
    },
    OneOf {
        values: Vec<String>,
    },
    AsStrictAs {
        value: String,
    },
    NoStricterThan {
        value: String,
    },
    SubsetOf {
        values: Vec<String>,
    },
    ItemsEqual {
        values: Vec<String>,
    },
    Required,
    Absent,
    Arms {
        names: Vec<String>,
    },
    Any,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(transparent)]
struct ProfileTwin {
    entries: BTreeMap<String, ExpectTwin>,
}

fn every() -> Vec<(Expect, ExpectTwin)> {
    let five = || String::from("5");
    vec![
        (Expect::Equal(five()), ExpectTwin::Equal { value: five() }),
        (
            Expect::Within {
                min: Some("1".into()),
                max: None,
            },
            ExpectTwin::Within {
                min: Some("1".into()),
                max: None,
            },
        ),
        (
            Expect::OneOf(vec!["a".into(), "b".into()]),
            ExpectTwin::OneOf {
                values: vec!["a".into(), "b".into()],
            },
        ),
        (
            Expect::AsStrictAs(five()),
            ExpectTwin::AsStrictAs { value: five() },
        ),
        (
            Expect::NoStricterThan(five()),
            ExpectTwin::NoStricterThan { value: five() },
        ),
        (
            Expect::SubsetOf(vec!["x".into()]),
            ExpectTwin::SubsetOf {
                values: vec!["x".into()],
            },
        ),
        (
            Expect::ItemsEqual(vec!["x".into(), "y".into()]),
            ExpectTwin::ItemsEqual {
                values: vec!["x".into(), "y".into()],
            },
        ),
        (Expect::Required, ExpectTwin::Required),
        (Expect::Absent, ExpectTwin::Absent),
        (
            Expect::Arms(vec!["Pw".into()]),
            ExpectTwin::Arms {
                names: vec!["Pw".into()],
            },
        ),
        (Expect::Any, ExpectTwin::Any),
    ]
}

fn key(text: &str) -> Vec<Segment> {
    text.parse::<KeyPath>().unwrap().segments().to_vec()
}

#[test]
fn an_expectation_is_described_as_its_derived_twin_is() {
    assert_eq!(Expect::description(), ExpectTwin::description());
}

#[test]
fn a_profile_is_described_as_its_derived_twin_is() {
    assert_eq!(Profile::description(), ProfileTwin::description());
}

#[test]
fn an_expectation_answers_what_its_twin_answers() {
    for (expect, twin) in every() {
        assert_eq!(expect.at(&[]), twin.at(&[]), "{expect:?}");
        assert_eq!(expect.entries(&[]), twin.entries(&[]), "{expect:?}");
        for path in [
            "expect",
            "value",
            "min",
            "max",
            "values",
            "values[0]",
            "names",
            "nope",
        ] {
            assert_eq!(
                expect.at(&key(path)),
                twin.at(&key(path)),
                "{expect:?} {path}"
            );
            assert_eq!(
                expect.entries(&key(path)),
                twin.entries(&key(path)),
                "{expect:?} {path}"
            );
        }
    }
}

#[test]
fn an_expectation_made_from_a_value_builds_back_the_value() {
    for (expect, _) in every() {
        assert_eq!(Partial::from(expect.clone()).build().unwrap(), expect);
    }
}

#[test]
fn an_expectation_is_filled_a_key_at_a_time() {
    let mut p = Partial::<Expect>::new();
    p.set("expect", "within").unwrap();
    p.set("max", "9").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Expect::Within {
            min: None,
            max: Some("9".into())
        }
    );
    let mut p = Partial::<Expect>::new();
    p.set("expect", "arms").unwrap();
    p.set("names[0]", "Pw").unwrap();
    p.set("names[1]", "Token").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Expect::Arms(vec!["Pw".into(), "Token".into()])
    );
    let mut p = Partial::<Expect>::new();
    p.set("expect", "items_equal").unwrap();
    p.set_items("values", ["a", "b"]).unwrap();
    assert_eq!(
        p.build().unwrap(),
        Expect::ItemsEqual(vec!["a".into(), "b".into()])
    );
    let mut p = Partial::<Expect>::new();
    p.set("expect", "equal").unwrap();
    assert_eq!(
        p.build().unwrap_err().to_string(),
        "missing settings: value"
    );
    assert!(Partial::<Expect>::new().set("expect", "nothing").is_err());
    assert!(Partial::<Expect>::new().set("nope", "1").is_err());
    assert_eq!(
        Partial::<Expect>::new().build().unwrap_err().to_string(),
        "missing settings: expect"
    );
}

#[test]
fn a_profile_is_filled_from_keys_that_are_paths() {
    let mut p = Partial::<Profile>::new();
    p.set("[\"limits.attempts\"].expect", "equal").unwrap();
    p.set("[\"limits.attempts\"].value", "3").unwrap();
    p.set("[\"tls\"].expect", "absent").unwrap();
    let profile = p.build().unwrap();
    assert_eq!(profile.len(), 2);
    assert_eq!(
        profile.get(&"limits.attempts".parse().unwrap()),
        Some(&Expect::Equal("3".into()))
    );
    assert_eq!(profile.get(&"tls".parse().unwrap()), Some(&Expect::Absent));
}

#[test]
fn an_untouched_profile_is_empty_and_a_key_that_is_no_path_is_missing() {
    assert!(Partial::<Profile>::new().build().unwrap().is_empty());
    let mut p = Partial::<Profile>::new();
    p.set("[\"a..b\"].expect", "required").unwrap();
    assert!(p.build().is_err());
}

#[test]
fn a_profile_round_trips_through_a_partial_and_answers_like_its_twin() {
    let profile = Profile::new()
        .expecting("a.b", Expect::Equal("1".into()))
        .unwrap()
        .expecting("c", Expect::Required)
        .unwrap();
    assert_eq!(Partial::from(profile.clone()).build().unwrap(), profile);
    let twin = ProfileTwin {
        entries: BTreeMap::from([
            ("a.b".to_string(), ExpectTwin::Equal { value: "1".into() }),
            ("c".to_string(), ExpectTwin::Required),
        ]),
    };
    assert_eq!(profile.at(&[]), twin.at(&[]));
    assert_eq!(
        profile.at(&key("[\"a.b\"].value")),
        Some(Datum::new("String", "1"))
    );
    assert_eq!(Inspect::entries(&profile, &[]), twin.entries(&[]));
}

#[test]
fn a_profile_read_back_from_its_own_answers_still_checks_the_same() {
    #[derive(Describe, Debug, PartialEq)]
    struct Subject {
        #[map(default = 3)]
        attempts: u32,
    }
    let profile = Profile::new()
        .expecting("attempts", Expect::Equal("4".into()))
        .unwrap();
    let mut again = Partial::<Profile>::new();
    again.set("[\"attempts\"].expect", "equal").unwrap();
    again.set("[\"attempts\"].value", "4").unwrap();
    let again = again.build().unwrap();
    assert_eq!(again, profile);
    let subject = Subject { attempts: 3 };
    let report = again.verify(&subject);
    assert_eq!(report.findings()[0].reason(), Some(&Reason::Differs));
}
