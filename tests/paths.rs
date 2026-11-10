// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The path grammar and its spelling.

use coqui_builder::{KeyPath, PathError, Segment};

fn key(parts: &[&str]) -> KeyPath {
    parts.iter().map(|p| Segment::Key((*p).into())).collect()
}

fn parse(text: &str) -> Result<KeyPath, PathError> {
    text.parse()
}

#[test]
fn a_name_is_a_key_and_a_bracket_of_digits_is_a_position() {
    assert_eq!(parse("host").unwrap(), key(&["host"]));
    assert_eq!(
        parse("agent[1].name").unwrap().segments(),
        [
            Segment::Key("agent".into()),
            Segment::Index(1),
            Segment::Key("name".into())
        ]
    );
    assert_eq!(parse("[0]").unwrap().segments(), [Segment::Index(0)]);
}

#[test]
fn a_bare_bracket_of_text_is_a_map_key() {
    assert_eq!(
        parse("env[PATH]").unwrap().segments(),
        [Segment::Key("env".into()), Segment::Key("PATH".into())]
    );
    assert_eq!(parse("env.PATH").unwrap(), parse("env[PATH]").unwrap());
}

#[test]
fn a_quoted_key_can_hold_a_dot_a_bracket_a_quote_and_a_backslash() {
    assert_eq!(parse("[\"a.b\"]").unwrap(), key(&["a.b"]));
    assert_eq!(parse("env[\"a.b\"]").unwrap(), key(&["env", "a.b"]));
    assert_eq!(parse("[\"a[b]\"]").unwrap(), key(&["a[b]"]));
    assert_eq!(parse("[\"say \\\"hi\\\"\"]").unwrap(), key(&["say \"hi\""]));
    assert_eq!(parse("[\"back\\\\slash\"]").unwrap(), key(&["back\\slash"]));
}

#[test]
fn a_position_has_no_leading_zero_and_fits_a_usize() {
    assert_eq!(parse("[0]").unwrap().segments(), [Segment::Index(0)]);
    assert_eq!(parse("[01]").unwrap(), key(&["01"]));
    assert_eq!(parse("[\"1\"]").unwrap(), key(&["1"]));
    let huge = "[99999999999999999999999999]";
    assert_eq!(parse(huge).unwrap(), key(&["99999999999999999999999999"]));
}

#[test]
fn text_off_the_grammar_is_not_a_path() {
    for (bad, error) in [
        ("", PathError::Empty),
        (".a", PathError::EmptyField { at: 0 }),
        ("a.", PathError::EmptyField { at: 2 }),
        ("a..b", PathError::EmptyField { at: 2 }),
        ("a[0", PathError::UnclosedIndex { at: 1 }),
        ("a]", PathError::UnexpectedClose { at: 1 }),
        ("[a[b]", PathError::UnclosedIndex { at: 0 }),
        ("a[0]b", PathError::MissingSeparator { at: 4 }),
        ("a.[0]", PathError::EmptyField { at: 2 }),
        ("a[]", PathError::EmptyIndex { at: 1 }),
        ("a[\"x]", PathError::UnclosedQuote { at: 2 }),
        ("a[\"x\\q\"]", PathError::BadEscape { at: 4 }),
        ("a[\"x\"y]", PathError::TrailingQuote { at: 5 }),
    ] {
        assert_eq!(parse(bad), Err(error), "{bad:?}");
    }
}

#[test]
fn a_path_prints_the_way_the_grammar_reads_it() {
    assert_eq!(key(&[]).to_string(), "");
    assert_eq!(key(&["tls", "ca"]).to_string(), "tls.ca");
    assert_eq!(
        key(&["waiting", "per_source"]).to_string(),
        "waiting.per_source"
    );
    assert_eq!(
        KeyPath::new(vec![
            Segment::Key("peers".into()),
            Segment::Index(0),
            Segment::Key("name".into())
        ])
        .to_string(),
        "peers[0].name"
    );
    assert_eq!(key(&["tls.ca"]).to_string(), "[\"tls.ca\"]");
    assert_eq!(key(&["env", "a.b"]).to_string(), "env[\"a.b\"]");
    assert_eq!(key(&["a", "b.c", "d"]).to_string(), "a[\"b.c\"].d");
    assert_eq!(key(&["say \"hi\""]).to_string(), "say \"hi\"");
    assert_eq!(key(&["a]"]).to_string(), "[\"a]\"]");
    assert_eq!(key(&["1"]).to_string(), "[\"1\"]");
    assert_eq!(key(&["01"]).to_string(), "[01]");
    assert_eq!(key(&["", "x"]).to_string(), "[\"\"].x");
}

#[test]
fn every_path_parses_back_to_its_segments() {
    let paths = [
        key(&["tls", "ca"]),
        key(&["tls.ca"]),
        key(&["a", "b.c", "d"]),
        key(&["a[0]"]),
        key(&["quote\"d", "back\\slash"]),
        key(&["", "x"]),
        key(&["0"]),
        key(&["01"]),
        KeyPath::new(vec![Segment::Index(3), Segment::Key("x".into())]),
        KeyPath::new(vec![
            Segment::Key("env".into()),
            Segment::Key("1".into()),
            Segment::Index(2),
        ]),
    ];
    for path in paths {
        let text = path.to_string();
        assert_eq!(parse(&text).unwrap(), path, "{text:?}");
    }
}

#[test]
fn a_path_can_be_built_and_asked_about_its_prefix() {
    let mut path = KeyPath::root();
    assert!(path.is_empty());
    path.push("waiting");
    path.push("per_source");
    assert_eq!(path.len(), 2);
    assert!(path.starts_with(&[Segment::Key("waiting".into())]));
    assert_eq!(path.child(3usize).to_string(), "waiting.per_source[3]");
    assert_eq!(path.pop(), Some(Segment::Key("per_source".into())));
}

#[test]
fn a_position_names_an_open_map_key_by_its_text() {
    assert_eq!(Segment::Index(7).name(), "7");
    assert_eq!(Segment::Key("x".into()).name(), "x");
}
