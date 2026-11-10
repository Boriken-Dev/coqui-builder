// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! An `Option` with a written None: `default = Some(..)` and `none = "never"`.

use core::time::Duration;

use coqui_builder::describe::Kind;
use coqui_builder::{Datum, Describe, Inspect, Partial, Segment, Shown, non_defaults};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Timeouts {
    #[map(default = Some(Duration::from_secs(30)), none = "never")]
    read_timeout: Option<Duration>,
}

fn built(set: impl FnOnce(&mut Partial<Timeouts>)) -> Option<Duration> {
    let mut p = Partial::<Timeouts>::new();
    set(&mut p);
    p.build().unwrap().read_timeout
}

fn key() -> Vec<Segment> {
    vec![Segment::Key("read_timeout".into())]
}

#[test]
fn absent_is_the_default() {
    assert_eq!(built(|_| {}), Some(Duration::from_secs(30)));
}

#[test]
fn the_text_is_none_and_every_duration_spelling_is_some() {
    assert_eq!(built(|p| p.set("read_timeout", "never").unwrap()), None);
    for (text, want) in [
        ("45", Duration::from_secs(45)),
        ("30", Duration::from_secs(30)),
        ("0.25", Duration::from_millis(250)),
        ("30s", Duration::from_secs(30)),
        ("250ms", Duration::from_millis(250)),
        ("1.5h", Duration::from_secs(5400)),
        ("0", Duration::ZERO),
    ] {
        assert_eq!(
            built(|p| p.set("read_timeout", text).unwrap()),
            Some(want),
            "{text}"
        );
    }
}

#[test]
fn a_structure_where_a_leaf_is_expected_and_a_bad_text_are_refused() {
    let mut p = Partial::<Timeouts>::new();
    for bad in ["[30, 0]", "{secs: 30}", "soon", "-1", "Never"] {
        assert!(p.set("read_timeout", bad).is_err(), "{bad}");
    }
}

#[test]
fn the_last_write_of_the_key_wins() {
    assert_eq!(
        built(|p| {
            p.set("read_timeout", "never").unwrap();
            p.set("read_timeout", "9").unwrap();
        }),
        Some(Duration::from_secs(9))
    );
    assert_eq!(
        built(|p| {
            p.set("read_timeout", "9").unwrap();
            p.set("read_timeout", "never").unwrap();
        }),
        None
    );
}

#[test]
fn the_description_states_the_default_and_the_text() {
    let d = Timeouts::description();
    let Kind::Group(group) = d.kind() else {
        panic!("a group")
    };
    let field = group.field("read_timeout").unwrap();
    assert_eq!(field.none.as_deref(), Some("never"));
    assert_eq!(field.default.as_deref(), Some("30s"));
    assert!(field.optional && !field.required);
    assert!(matches!(field.description.kind(), Kind::Leaf(info) if info.ty() == "Duration"));
}

#[test]
fn a_value_is_read_back_as_it_would_be_written() {
    let none = Timeouts { read_timeout: None };
    let some = Timeouts {
        read_timeout: Some(Duration::from_millis(60_005)),
    };
    assert_eq!(none.at(&key()), Some(Datum::new("Duration", "never")));
    assert_eq!(some.at(&key()), Some(Datum::new("Duration", "60005ms")));
}

#[test]
fn a_round_trip_through_inspect_gives_the_same_value() {
    for value in [None, Some(Duration::new(1, 2)), Some(Duration::ZERO)] {
        let original = Timeouts {
            read_timeout: value,
        };
        let datum = original.at(&key()).unwrap();
        let mut p = Partial::<Timeouts>::new();
        p.set_at(&key(), datum.text()).unwrap();
        assert_eq!(p.build().unwrap(), original);
    }
}

#[test]
fn a_partial_made_from_none_reads_back_as_the_text() {
    let p = Partial::from(Timeouts { read_timeout: None });
    assert_eq!(p.at(&key()), Some(Datum::new("Duration", "never")));
    assert_eq!(p.build().unwrap().read_timeout, None);
}

#[test]
fn only_a_value_that_is_not_the_default_is_a_non_default() {
    // 30s is 30000ms: typed values are compared, not texts.
    assert!(
        non_defaults(&Timeouts {
            read_timeout: Some(Duration::from_millis(30_000))
        })
        .is_empty()
    );
    let found = non_defaults(&Timeouts { read_timeout: None });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].setting, "read_timeout");
    assert_eq!(found[0].value.to_string(), "Duration(never)");
    assert_eq!(found[0].default.to_string(), "Duration(30s)");
    let found = non_defaults(&Timeouts {
        read_timeout: Some(Duration::from_secs(5)),
    });
    assert_eq!(found[0].value, Shown::One(Datum::new("Duration", "5s")));
}

#[test]
fn a_later_none_replaces_an_earlier_value_in_an_overlay() {
    let mut base = Partial::<Timeouts>::new();
    base.set("read_timeout", "5").unwrap();
    base.overlay(Partial::from(Timeouts { read_timeout: None }));
    assert_eq!(base.build().unwrap().read_timeout, None);
}

#[test]
fn the_text_of_none_is_a_value_a_profile_and_a_comparison_understand() {
    use coqui_builder::{Check, Expect, Profile, differences};
    let never = Timeouts { read_timeout: None };
    let thirty = Timeouts {
        read_timeout: Some(Duration::from_secs(30)),
    };
    assert!(Profile::against(&never).verify(&never).is_met());
    assert!(!Profile::against(&never).verify(&thirty).is_met());
    assert!(differences(&Timeouts::description(), &never, Some(&never)).is_empty());
    assert_eq!(
        differences(&Timeouts::description(), &never, Some(&thirty)).len(),
        1
    );
    let equal = Profile::new()
        .expecting("read_timeout", Expect::Equal("never".into()))
        .unwrap();
    equal.bind::<Timeouts>().unwrap();
    assert!(equal.verify(&never).is_met());
    let one_of = Profile::new()
        .expecting(
            "read_timeout",
            Expect::OneOf(vec!["never".into(), "30s".into()]),
        )
        .unwrap();
    one_of.bind::<Timeouts>().unwrap();
    assert!(one_of.verify(&thirty).is_met() && one_of.verify(&never).is_met());
    assert!(
        Profile::new()
            .expecting("read_timeout", Expect::Equal("sometimes".into()))
            .unwrap()
            .bind::<Timeouts>()
            .is_err()
    );
}
