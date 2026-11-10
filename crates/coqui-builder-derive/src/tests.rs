// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The derive's refusals and accepted spellings, held to `tests/vectors/attributes.json`.

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use serde_json::Value;
use syn::{Attribute, Data, DeriveInput, Fields};

use crate::attrs::{
    BUILDER_FIELD_KEYS, BUILDER_TYPE_KEYS, MAP_ENUM_KEYS, MAP_FIELD_KEYS, MAP_STRUCT_KEYS,
    MAP_VARIANT_KEYS, RESERVED_SETTERS, SCHEMA_ARM_KEYS, SCHEMA_CHOICE_KEYS, SCHEMA_FIELD_KEYS,
};

fn vectors() -> Value {
    serde_json::from_str(include_str!("../tests/vectors/attributes.json")).expect("valid JSON")
}

/// The items a case derives, with the helper text before the first one dropped.
fn derived(item: &str) -> Vec<String> {
    item.split("{derive}").skip(1).map(str::to_owned).collect()
}

fn expand_text(item: &str) -> syn::Result<TokenStream> {
    let tokens: TokenStream = item.parse().expect("tokens");
    crate::try_expand(tokens)
}

fn cases() -> Vec<Value> {
    vectors()["cases"].as_array().expect("cases").clone()
}

#[test]
fn every_case_is_accepted_or_refused_as_the_file_says() {
    for case in cases() {
        let name = case["name"].as_str().expect("name");
        let items = derived(case["item"].as_str().expect("item"));
        assert!(!items.is_empty(), "{name}: no {{derive}} marker");
        let results: Vec<_> = items.iter().map(|i| expand_text(i)).collect();
        match &case["coqui_builder"] {
            Value::String(s) if s == "accept" => {
                for (item, result) in items.iter().zip(&results) {
                    let tokens = result
                        .as_ref()
                        .unwrap_or_else(|e| panic!("{name}: refused: {e}\n{item}"));
                    syn::parse2::<syn::File>(tokens.clone())
                        .unwrap_or_else(|e| panic!("{name}: the expansion is not Rust: {e}"));
                }
            }
            Value::Object(o) => {
                let stem = o["refuse"].as_str().expect("a stem");
                let refusals: Vec<String> = results
                    .iter()
                    .filter_map(|r| r.as_ref().err().map(ToString::to_string))
                    .collect();
                assert!(
                    refusals.iter().any(|m| m.contains(stem)),
                    "{name}: expected a refusal containing {stem:?}, got {refusals:?}"
                );
            }
            other => panic!("{name}: unreadable outcome {other:?}"),
        }
    }
}

#[derive(Default)]
struct Seen {
    map_struct: BTreeSet<String>,
    map_enum: BTreeSet<String>,
    map_field: BTreeSet<String>,
    map_variant: BTreeSet<String>,
    schema_field: BTreeSet<String>,
    schema_arm: BTreeSet<String>,
    schema_choice: BTreeSet<String>,
    builder_field: BTreeSet<String>,
    builder_type: BTreeSet<String>,
}

fn keys(attrs: &[Attribute], name: &str, into: &mut BTreeSet<String>) {
    for attr in attrs.iter().filter(|a| a.path().is_ident(name)) {
        attr.parse_nested_meta(|meta| {
            if let Some(ident) = meta.path.get_ident() {
                into.insert(ident.to_string());
            }
            if meta.input.peek(syn::Token![=]) {
                let _: syn::Expr = meta.value()?.parse()?;
            } else if meta.input.peek(syn::token::Paren) {
                let content;
                syn::parenthesized!(content in meta.input);
                let _: TokenStream = content.parse()?;
            }
            Ok(())
        })
        .expect("attributes parse");
    }
}

fn scan(ast: &DeriveInput, seen: &mut Seen) {
    let fields_of = |fields: &Fields, seen: &mut Seen| {
        for field in fields {
            keys(&field.attrs, "map", &mut seen.map_field);
            keys(&field.attrs, "schema", &mut seen.schema_field);
            keys(&field.attrs, "builder", &mut seen.builder_field);
        }
    };
    keys(&ast.attrs, "builder", &mut seen.builder_type);
    match &ast.data {
        Data::Struct(data) => {
            keys(&ast.attrs, "map", &mut seen.map_struct);
            fields_of(&data.fields, seen);
        }
        Data::Enum(data) => {
            keys(&ast.attrs, "map", &mut seen.map_enum);
            let mut own = BTreeSet::new();
            keys(&ast.attrs, "map", &mut own);
            let tagged = own.contains("tag");
            for variant in &data.variants {
                keys(&variant.attrs, "map", &mut seen.map_variant);
                if tagged {
                    keys(&variant.attrs, "schema", &mut seen.schema_arm);
                } else {
                    keys(&variant.attrs, "schema", &mut seen.schema_choice);
                }
                fields_of(&variant.fields, seen);
            }
        }
        Data::Union(_) => {}
    }
}

#[test]
fn the_key_lists_are_exactly_the_keys_the_accepted_cases_use() {
    let mut seen = Seen::default();
    for case in cases() {
        if case["coqui_builder"] != "accept" {
            continue;
        }
        for item in derived(case["item"].as_str().expect("item")) {
            let ast: DeriveInput = syn::parse_str(&item).unwrap_or_else(|e| panic!("{item}: {e}"));
            scan(&ast, &mut seen);
        }
    }
    let list =
        |keys: &[&str]| -> BTreeSet<String> { keys.iter().map(|k| (*k).to_owned()).collect() };
    for (what, got, want) in [
        (
            "#[map] on a struct",
            &seen.map_struct,
            list(MAP_STRUCT_KEYS),
        ),
        ("#[map] on an enum", &seen.map_enum, list(MAP_ENUM_KEYS)),
        ("#[map] on a field", &seen.map_field, list(MAP_FIELD_KEYS)),
        (
            "#[map] on a variant",
            &seen.map_variant,
            list(MAP_VARIANT_KEYS),
        ),
        (
            "#[schema] on a field",
            &seen.schema_field,
            list(SCHEMA_FIELD_KEYS),
        ),
        (
            "#[schema] on an arm",
            &seen.schema_arm,
            list(SCHEMA_ARM_KEYS),
        ),
        (
            "#[schema] on a choice",
            &seen.schema_choice,
            list(SCHEMA_CHOICE_KEYS),
        ),
        (
            "#[builder] on a field",
            &seen.builder_field,
            list(BUILDER_FIELD_KEYS),
        ),
        (
            "#[builder] on a type",
            &seen.builder_type,
            list(BUILDER_TYPE_KEYS),
        ),
    ] {
        assert_eq!(
            *got, want,
            "{what}: the accepted cases and the key list disagree"
        );
    }
}

#[test]
fn the_reserved_setter_names_are_the_ones_every_builder_has() {
    assert_eq!(
        RESERVED_SETTERS,
        [
            "build",
            "fill",
            "fill_from",
            "builder",
            "new",
            "configured",
            "non_defaults",
            "partial_mut"
        ]
    );
}

#[test]
fn a_message_never_quotes_the_input_it_refused() {
    let err = expand_text("struct S { #[map(rename_all = \"x\")] a: u8 }").unwrap_err();
    assert!(!err.to_string().contains("rename_all = \"x\""));
}

#[test]
fn a_field_naming_the_owner_is_refused_unless_it_is_skipped_or_another_type() {
    let refused = |item: &str| expand_text(item).unwrap_err().to_string();
    assert!(
        refused("#[derive(Describe)] struct Node { next: Option<Box<Node>> }")
            .contains("holds itself")
    );
    assert!(
        refused("#[derive(Describe)] struct Node { next: Vec<Self> }").contains("holds itself")
    );
    for ok in [
        "#[derive(Describe)] struct Duration { d: core::time::Duration }",
        "#[derive(Describe)] struct Tls { inner: other::Tls }",
        "#[derive(Describe)] struct Node { #[map(skip)] parent: Option<Box<Node>> }",
    ] {
        assert!(expand_text(ok).is_ok(), "{ok}");
    }
}
