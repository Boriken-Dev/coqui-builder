// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Reports: what is hidden, where, and how a finding reads as text.

use std::net::Ipv4Addr;

use coqui_builder::describe::Field;
use coqui_builder::{
    Datum, Describe, Description, Expect, Finding, KeyPath, NonDefault, Reason, Report, Shown,
};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Peer {
    host: String,
    #[schema(sensitive)]
    key: String,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Auth {
    Password {
        user: String,
        #[schema(sensitive)]
        password: String,
    },
    Anonymous,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Conf {
    peers: Vec<Peer>,
    auth: Auth,
    #[schema(sensitive)]
    whole: Vec<String>,
    plain: String,
    #[map(as_text)]
    addr: Ipv4Addr,
    #[map(as_text, default)]
    addrs: Vec<Ipv4Addr>,
}

fn key(text: &str) -> KeyPath {
    text.parse().unwrap()
}

fn string(text: &str) -> Shown {
    Shown::One(Datum::new("String", text))
}

fn finding(at: &str, value: Shown, expected: Expect) -> Finding {
    Finding::new(key(at), false, value, expected, Some(Reason::Differs))
}

#[test]
fn a_sensitive_key_is_hidden_in_the_value_and_in_every_expectation() {
    let mut report = Report::new(&Conf::description());
    let masked = || Expect::Equal("***".into());
    report.push(finding(
        "auth.password",
        string("s3cret"),
        Expect::Equal("s3cret".into()),
    ));
    report.push(finding(
        "auth.password",
        string("s3cret"),
        Expect::OneOf(vec!["s3cret".into(), "b".into()]),
    ));
    report.push(finding(
        "auth.password",
        string("s3cret"),
        Expect::Within {
            min: Some("s3cret".into()),
            max: None,
        },
    ));
    report.push(finding(
        "auth.password",
        string("s3cret"),
        Expect::AsStrictAs("s3cret".into()),
    ));
    for f in report.findings() {
        assert_eq!(f.value().to_string(), "String(***)");
    }
    assert_eq!(report.findings()[0].expected(), &masked());
    assert_eq!(
        report.findings()[1].expected(),
        &Expect::OneOf(vec!["***".into(), "***".into()])
    );
    assert_eq!(
        report.findings()[2].expected(),
        &Expect::Within {
            min: Some("***".into()),
            max: None
        }
    );
    assert_eq!(
        report.findings()[3].expected(),
        &Expect::AsStrictAs("***".into())
    );
    assert!(!format!("{report:?}").contains("s3cret"));
    assert!(!format!("{report:#?}").contains("s3cret"));
}

#[test]
fn a_secret_inside_a_list_of_groups_is_hidden_through_the_item_pattern() {
    let mut report = Report::new(&Conf::description());
    report.push(finding(
        "peers[1].key",
        string("s3cret"),
        Expect::Equal("old".into()),
    ));
    report.push(finding(
        "peers[1].host",
        string("h"),
        Expect::Equal("g".into()),
    ));
    assert_eq!(report.findings()[0].value().to_string(), "String(***)");
    assert_eq!(
        report.findings()[0].expected(),
        &Expect::Equal("***".into())
    );
    assert_eq!(report.findings()[1].value().to_string(), "String(h)");
}

#[test]
fn a_secret_of_any_arm_is_hidden_and_the_tag_is_not() {
    let mut report = Report::new(&Conf::description());
    report.push(finding("auth.password", string("p"), Expect::Absent));
    report.push(finding("auth.user", string("u"), Expect::Absent));
    report.push(finding(
        "auth.kind",
        string("Password"),
        Expect::Equal("Anonymous".into()),
    ));
    assert_eq!(report.findings()[0].value().to_string(), "String(***)");
    assert_eq!(report.findings()[1].value().to_string(), "String(u)");
    assert_eq!(report.findings()[2].value().to_string(), "String(Password)");
}

#[test]
fn a_sensitive_list_is_hidden_whole_and_item_by_item() {
    let mut report = Report::new(&Conf::description());
    report.push(finding(
        "whole",
        Shown::Items(vec![Datum::new("String", "a"), Datum::new("String", "b")]),
        Expect::ItemsEqual(vec!["c".into()]),
    ));
    report.push(finding("whole[0]", string("a"), Expect::Absent));
    assert_eq!(
        report.findings()[0].value().to_string(),
        "[String(***), String(***)]"
    );
    assert_eq!(
        report.findings()[0].expected(),
        &Expect::ItemsEqual(vec!["***".into()])
    );
    assert_eq!(report.findings()[1].value().to_string(), "String(***)");
    assert!(!format!("{report:?}").contains("\"a\""));
}

#[test]
fn the_values_of_an_open_map_are_hidden_when_their_description_is_sensitive() {
    let mut secret = Description::leaf::<String>();
    secret.notes_mut().sensitive = true;
    let description = Description::group(vec![Field::new("env", Description::map(secret))]);
    let mut report = Report::new(&description);
    report.push(finding("env.A", string("1"), Expect::Absent));
    report.push(finding("env[\"b.c\"]", string("2"), Expect::Absent));
    assert_eq!(report.findings()[0].value().to_string(), "String(***)");
    assert_eq!(report.findings()[1].value().to_string(), "String(***)");
}

#[test]
fn a_place_that_is_not_secret_is_shown() {
    let mut report = Report::new(&Conf::description());
    report.push(finding(
        "plain",
        string("shown"),
        Expect::Equal("also".into()),
    ));
    assert_eq!(report.findings()[0].value().to_string(), "String(shown)");
    assert_eq!(
        report.findings()[0].expected(),
        &Expect::Equal("also".into())
    );
}

#[test]
fn a_report_says_whether_every_expectation_is_met() {
    let mut report = Report::new(&Conf::description());
    assert!(report.is_met());
    assert!(report.is_empty());
    report.push(Finding::new(
        key("plain"),
        true,
        Shown::Unset,
        Expect::Any,
        None,
    ));
    assert!(report.is_met());
    report.push(finding("plain", Shown::Unset, Expect::Required));
    assert!(!report.is_met());
    assert_eq!(report.len(), 2);
    assert_eq!(report.unmet().count(), 1);
}

#[test]
fn a_value_shows_through_its_own_type_name_and_text() {
    let one = Shown::One(Datum::new("Ipv4Addr", "10.0.0.1"));
    assert_eq!(one.to_string(), "Ipv4Addr(10.0.0.1)");
    assert_eq!(Shown::Unset.to_string(), "unset");
    let list = Shown::Items(vec![
        Datum::new("Ipv4Addr", "10.0.0.1"),
        Datum::new("Ipv4Addr", "10.0.0.2"),
    ]);
    assert_eq!(list.to_string(), "[Ipv4Addr(10.0.0.1), Ipv4Addr(10.0.0.2)]");
}

#[test]
fn an_as_text_field_and_each_item_of_an_as_text_list_show_the_types_own_text() {
    let conf = Conf {
        peers: vec![],
        auth: Auth::Anonymous,
        whole: vec![],
        plain: "p".into(),
        addr: Ipv4Addr::new(10, 0, 0, 9),
        addrs: vec![Ipv4Addr::new(10, 0, 0, 1), Ipv4Addr::new(10, 0, 0, 2)],
    };
    let report = coqui_builder::differences(&Conf::description(), &conf, None);
    let shown: Vec<(String, String)> = report
        .findings()
        .iter()
        .map(|f| (f.key().to_string(), f.value().to_string()))
        .collect();
    assert!(shown.contains(&("addr".to_string(), "Ipv4Addr(10.0.0.9)".to_string())));
    assert!(shown.contains(&(
        "addrs".to_string(),
        "[Ipv4Addr(10.0.0.1), Ipv4Addr(10.0.0.2)]".to_string()
    )));
}

#[test]
fn a_finding_reads_as_a_non_default_with_the_type_of_the_value() {
    let n = NonDefault::from(&finding(
        "setting",
        Shown::One(Datum::new("u32", "16")),
        Expect::Equal("8".into()),
    ));
    assert_eq!(n.to_string(), "setting = u32(16) (default u32(8))");
    let n = NonDefault::from(&finding(
        "setting",
        Shown::Unset,
        Expect::Equal("root".into()),
    ));
    assert_eq!(n.to_string(), "setting = unset (default String(root))");
    let n = NonDefault::from(&finding("setting", string("x"), Expect::Absent));
    assert_eq!(n.default, Shown::Unset);
    let n = NonDefault::from(&finding("env[\"a.b\"]", string("1"), Expect::Absent));
    assert_eq!(n.setting, "env[\"a.b\"]");
}
