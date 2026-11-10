// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Partial`: the order keys are set in, what text sets, what a refused key leaves alone.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::Duration;

use coqui_builder::fill::FillErrorKind;
use coqui_builder::{Datum, Describe, Inspect, KeyPath, Missing, Partial};

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Mode {
    Fast { level: u8 },
    Slow { delay: Duration },
}

#[derive(Describe, Debug, Clone, PartialEq)]
enum Colour {
    Red,
    #[map(rename = "dark-blue")]
    DarkBlue,
    #[map(other)]
    Other(String),
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Port {
    Auto,
    Number(u16),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Conf {
    flag: bool,
    count: i32,
    ratio: f64,
    name: String,
    delay: Duration,
    addr: Ipv4Addr,
    colour: Colour,
    port: Port,
    mode: Mode,
}

fn key(text: &str) -> Vec<coqui_builder::Segment> {
    text.parse::<KeyPath>().unwrap().segments().to_vec()
}

fn complete() -> Partial<Conf> {
    let mut p = Partial::<Conf>::new();
    for (k, v) in [
        ("flag", "on"),
        ("count", "-4"),
        ("ratio", "0.5"),
        ("name", "n"),
        ("delay", "1.5"),
        ("addr", "10.1.2.3"),
        ("colour", "dark-blue"),
        ("port", "8080"),
        ("mode.kind", "Slow"),
        ("mode.delay", "2s"),
    ] {
        p.set(k, v).unwrap();
    }
    p
}

#[test]
fn text_sets_every_kind_of_leaf() {
    assert_eq!(
        complete().build().unwrap(),
        Conf {
            flag: true,
            count: -4,
            ratio: 0.5,
            name: "n".into(),
            delay: Duration::new(1, 500_000_000),
            addr: Ipv4Addr::new(10, 1, 2, 3),
            colour: Colour::DarkBlue,
            port: Port::Number(8080),
            mode: Mode::Slow {
                delay: Duration::from_secs(2)
            },
        }
    );
}

#[test]
fn a_choice_reads_its_names_and_other_keeps_a_name_it_does_not_know() {
    let mut p = complete();
    p.set("colour", "Red").unwrap();
    assert_eq!(p.build().unwrap().colour, Colour::Red);
    let mut p = complete();
    p.set("colour", "teal").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.colour, Colour::Other("teal".into()));
    assert_eq!(built.at(&key("colour")), Some(Datum::new("Colour", "teal")));
}

#[test]
fn a_choice_without_other_refuses_a_name_it_does_not_have() {
    #[derive(Describe, Debug, PartialEq)]
    enum Closed {
        A,
    }
    let mut p = Partial::<Closed>::new();
    assert!(p.set_at(&[], "B").is_err());
    p.set_at(&[], "A").unwrap();
    assert_eq!(p.build().unwrap(), Closed::A);
    let info = match <Closed as Describe>::description().kind() {
        coqui_builder::describe::Kind::Leaf(info) => *info,
        other => panic!("{other:?}"),
    };
    assert_eq!(info.choices(), Some(&["A"][..]));
}

#[test]
fn an_untagged_union_reads_its_names_then_its_held_leaves_in_order() {
    let mut p = complete();
    p.set("port", "Auto").unwrap();
    assert_eq!(p.build().unwrap().port, Port::Auto);
    let mut p = complete();
    p.set("port", "443").unwrap();
    assert_eq!(p.build().unwrap().port, Port::Number(443));
    let mut p = complete();
    assert!(p.set("port", "Nope").is_err());
    let built = complete().build().unwrap();
    assert_eq!(built.at(&key("port")), Some(Datum::new("Port", "8080")));
    let auto = Conf {
        port: Port::Auto,
        ..built
    };
    assert_eq!(auto.at(&key("port")), Some(Datum::new("Port", "Auto")));
}

#[test]
fn a_key_a_leaf_refuses_keeps_what_it_held() {
    let mut p = complete();
    p.set("count", "9").unwrap();
    assert!(p.set("count", "nine").is_err());
    assert!(p.set("count", "1.5").is_err());
    assert_eq!(p.build().unwrap().count, 9);
}

#[test]
fn the_last_write_of_a_key_wins_whatever_spelling_it_came_in() {
    let mut p = complete();
    p.set("delay", "3").unwrap();
    p.set("delay", "4000ms").unwrap();
    p.set("delay", "5s").unwrap();
    assert_eq!(p.build().unwrap().delay, Duration::from_secs(5));
}

#[test]
fn a_missing_key_is_named_by_its_path_and_the_error_lists_them_all() {
    let mut p = Partial::<Conf>::new();
    p.set("mode.kind", "Fast").unwrap();
    let missing: Missing = p.build().unwrap_err();
    let keys: Vec<String> = missing.keys().iter().map(ToString::to_string).collect();
    assert_eq!(
        keys,
        [
            "flag",
            "count",
            "ratio",
            "name",
            "delay",
            "addr",
            "colour",
            "port",
            "mode.level"
        ]
    );
    assert!(
        missing
            .to_string()
            .starts_with("missing settings: flag, count")
    );
}

#[test]
fn an_arm_overlaid_replaces_the_tag_and_keeps_the_fields_it_does_not_set() {
    let mut base = complete();
    let mut later = Partial::<Conf>::new();
    later.set("mode.kind", "Fast").unwrap();
    later.set("mode.level", "7").unwrap();
    base.overlay(later);
    assert_eq!(base.build().unwrap().mode, Mode::Fast { level: 7 });
}

#[test]
fn a_field_of_an_arm_set_before_the_tag_is_answered() {
    let mut p = Partial::<Mode>::new();
    p.set("level", "3").unwrap();
    assert_eq!(p.at(&key("level")), Some(Datum::new("u8", "3")));
    p.set("kind", "Fast").unwrap();
    assert_eq!(p.build().unwrap(), Mode::Fast { level: 3 });
}

#[test]
fn a_partial_is_a_default_and_prints_without_a_value() {
    let p = Partial::<Conf>::default();
    assert!(format!("{p:?}").starts_with("Partial"));
    assert!(p.description().notes().description.is_none());
    assert!(p.build().is_err());
}

#[test]
fn a_value_overlaid_on_a_partial_made_from_it_changes_nothing() {
    let built = complete().build().unwrap();
    let mut p = Partial::from(built.clone());
    p.overlay(Partial::from(built.clone()));
    assert_eq!(p.build().unwrap(), built);
}

#[test]
fn a_secret_is_not_printed_by_a_partial_that_holds_it() {
    #[derive(Describe, Debug)]
    struct S {
        #[schema(sensitive)]
        token: String,
    }
    let mut p = Partial::<S>::new();
    p.set("token", "hunter2").unwrap();
    assert!(!format!("{p:?}").contains("hunter2"));
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Pool {
    hosts: Vec<Member>,
    labels: std::collections::BTreeMap<String, Member>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Member {
    name: String,
}

#[test]
fn an_empty_list_of_groups_and_an_empty_map_count_as_set_when_overlaid() {
    let mut p = Partial::<Pool>::new();
    p.overlay(Partial::from(Pool {
        hosts: Vec::new(),
        labels: std::collections::BTreeMap::new(),
    }));
    let built = p.build().unwrap();
    assert!(built.hosts.is_empty() && built.labels.is_empty());
}

#[test]
fn an_empty_list_of_groups_replaces_an_earlier_one_and_an_empty_map_keeps_its_entries() {
    let mut base = Partial::<Pool>::new();
    base.set("hosts[0].name", "a").unwrap();
    base.set("labels.x.name", "b").unwrap();
    base.overlay(Partial::from(Pool {
        hosts: Vec::new(),
        labels: std::collections::BTreeMap::new(),
    }));
    let built = base.build().unwrap();
    assert!(built.hosts.is_empty());
    assert_eq!(built.labels["x"].name, "b");
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Pace {
    Fast {
        #[map(default = 1)]
        level: u8,
    },
    Slow {
        #[map(default = 9)]
        level: u8,
    },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Run {
    #[map(default = Pace::Fast { level: 1 })]
    pace: Pace,
}

#[test]
fn a_shared_key_reads_its_default_from_the_chosen_arm() {
    let key: Vec<coqui_builder::Segment> =
        "pace.level".parse::<KeyPath>().unwrap().segments().to_vec();
    let mut p = Partial::<Run>::new();
    assert_eq!(p.at(&key).unwrap().text(), "1");
    p.set("pace.kind", "Slow").unwrap();
    assert_eq!(p.at(&key).unwrap().text(), "9");
    assert_eq!(p.build().unwrap().pace, Pace::Slow { level: 9 });
}

// ---- an explicit None, and a step that is not a whole number of steps ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Optionals {
    #[map(default)]
    base: u8,
    limit: Option<u32>,
    #[map(default = Some(Duration::from_secs(30)), none = "never")]
    wait: Option<Duration>,
}

#[test]
fn overlaying_an_explicit_none_clears_what_the_target_holds() {
    let mut target = Partial::<Optionals>::new();
    target.set("limit", "5").unwrap();
    target.set("wait", "10s").unwrap();
    let mut later = Partial::from(Optionals {
        base: 1,
        limit: None,
        wait: None,
    });
    later.set("base", "2").unwrap();
    target.overlay(later);
    let built = target.build().unwrap();
    assert_eq!(
        built.limit, None,
        "no `none` text: the earlier value is forgotten"
    );
    assert_eq!(
        built.wait, None,
        "a `none` text: the earlier value is replaced"
    );
    assert_eq!(built.base, 2);
}

#[test]
fn overlaying_a_set_option_still_replaces_and_an_unset_one_keeps() {
    let mut target = Partial::<Optionals>::new();
    target.set("limit", "5").unwrap();
    let mut later = Partial::<Optionals>::new();
    later.set("wait", "1s").unwrap();
    target.overlay(later);
    let built = target.build().unwrap();
    assert_eq!(built.limit, Some(5));
    assert_eq!(built.wait, Some(Duration::from_secs(1)));
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Tuning {
    ceiling: Option<u8>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Nested {
    tuning: Option<Tuning>,
    #[map(default)]
    names: Vec<String>,
}

#[test]
fn an_explicit_none_below_a_group_and_a_group_that_is_none() {
    let mut target = Partial::<Nested>::new();
    target.set("tuning.ceiling", "9").unwrap();
    let later = Partial::from(Nested {
        tuning: Some(Tuning { ceiling: None }),
        names: Vec::new(),
    });
    target.overlay(later);
    assert_eq!(
        target.build().unwrap().tuning,
        Some(Tuning { ceiling: None })
    );

    let mut target = Partial::<Nested>::new();
    target.set("tuning.ceiling", "9").unwrap();
    target.overlay(Partial::from(Nested {
        tuning: None,
        names: Vec::new(),
    }));
    assert_eq!(target.build().unwrap().tuning, None);
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Steps {
    #[schema(multiple_of = 1)]
    big: u128,
}

#[test]
fn a_step_is_checked_past_the_range_of_a_64_bit_integer() {
    let mut p = Partial::<Steps>::new();
    p.set("big", "100000000000000000000").unwrap();
    assert_eq!(p.build().unwrap().big, 100_000_000_000_000_000_000);
}

// ---- put: every key from a value, keeping the defaults ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Held {
    #[map(default = 5)]
    n: u8,
    #[map(default = Port::Auto)]
    port: Port,
    #[map(default = vec!["d".to_string()])]
    tags: Vec<String>,
    opt: Option<u8>,
    mode: Mode,
}

fn held(n: u8) -> Held {
    Held {
        n,
        port: Port::Number(80),
        tags: vec!["x".into(), "y".into()],
        opt: Some(1),
        mode: Mode::Slow {
            delay: Duration::from_secs(2),
        },
    }
}

#[test]
fn put_sets_every_key_and_clear_reads_the_default_again() {
    use coqui_builder::Fill;
    let mut p = Partial::<Held>::new();
    p.set("n", "1").unwrap();
    p.set("mode.kind", "Fast").unwrap();
    p.set("mode.level", "3").unwrap();
    Fill::put(p.__fill_mut(), held(9));
    assert_eq!(p.at(&key("n")), Some(Datum::new("u8", "9")));
    assert_eq!(p.at(&key("port")), Some(Datum::new("Port", "80")));
    assert_eq!(p.at(&key("mode.kind")), Some(Datum::new("String", "Slow")));
    let mut again = Partial::<Held>::new();
    Fill::put(again.__fill_mut(), held(9));
    assert_eq!(again.build().unwrap(), held(9));

    // The defaults the fill was given survive: clearing a key reads the default, and the
    // build has it.
    p.clear("n").unwrap();
    p.clear("port").unwrap();
    p.clear("tags").unwrap();
    p.clear("opt").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.n, 5);
    assert_eq!(built.port, Port::Auto);
    assert_eq!(built.tags, ["d"]);
    assert_eq!(built.opt, None);
}

#[test]
fn put_of_a_none_forgets_what_an_option_held_and_put_after_it_sets_again() {
    use coqui_builder::Fill;
    let mut p = Partial::<Held>::new();
    p.set("opt", "4").unwrap();
    let mut value = held(1);
    value.opt = None;
    Fill::put(p.__fill_mut(), value);
    assert_eq!(p.at(&key("opt")), None);
    Fill::put(p.__fill_mut(), held(1));
    assert_eq!(p.at(&key("opt")), Some(Datum::new("u8", "1")));
}

// ---- Partial::put: a typed value at any path ----

#[derive(Describe, Debug, Clone, PartialEq)]
struct Waiting {
    #[map(default = 10)]
    total: u32,
    #[map(default = Duration::from_secs(1))]
    interval: Duration,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Peer {
    host: String,
    #[map(default = 1)]
    weight: u8,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct CacheTable {
    ttl: u32,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Caching {
    Off,
    Flag(bool),
    Table(CacheTable),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Typed {
    waiting: Waiting,
    peers: Vec<Peer>,
    by_name: BTreeMap<String, Peer>,
    cache: Caching,
    cap: Option<Waiting>,
}

fn typed() -> Partial<Typed> {
    let mut p = Partial::new();
    p.put(
        "waiting",
        Waiting {
            total: 1,
            interval: Duration::from_secs(2),
        },
    )
    .unwrap();
    p.put("peers", Vec::<Peer>::new()).unwrap();
    p.put("by_name", BTreeMap::<String, Peer>::new()).unwrap();
    p.put("cache", Caching::Off).unwrap();
    p
}

#[test]
fn put_sets_a_nested_key_a_list_item_and_a_map_entry_by_type() {
    let mut p = typed();
    p.put("waiting.total", 7u32).unwrap();
    p.put("waiting.interval", Duration::from_millis(250))
        .unwrap();
    p.put(
        "peers[0]",
        Peer {
            host: "a".into(),
            weight: 2,
        },
    )
    .unwrap();
    p.put("peers[0].weight", 5u8).unwrap();
    p.put(
        "by_name[x]",
        Peer {
            host: "h".into(),
            weight: 1,
        },
    )
    .unwrap();
    p.put("by_name[x].host", "hh".to_owned()).unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.waiting.total, 7);
    assert_eq!(built.waiting.interval, Duration::from_millis(250));
    assert_eq!(built.peers.len(), 1);
    assert_eq!(built.peers[0].weight, 5);
    assert_eq!(built.by_name["x"].host, "hh");
}

#[test]
fn put_chooses_an_untagged_arm_by_the_type_it_is_given() {
    let mut p = typed();
    p.put("cache", true).unwrap();
    assert_eq!(p.build().unwrap().cache, Caching::Flag(true));
    let mut p = typed();
    p.put("cache", CacheTable { ttl: 3 }).unwrap();
    assert_eq!(
        p.build().unwrap().cache,
        Caching::Table(CacheTable { ttl: 3 })
    );
    let mut p = typed();
    p.put("cache.ttl", 9u32).unwrap();
    assert_eq!(
        p.build().unwrap().cache,
        Caching::Table(CacheTable { ttl: 9 })
    );
    let mut p = typed();
    p.put("cache", Caching::Flag(false)).unwrap();
    p.put("cache", CacheTable { ttl: 1 }).unwrap();
    p.put("cache", Caching::Off).unwrap();
    assert_eq!(p.build().unwrap().cache, Caching::Off);
}

#[test]
fn put_into_an_optional_takes_the_value_or_an_option_of_it() {
    let mut p = typed();
    p.put(
        "cap",
        Waiting {
            total: 2,
            interval: Duration::ZERO,
        },
    )
    .unwrap();
    assert_eq!(p.build().unwrap().cap.unwrap().total, 2);
    let mut p = typed();
    p.put("cap.total", 4u32).unwrap();
    assert_eq!(p.build().unwrap().cap.unwrap().total, 4);
    let mut p = typed();
    p.put(
        "cap",
        Some(Waiting {
            total: 6,
            interval: Duration::ZERO,
        }),
    )
    .unwrap();
    p.put("cap", None::<Waiting>).unwrap();
    assert_eq!(p.build().unwrap().cap, None);
}

#[test]
fn put_refuses_a_value_of_the_wrong_type_and_an_unknown_key_and_changes_nothing() {
    let mut p = typed();
    p.put("waiting.total", 7u32).unwrap();
    let wrong = p.put("waiting.total", "7".to_owned()).unwrap_err();
    assert!(matches!(wrong.kind(), FillErrorKind::Unsupported(_)));
    assert!(p.put("waiting.total", 7u8).is_err());
    assert!(p.put("waiting", 7u32).is_err());
    let unknown = p.put("waiting.nope", 1u32).unwrap_err();
    assert_eq!(unknown.kind(), &FillErrorKind::UnknownKey);
    assert_eq!(unknown.at().to_string(), "waiting.nope");
    assert!(p.put("nope", 1u32).is_err());
    assert!(p.put("peers[0].nope", 1u32).is_err());
    assert!(p.put("not a path[", 1u32).is_err());
    assert_eq!(p.build().unwrap().waiting.total, 7);
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Shared {
    Fast { level: u8 },
    Slow { level: u8, delay: Duration },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Sharing {
    mode: Shared,
}

#[test]
fn put_gives_a_key_two_arms_share_to_the_arm_in_force() {
    let mut p = Partial::<Sharing>::new();
    p.set("mode.kind", "Slow").unwrap();
    p.put("mode.level", 3u8).unwrap();
    p.set("mode.delay", "1s").unwrap();
    assert_eq!(
        p.build().unwrap().mode,
        Shared::Slow {
            level: 3,
            delay: Duration::from_secs(1)
        }
    );
    // With no arm chosen the first arm that has the key takes it.
    let mut p = Partial::<Sharing>::new();
    p.put("mode.level", 4u8).unwrap();
    p.set("mode.kind", "Fast").unwrap();
    assert_eq!(p.build().unwrap().mode, Shared::Fast { level: 4 });
}

struct NotDescribed;

#[test]
fn put_takes_any_value_and_refuses_one_that_is_not_the_places() {
    let mut p = typed();
    assert!(p.put("waiting.total", NotDescribed).is_err());
}

#[test]
fn put_below_an_expectation_of_a_profile_reaches_its_parts() {
    use coqui_builder::Expect;
    let mut p = Partial::<Expect>::new();
    p.set("expect", "equal").unwrap();
    p.put("value", "30s".to_owned()).unwrap();
    assert!(p.put("nope", 1u8).is_err());
    assert_eq!(p.build().unwrap(), Expect::Equal("30s".into()));
}
