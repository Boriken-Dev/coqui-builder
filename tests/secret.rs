// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `Secret`: sensitive in the description, masked wherever it is shown, built like any leaf,
//! and zeroized when it drops.

use std::cell::Cell;
use std::rc::Rc;

use coqui_builder::{Describe, Inspect, Partial, Report, Secret, Segment, Shown, non_defaults};
use zeroize::Zeroize;

#[derive(Describe, Debug, Clone, PartialEq)]
struct Login {
    user: String,
    token: Secret<String>,
    port: Secret<u16>,
}

fn key(name: &str) -> [Segment; 1] {
    [Segment::key(name)]
}

fn login() -> Login {
    Login {
        user: "ana".into(),
        token: Secret::new("hunter2".into()),
        port: Secret::new(5432),
    }
}

#[test]
fn debug_is_masked_and_the_value_is_behind_expose() {
    let secret = Secret::new("hunter2".to_owned());
    assert_eq!(format!("{secret:?}"), "Secret(***)");
    assert_eq!(secret.expose(), "hunter2");
    assert_eq!(Secret::from(7u16).expose(), &7);
    assert_eq!(secret.clone(), secret);
    assert!(format!("{:?}", login()).contains("Secret(***)"));
    assert!(!format!("{:?}", login()).contains("hunter2"));
}

#[test]
fn a_secret_field_is_sensitive_in_the_description() {
    let description = Login::description();
    let field = |name| {
        description
            .group_fields()
            .unwrap()
            .iter()
            .find(|f| &*f.key == name)
            .unwrap()
    };
    assert!(field("token").description.notes().sensitive);
    assert!(field("port").description.notes().sensitive);
    assert!(!field("user").description.notes().sensitive);
}

#[test]
fn a_datum_read_from_a_secret_is_sensitive() {
    let value = login();
    let datum = value.at(&key("token")).unwrap();
    assert!(datum.is_sensitive());
    assert_eq!(datum.to_string(), "String(***)");
    assert_eq!(datum.text(), "hunter2");
    assert_eq!(value.at(&key("port")).unwrap().to_string(), "u16(***)");
    assert!(!value.at(&key("user")).unwrap().is_sensitive());
    let partial = Partial::from(login());
    assert_eq!(
        partial.at(&key("token")).unwrap().to_string(),
        "String(***)"
    );
}

#[test]
fn a_report_and_non_defaults_show_the_mask() {
    let found = non_defaults(&login());
    let token = found.iter().find(|n| n.setting == "token").unwrap();
    assert_eq!(token.value.to_string(), "String(***)");
    assert!(!format!("{found:?}").contains("hunter2"));

    let mut report = Report::new(&Login::description());
    let datum = login().at(&key("token")).unwrap();
    report.push(coqui_builder::Finding::new(
        "token".parse().unwrap(),
        false,
        Shown::One(datum),
        coqui_builder::Expect::Equal("other".into()),
        Some(coqui_builder::Reason::Differs),
    ));
    let finding = report.findings().first().unwrap();
    assert_eq!(finding.value().to_string(), "String(***)");
}

#[test]
fn a_secret_is_built_through_partial_set() {
    let mut partial = Partial::<Login>::new();
    partial.set("user", "ana").unwrap();
    partial.set("token", "hunter2").unwrap();
    partial.set("port", "5432").unwrap();
    assert_eq!(partial.build().unwrap(), login());
}

#[test]
fn a_bad_secret_text_is_refused_without_quoting_it() {
    let mut partial = Partial::<Login>::new();
    let error = partial.set("port", "not-a-port-9999").unwrap_err();
    assert!(!error.to_string().contains("9999"), "{error}");
}

#[test]
fn a_secret_round_trips_through_partial_from_and_build() {
    let built = Partial::from(login()).build().unwrap();
    assert_eq!(built, login());
    assert_eq!(*built.port.expose(), 5432);
}

#[test]
fn a_secret_takes_a_typed_put() {
    let mut partial = Partial::<Login>::new();
    partial.put("port", Secret::new(80u16)).unwrap();
    partial.put("token", Secret::new("t".to_owned())).unwrap();
    partial.set("user", "ana").unwrap();
    assert_eq!(*partial.build().unwrap().port.expose(), 80);
}

struct Probe(Rc<Cell<u32>>);

impl Zeroize for Probe {
    fn zeroize(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn dropping_a_secret_zeroizes_its_value_once() {
    let wiped = Rc::new(Cell::new(0));
    let secret = Secret::new(Probe(wiped.clone()));
    assert_eq!(wiped.get(), 0);
    drop(secret);
    assert_eq!(wiped.get(), 1);
}

#[test]
fn a_replaced_secret_is_wiped_when_the_old_one_drops() {
    let wiped = Rc::new(Cell::new(0));
    let mut secret = Secret::new(Probe(wiped.clone()));
    let old = std::mem::replace(&mut secret, Secret::new(Probe(Rc::new(Cell::new(0)))));
    assert_eq!(wiped.get(), 0);
    drop(old);
    assert_eq!(wiped.get(), 1);
}
