// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tagged enum: three arms in the description, each built through a `Partial`, and a
//! changed password hidden in a report and in `non_defaults`.

use coqui_builder::describe::Kind;
use coqui_builder::{
    Check, Datum, Describe, Entries, Expect, Inspect, Partial, Profile, Reason, Segment, Shown,
    differences, non_defaults,
};

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Credential {
    /// A user and a password.
    Password {
        user: String,
        #[schema(sensitive)]
        password: String,
    },
    KeyFile {
        path: String,
    },
    Token {
        #[schema(sensitive)]
        token: String,
    },
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Login {
    #[map(default = Credential::Password { user: "root".into(), password: String::new() })]
    credential: Credential,
    #[map(default = 22)]
    port: u16,
}

fn key(text: &str) -> Vec<Segment> {
    text.parse::<coqui_builder::KeyPath>()
        .unwrap()
        .segments()
        .to_vec()
}

#[test]
fn the_description_has_three_arms_with_their_own_fields() {
    let d = Credential::description();
    let Kind::Variant(variant) = d.kind() else {
        panic!("a variant")
    };
    assert_eq!(variant.tag(), "kind");
    let names: Vec<&str> = variant.arms().iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Password", "KeyFile", "Token"]);
    let fields = |arm: &str| -> Vec<&str> {
        variant
            .arm(arm)
            .unwrap()
            .fields
            .iter()
            .map(|f| f.key.as_str())
            .collect()
    };
    assert_eq!(fields("Password"), ["user", "password"]);
    assert_eq!(fields("KeyFile"), ["path"]);
    assert_eq!(fields("Token"), ["token"]);
    assert_eq!(
        variant.arm("Password").unwrap().description.as_deref(),
        Some("A user and a password.")
    );
    assert!(
        variant.arm("Token").unwrap().fields[0]
            .description
            .notes()
            .sensitive
    );
}

#[test]
fn each_arm_is_built_through_a_partial() {
    let mut p = Partial::<Credential>::new();
    p.set("kind", "Password").unwrap();
    p.set("user", "ana").unwrap();
    p.set("password", "pw").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Credential::Password {
            user: "ana".into(),
            password: "pw".into()
        }
    );
    let mut p = Partial::<Credential>::new();
    p.set("kind", "KeyFile").unwrap();
    p.set("path", "/k").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Credential::KeyFile { path: "/k".into() }
    );
    let mut p = Partial::<Credential>::new();
    p.set("kind", "Token").unwrap();
    p.set("token", "t").unwrap();
    assert_eq!(p.build().unwrap(), Credential::Token { token: "t".into() });
}

#[test]
fn keys_of_the_other_arms_are_set_but_not_built() {
    let mut p = Partial::<Credential>::new();
    p.set("user", "ana").unwrap();
    p.set("path", "/k").unwrap();
    p.set("token", "t").unwrap();
    p.set("kind", "KeyFile").unwrap();
    assert_eq!(
        p.build().unwrap(),
        Credential::KeyFile { path: "/k".into() }
    );
}

#[test]
fn a_missing_tag_and_a_missing_field_of_the_chosen_arm_are_listed() {
    assert_eq!(
        Partial::<Credential>::new()
            .build()
            .unwrap_err()
            .to_string(),
        "missing settings: kind"
    );
    let mut p = Partial::<Credential>::new();
    p.set("kind", "Password").unwrap();
    assert_eq!(
        p.build().unwrap_err().to_string(),
        "missing settings: user, password"
    );
}

#[test]
fn a_name_that_is_no_arm_and_a_key_that_is_no_field_are_refused() {
    let mut p = Partial::<Credential>::new();
    assert!(p.set("kind", "Nothing").is_err());
    assert!(p.set("nope", "x").is_err());
    assert!(p.set("kind.x", "x").is_err());
}

#[test]
fn a_built_value_answers_its_tag_its_fields_and_the_keys_that_are_set() {
    let value = Credential::KeyFile { path: "/k".into() };
    assert_eq!(
        value.at(&key("kind")),
        Some(Datum::new("String", "KeyFile"))
    );
    assert_eq!(value.at(&key("path")), Some(Datum::new("String", "/k")));
    assert_eq!(value.at(&key("user")), None);
    assert_eq!(
        value.entries(&[]),
        Some(Entries::Keys(vec!["kind".into(), "path".into()]))
    );
}

#[test]
fn a_partial_made_from_a_value_chooses_its_arm() {
    let value = Credential::Token { token: "t".into() };
    let partial = Partial::from(value.clone());
    assert_eq!(
        partial.at(&key("kind")),
        Some(Datum::new("String", "Token"))
    );
    assert_eq!(partial.build().unwrap(), value);
}

#[test]
fn the_default_arm_is_taken_when_the_tag_is_not_set() {
    let login = Partial::<Login>::new().build().unwrap();
    assert_eq!(
        login.credential,
        Credential::Password {
            user: "root".into(),
            password: String::new()
        }
    );
    let mut p = Partial::<Login>::new();
    p.set("credential.password", "pw").unwrap();
    assert_eq!(
        p.build().unwrap().credential,
        Credential::Password {
            user: "root".into(),
            password: "pw".into()
        }
    );
    let mut p = Partial::<Login>::new();
    p.set("credential.kind", "KeyFile").unwrap();
    assert_eq!(
        p.build().unwrap_err().to_string(),
        "missing settings: credential.path"
    );
}

#[test]
fn a_changed_password_is_a_non_default_shown_as_masked() {
    let login = Login {
        credential: Credential::Password {
            user: "root".into(),
            password: "hunter2".into(),
        },
        port: 22,
    };
    let found = non_defaults(&login);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].setting, "credential.password");
    assert_eq!(found[0].value.to_string(), "String(***)");
    assert_eq!(found[0].default.to_string(), "String(***)");
    assert!(!format!("{found:?}").contains("hunter2"));
}

#[test]
fn a_password_that_did_not_change_is_not_listed() {
    let login = Login {
        credential: Credential::Password {
            user: "root".into(),
            password: String::new(),
        },
        port: 22,
    };
    assert!(non_defaults(&login).is_empty());
}

#[test]
fn a_report_hides_the_password_in_what_it_found_and_what_it_expected() {
    let login = Login {
        credential: Credential::Password {
            user: "ana".into(),
            password: "hunter2".into(),
        },
        port: 2222,
    };
    let report = differences(&Login::description(), &login, None);
    assert!(!format!("{report:?}").contains("hunter2"));
    let keys: Vec<String> = report
        .findings()
        .iter()
        .map(|f| f.key().to_string())
        .collect();
    assert_eq!(keys, ["credential.password", "credential.user", "port"]);
    let password = &report.findings()[0];
    assert_eq!(password.value().to_string(), "String(***)");
    assert_eq!(password.expected(), &Expect::Equal("***".into()));
}

#[test]
fn another_arm_is_one_difference_at_the_tag() {
    let login = Login {
        credential: Credential::Token {
            token: "tok".into(),
        },
        port: 22,
    };
    let report = differences(&Login::description(), &login, None);
    assert_eq!(report.len(), 1);
    let finding = &report.findings()[0];
    assert_eq!(finding.key().to_string(), "credential.kind");
    assert_eq!(finding.value(), &Shown::One(Datum::new("String", "Token")));
    assert_eq!(finding.expected(), &Expect::Equal("Password".into()));
}

#[test]
fn a_profile_that_allows_two_arms_judges_a_value_of_either() {
    let profile = Profile::new()
        .expecting(
            "credential",
            Expect::Arms(vec!["Password".into(), "Token".into()]),
        )
        .unwrap();
    let ok = Login {
        credential: Credential::Token { token: "t".into() },
        port: 22,
    };
    assert!(profile.verify(&ok).is_met());
    let bad = Login {
        credential: Credential::KeyFile { path: "p".into() },
        port: 22,
    };
    let report = profile.verify(&bad);
    assert!(!report.is_met());
    assert_eq!(report.findings()[0].reason(), Some(&Reason::ArmNotAllowed));
}
