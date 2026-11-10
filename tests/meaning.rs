// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What each spelling of `#[map(..)]`, `#[schema(..)]` and `#[builder(..)]` means, rendered
//! from `crates/coqui-builder-derive/tests/vectors/attributes.json`: the vectors guatiao's
//! derives are held to as well, so a type can move between the two.
//!
//! Between the markers below is generated: `COQUI_BUILDER_WRITE_MEANING=1 cargo test --test
//! meaning` rewrites it, and `the_rendered_cases_are_current` fails when it is not what the
//! vectors render to. Refusals are held by the derive crate's own tests.

#![allow(dead_code, unused)]

use std::fs;
use std::path::Path;

use coqui_builder::Description;
use coqui_builder::describe::{Kind, Stricter};
use serde_json::{Value, json};

// ---- helpers the rendered cases call ----

fn kind_name(d: &Description) -> &'static str {
    match d.kind() {
        Kind::Leaf(_) => "leaf",
        Kind::List(_) => "list",
        Kind::Map(_) => "map",
        Kind::Group(_) => "group",
        Kind::Variant(_) => "variant",
        Kind::Union(_) => "union",
        _ => "other",
    }
}

fn group_of(d: &Description) -> &coqui_builder::describe::Group {
    match d.kind() {
        Kind::Group(group) => group,
        other => panic!("not a group: {other:?}"),
    }
}

fn keys(d: &Description) -> Vec<String> {
    group_of(d).fields.iter().map(|f| f.key.clone()).collect()
}

fn required(d: &Description) -> Vec<String> {
    group_of(d)
        .fields
        .iter()
        .filter(|f| f.required)
        .map(|f| f.key.clone())
        .collect()
}

fn sensitive(d: &Description) -> Vec<String> {
    group_of(d)
        .fields
        .iter()
        .filter(|f| f.description.notes().sensitive)
        .map(|f| f.key.clone())
        .collect()
}

fn field<'a>(d: &'a Description, key: &str) -> &'a coqui_builder::describe::Field {
    group_of(d)
        .field(key)
        .unwrap_or_else(|| panic!("no field {key}"))
}

fn leaf_of(d: &Description) -> &coqui_builder::describe::LeafInfo {
    match d.kind() {
        Kind::Leaf(info) => info,
        other => panic!("not a leaf: {other:?}"),
    }
}

fn choices(d: &Description) -> Vec<String> {
    leaf_of(d)
        .choices()
        .expect("a leaf with choices")
        .iter()
        .map(|c| (*c).to_owned())
        .collect()
}

fn arms(d: &Description) -> (String, Vec<String>) {
    match d.kind() {
        Kind::Variant(variant) => (
            variant.tag().to_owned(),
            variant.arms().iter().map(|a| a.name.clone()).collect(),
        ),
        other => panic!("not a variant: {other:?}"),
    }
}

fn default_of(d: &Description, key: &str) -> Value {
    json!(
        field(d, key)
            .default
            .as_deref()
            .unwrap_or_else(|| panic!("no default for {key}"))
    )
}

// ---- the rendering ----

const BEGIN: &str = concat!("// BEGIN ", "GENERATED");
const END: &str = concat!("// END ", "GENERATED");

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

/// The name of the last derived item of `item`, and whether it is generic.
fn last_item(item: &str) -> (String, bool) {
    let piece = item.rsplit("{derive}").next().expect("a piece");
    let at = piece
        .find("struct ")
        .map(|i| i + "struct ".len())
        .or_else(|| piece.find("enum ").map(|i| i + "enum ".len()))
        .or_else(|| piece.find("union ").map(|i| i + "union ".len()))
        .expect("an item");
    let rest = &piece[at..];
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let generic = rest[end..].starts_with('<');
    (rest[..end].to_owned(), generic)
}

fn literal(text: &str) -> String {
    format!("{text:?}")
}

fn strings(value: &Value) -> String {
    let items: Vec<String> = value
        .as_array()
        .expect("a list")
        .iter()
        .map(|v| literal(v.as_str().expect("text")))
        .collect();
    if items.is_empty() {
        return "Vec::<&str>::new()".to_owned();
    }
    format!("vec![{}]", items.join(", "))
}

fn render() -> String {
    let vectors: Value = serde_json::from_str(include_str!(
        "../crates/coqui-builder-derive/tests/vectors/attributes.json"
    ))
    .expect("valid JSON");
    let mut out = String::new();
    for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
        if case["coqui_builder"] != "accept" {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let item = case
            .get("coqui_item")
            .unwrap_or(&case["item"])
            .as_str()
            .unwrap();
        let (ty, generic) = last_item(item);
        let module = format!("case_{index:02}_{}", slug(name));
        let derived = item.replace("{derive}", "#[derive(::coqui_builder::Describe)]");
        let ty_expr = if generic {
            format!("{ty}::<u16>")
        } else {
            ty.clone()
        };
        out.push_str(&format!(
            "#[rustfmt::skip]\n#[allow(dead_code, unused)]\nmod {module} {{\n"
        ));
        out.push_str("    use super::*;\n\n");
        out.push_str(&format!("    {derived}\n\n"));
        out.push_str("    pub fn check() {\n");
        out.push_str(&format!(
            "        let d = <{ty_expr} as ::coqui_builder::Describe>::description();\n"
        ));
        if let Some(expect) = case.get("coqui_expect").or_else(|| case.get("expect")) {
            if let Some(kind) = expect.get("kind") {
                out.push_str(&format!(
                    "        assert_eq!(kind_name(&d), {});\n",
                    literal(kind.as_str().unwrap())
                ));
            }
            if let Some(list) = expect.get("keys") {
                out.push_str(&format!(
                    "        assert_eq!(keys(&d), {});\n",
                    strings(list)
                ));
            }
            if let Some(list) = expect.get("required") {
                out.push_str(&format!(
                    "        assert_eq!(required(&d), {});\n",
                    strings(list)
                ));
            }
            if let Some(list) = expect.get("sensitive") {
                out.push_str(&format!(
                    "        assert_eq!(sensitive(&d), {});\n",
                    strings(list)
                ));
            }
            if let Some(ty) = expect.get("type") {
                out.push_str(&format!(
                    "        assert_eq!(leaf_of(&d).ty(), {});\n",
                    literal(ty.as_str().unwrap())
                ));
            }
            if let Some(list) = expect.get("choices") {
                out.push_str(&format!(
                    "        assert_eq!(choices(&d), {});\n",
                    strings(list)
                ));
            }
            if let Some(tag) = expect.get("tag") {
                out.push_str(&format!(
                    "        assert_eq!(arms(&d).0, {});\n",
                    literal(tag.as_str().unwrap())
                ));
            }
            if let Some(list) = expect.get("arms") {
                out.push_str(&format!(
                    "        assert_eq!(arms(&d).1, {});\n",
                    strings(list)
                ));
            }
            if expect.get("deny_unknown_fields") == Some(&json!(true)) {
                out.push_str("        assert!(group_of(&d).deny_unknown_fields);\n");
            }
            for (key, want) in expect
                .get("defaults")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                out.push_str(&format!(
                    "        assert_eq!(default_of(&d, {}), json!({}));\n",
                    literal(key),
                    want
                ));
            }
            for (key, want) in expect
                .get("descriptions")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                out.push_str(&format!(
                    "        assert_eq!(field(&d, {}).description.notes().description.as_deref(), Some({}));\n",
                    literal(key),
                    literal(want.as_str().unwrap())
                ));
            }
            for (key, want) in expect
                .get("titles")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                out.push_str(&format!(
                    "        assert_eq!(field(&d, {}).description.notes().title.as_deref(), Some({}));\n",
                    literal(key),
                    literal(want.as_str().unwrap())
                ));
            }
            for (key, want) in expect
                .get("field_kinds")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                out.push_str(&format!(
                    "        assert_eq!(kind_name(&field(&d, {}).description), {});\n",
                    literal(key),
                    literal(want.as_str().unwrap())
                ));
            }
            for (key, want) in expect
                .get("stricter")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let way = if want == "lower" { "Lower" } else { "Higher" };
                out.push_str(&format!(
                    "        assert_eq!(field(&d, {}).description.notes().stricter, Some(Stricter::{way}));\n",
                    literal(key)
                ));
            }
            for (key, want) in expect
                .get("none")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                out.push_str(&format!(
                    "        assert_eq!(field(&d, {}).none.as_deref(), Some({}));\n",
                    literal(key),
                    literal(want.as_str().unwrap())
                ));
            }
        }
        out.push_str("    }\n}\n\n");
        out.push_str(&format!(
            "#[test]\nfn {module}() {{\n    {module}::check();\n}}\n\n"
        ));
    }
    out
}

#[test]
fn the_rendered_cases_are_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/meaning.rs");
    let source = fs::read_to_string(&path).unwrap();
    let begin = source.rfind(BEGIN).expect("the begin marker") + BEGIN.len();
    let end = source.rfind(END).expect("the end marker");
    let current = &source[begin..end];
    let rendered = format!("\n{}", render());
    if std::env::var_os("COQUI_BUILDER_WRITE_MEANING").is_some() {
        let mut next = String::new();
        next.push_str(&source[..begin]);
        next.push_str(&rendered);
        next.push_str(&source[end..]);
        if next != source {
            fs::write(&path, next).unwrap();
        }
        return;
    }
    assert!(
        current == rendered,
        "tests/meaning.rs is not what attributes.json renders to: run \
         `COQUI_BUILDER_WRITE_MEANING=1 cargo test --test meaning` and commit the result"
    );
}

// BEGIN GENERATED
#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_00_fields_keep_their_names {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { host: String, port: u16 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["host", "port"]);
        assert_eq!(required(&d), vec!["host", "port"]);
    }
}

#[test]
fn case_00_fields_keep_their_names() {
    case_00_fields_keep_their_names::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_01_rename_all_kebab_case {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(rename_all = "kebab-case")] struct S { view_only: bool, max_len: u32 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["view-only", "max-len"]);
    }
}

#[test]
fn case_01_rename_all_kebab_case() {
    case_01_rename_all_kebab_case::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_02_rename_all_screaming_snake_case {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(rename_all = "SCREAMING_SNAKE_CASE")] struct S { view_only: bool }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["VIEW_ONLY"]);
    }
}

#[test]
fn case_02_rename_all_screaming_snake_case() {
    case_02_rename_all_screaming_snake_case::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_03_rename_all_lowercase {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(rename_all = "lowercase")] struct S { view_only: bool }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["view_only"]);
    }
}

#[test]
fn case_03_rename_all_lowercase() {
    case_03_rename_all_lowercase::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_04_rename_all_snake_case {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(rename_all = "snake_case")] struct S { view_only: bool }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["view_only"]);
    }
}

#[test]
fn case_04_rename_all_snake_case() {
    case_04_rename_all_snake_case::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_05_a_field_s_own_rename_wins_over_rename_all {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(rename_all = "kebab-case")] struct S { #[map(rename = "Host")] host_name: String, max_len: u32 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["Host", "max-len"]);
    }
}

#[test]
fn case_05_a_field_s_own_rename_wins_over_rename_all() {
    case_05_a_field_s_own_rename_wins_over_rename_all::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_06_a_skipped_field_is_not_a_key {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { a: u8, #[map(skip)] b: u8 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["a"]);
        assert_eq!(required(&d), vec!["a"]);
    }
}

#[test]
fn case_06_a_skipped_field_is_not_a_key() {
    case_06_a_skipped_field_is_not_a_key::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_07_a_default_makes_a_key_optional {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[map(default)] n: u32, #[map(default = 5)] m: u32, #[schema(default = 7)] k: u32 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["n", "m", "k"]);
        assert_eq!(required(&d), Vec::<&str>::new());
        assert_eq!(default_of(&d, "k"), json!("7"));
        assert_eq!(default_of(&d, "m"), json!("5"));
        assert_eq!(default_of(&d, "n"), json!("0"));
    }
}

#[test]
fn case_07_a_default_makes_a_key_optional() {
    case_07_a_default_makes_a_key_optional::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_08_an_option_is_not_required {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { a: String, b: Option<String> }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["a", "b"]);
        assert_eq!(required(&d), vec!["a"]);
    }
}

#[test]
fn case_08_an_option_is_not_required() {
    case_08_an_option_is_not_required::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_09_a_doc_comment_is_the_description {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { /// The host name.
    /// Written in DNS form.
    host: String }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["host"]);
        assert_eq!(field(&d, "host").description.notes().description.as_deref(), Some("The host name. Written in DNS form."));
    }
}

#[test]
fn case_09_a_doc_comment_is_the_description() {
    case_09_a_doc_comment_is_the_description::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_10_schema_description_wins_over_the_doc_comment {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { /// Ignored.
    #[schema(description = "Stated.", title = "Host")] host: String }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["host"]);
        assert_eq!(field(&d, "host").description.notes().description.as_deref(), Some("Stated."));
        assert_eq!(field(&d, "host").description.notes().title.as_deref(), Some("Host"));
    }
}

#[test]
fn case_10_schema_description_wins_over_the_doc_comment() {
    case_10_schema_description_wins_over_the_doc_comment::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_11_a_sensitive_field {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { user: String, #[schema(sensitive)] token: String }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["user", "token"]);
        assert_eq!(sensitive(&d), vec!["token"]);
    }
}

#[test]
fn case_11_a_sensitive_field() {
    case_11_a_sensitive_field::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_12_section_order_advanced_read_only_deprecated_and_bounds {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[schema(section = "Network", order = 2, advanced, read_only, deprecated, min_length = 1, max_length = 8, pattern = "^a", format = "hostname")] name: String, #[schema(multiple_of = 5, min_items = 1, max_items = 2, examples("a".to_string()))] other: String }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["name", "other"]);
    }
}

#[test]
fn case_12_section_order_advanced_read_only_deprecated_and_bounds() {
    case_12_section_order_advanced_read_only_deprecated_and_bounds::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_13_a_flattened_struct_s_keys_sit_beside_the_others {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct Inner { x: u8 } #[derive(::coqui_builder::Describe)] struct S { a: u8, #[map(flatten)] inner: Inner }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
    }
}

#[test]
fn case_13_a_flattened_struct_s_keys_sit_beside_the_others() {
    case_13_a_flattened_struct_s_keys_sit_beside_the_others::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_14_deny_unknown_fields {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(deny_unknown_fields)] struct S { a: u8 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["a"]);
        assert!(group_of(&d).deny_unknown_fields);
    }
}

#[test]
fn case_14_deny_unknown_fields() {
    case_14_deny_unknown_fields::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_15_a_transparent_struct_is_its_field {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(transparent)] struct S { value: u16 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "u16");
    }
}

#[test]
fn case_15_a_transparent_struct_is_its_field() {
    case_15_a_transparent_struct_is_its_field::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_16_a_newtype_is_its_field {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S(u16);

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "u16");
    }
}

#[test]
fn case_16_a_newtype_is_its_field() {
    case_16_a_newtype_is_its_field::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_17_a_generic_struct {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S<T> { value: T }

    pub fn check() {
        let d = <S::<u16> as ::coqui_builder::Describe>::description();
    }
}

#[test]
fn case_17_a_generic_struct() {
    case_17_a_generic_struct::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_18_unit_variants_are_a_choice {
    use super::*;

    #[derive(::coqui_builder::Describe)] enum E { Fast, SlowDown }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "E");
        assert_eq!(choices(&d), vec!["Fast", "SlowDown"]);
    }
}

#[test]
fn case_18_unit_variants_are_a_choice() {
    case_18_unit_variants_are_a_choice::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_19_rename_all_on_unit_variants {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(rename_all = "kebab-case")] enum E { Fast, SlowDown, H264 }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "E");
        assert_eq!(choices(&d), vec!["fast", "slow-down", "h264"]);
    }
}

#[test]
fn case_19_rename_all_on_unit_variants() {
    case_19_rename_all_on_unit_variants::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_20_a_variant_s_own_rename_and_label {
    use super::*;

    #[derive(::coqui_builder::Describe)] enum E { #[map(rename = "quick")] #[schema(label = "Quick")] Fast, Slow }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "E");
        assert_eq!(choices(&d), vec!["quick", "Slow"]);
    }
}

#[test]
fn case_20_a_variant_s_own_rename_and_label() {
    case_20_a_variant_s_own_rename_and_label::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_21_other_keeps_a_name_no_variant_has {
    use super::*;

    #[derive(::coqui_builder::Describe)] enum E { Fast, #[map(other)] Other(String) }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "E");
        assert_eq!(choices(&d), vec!["Fast"]);
    }
}

#[test]
fn case_21_other_keeps_a_name_no_variant_has() {
    case_21_other_keeps_a_name_no_variant_has::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_22_a_tagged_enum_has_arms {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(tag = "kind")] enum E { Password { user: String, #[schema(sensitive)] password: String }, KeyFile { path: String }, Token { #[schema(sensitive)] token: String }, Anonymous }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "variant");
        assert_eq!(arms(&d).0, "kind");
        assert_eq!(arms(&d).1, vec!["Password", "KeyFile", "Token", "Anonymous"]);
    }
}

#[test]
fn case_22_a_tagged_enum_has_arms() {
    case_22_a_tagged_enum_has_arms::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_23_rename_all_names_the_arms {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(tag = "kind", rename_all = "snake_case")] enum E { KeyFile { path: String }, NoAuth }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "variant");
        assert_eq!(arms(&d).0, "kind");
        assert_eq!(arms(&d).1, vec!["key_file", "no_auth"]);
    }
}

#[test]
fn case_23_rename_all_names_the_arms() {
    case_23_rename_all_names_the_arms::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_24_a_newtype_arm_holds_a_type_whose_keys_sit_beside_the_tag {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct Pw { user: String } #[derive(::coqui_builder::Describe)] #[map(tag = "kind")] enum E { Pw(Pw), None }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "variant");
        assert_eq!(arms(&d).0, "kind");
        assert_eq!(arms(&d).1, vec!["Pw", "None"]);
    }
}

#[test]
fn case_24_a_newtype_arm_holds_a_type_whose_keys_sit_beside_the_tag() {
    case_24_a_newtype_arm_holds_a_type_whose_keys_sit_beside_the_tag::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_25_an_untagged_enum_is_told_apart_by_the_kind_of_value {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(untagged)] enum E { Auto, Number(u16) }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "E");
    }
}

#[test]
fn case_25_an_untagged_enum_is_told_apart_by_the_kind_of_value() {
    case_25_an_untagged_enum_is_told_apart_by_the_kind_of_value::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_26_as_text_writes_with_display_and_reads_with_fromstr {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[map(as_text)] addr: std::net::Ipv4Addr }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["addr"]);
        assert_eq!(required(&d), vec!["addr"]);
        assert_eq!(kind_name(&field(&d, "addr").description), "leaf");
    }
}

#[test]
fn case_26_as_text_writes_with_display_and_reads_with_fromstr() {
    case_26_as_text_writes_with_display_and_reads_with_fromstr::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_27_as_text_on_a_list_is_per_item {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[map(default, as_text)] addrs: Vec<std::net::Ipv4Addr> }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["addrs"]);
        assert_eq!(required(&d), Vec::<&str>::new());
        assert_eq!(kind_name(&field(&d, "addrs").description), "list");
    }
}

#[test]
fn case_27_as_text_on_a_list_is_per_item() {
    case_27_as_text_on_a_list_is_per_item::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_28_as_text_on_an_optional_list_is_per_item {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[map(as_text)] addrs: Option<Vec<std::net::Ipv4Addr>> }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["addrs"]);
        assert_eq!(required(&d), Vec::<&str>::new());
        assert_eq!(kind_name(&field(&d, "addrs").description), "list");
    }
}

#[test]
fn case_28_as_text_on_an_optional_list_is_per_item() {
    case_28_as_text_on_an_optional_list_is_per_item::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_29_with_names_a_module_that_converts_the_field_a_list_of_protocol_names {
    use super::*;

    mod alpn_list { use coqui_builder::describe::LeafError; pub const TYPE: &str = "AlpnList"; pub fn to_text(items: &[String]) -> String { items.join(",") } pub fn from_text(text: &str) -> Result<Vec<String>, LeafError> { Ok(text.split(",").filter(|s| !s.is_empty()).map(String::from).collect()) } } #[derive(::coqui_builder::Describe)] struct S { #[map(default, with = alpn_list)] alpn: Vec<String> }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["alpn"]);
        assert_eq!(required(&d), Vec::<&str>::new());
        assert_eq!(kind_name(&field(&d, "alpn").description), "leaf");
    }
}

#[test]
fn case_29_with_names_a_module_that_converts_the_field_a_list_of_protocol_names() {
    case_29_with_names_a_module_that_converts_the_field_a_list_of_protocol_names::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_30_with_names_a_module_that_converts_bytes_held_as_text_a_protocol_name {
    use super::*;

    mod alpn_text { use coqui_builder::describe::LeafError; pub const TYPE: &str = "Alpn"; pub fn to_text(alpn: &[u8]) -> String { String::from_utf8_lossy(alpn).into_owned() } pub fn from_text(text: &str) -> Result<Vec<u8>, LeafError> { Ok(text.as_bytes().to_vec()) } } #[derive(::coqui_builder::Describe)] struct S { #[map(default, with = alpn_text)] alpn: Vec<u8> }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["alpn"]);
        assert_eq!(required(&d), Vec::<&str>::new());
        assert_eq!(kind_name(&field(&d, "alpn").description), "leaf");
    }
}

#[test]
fn case_30_with_names_a_module_that_converts_bytes_held_as_text_a_protocol_name() {
    case_30_with_names_a_module_that_converts_bytes_held_as_text_a_protocol_name::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_31_an_option_with_a_written_none {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[map(default = Some(std::time::Duration::from_secs(30)), none = "never")] read_timeout: Option<std::time::Duration> }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["read_timeout"]);
        assert_eq!(required(&d), Vec::<&str>::new());
        assert_eq!(default_of(&d, "read_timeout"), json!("30s"));
        assert_eq!(field(&d, "read_timeout").none.as_deref(), Some("never"));
    }
}

#[test]
fn case_31_an_option_with_a_written_none() {
    case_31_an_option_with_a_written_none::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_32_crate_names_where_the_derive_s_library_is_re_exported {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(crate = guatiao)] #[builder(crate = coqui_builder)] struct S { a: u8 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["a"]);
    }
}

#[test]
fn case_32_crate_names_where_the_derive_s_library_is_re_exported() {
    case_32_crate_names_where_the_derive_s_library_is_re_exported::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_33_crate_on_an_enum {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(crate = guatiao)] enum E { A, B }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "leaf");
        assert_eq!(leaf_of(&d).ty(), "E");
        assert_eq!(choices(&d), vec!["A", "B"]);
    }
}

#[test]
fn case_33_crate_on_an_enum() {
    case_33_crate_on_an_enum::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_34_arm_title_and_description {
    use super::*;

    #[derive(::coqui_builder::Describe)] #[map(tag = "kind")] enum E { #[schema(title = "Password", description = "A user and a password.")] Pw { user: String }, None }

    pub fn check() {
        let d = <E as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "variant");
        assert_eq!(arms(&d).0, "kind");
        assert_eq!(arms(&d).1, vec!["Pw", "None"]);
    }
}

#[test]
fn case_34_arm_title_and_description() {
    case_34_arm_title_and_description::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_36_a_reserved_setter_name_is_renamed_and_the_key_stays {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[builder(setter = "fill_level")] fill: bool }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["fill"]);
        assert_eq!(required(&d), vec!["fill"]);
    }
}

#[test]
fn case_36_a_reserved_setter_name_is_renamed_and_the_key_stays() {
    case_36_a_reserved_setter_name_is_renamed_and_the_key_stays::check();
}

#[rustfmt::skip]
#[allow(dead_code, unused)]
mod case_37_stricter_says_which_way_a_setting_gets_stricter {
    use super::*;

    #[derive(::coqui_builder::Describe)] struct S { #[builder(stricter = "lower")] timeout: u32, #[builder(stricter = "higher")] attempts: u32 }

    pub fn check() {
        let d = <S as ::coqui_builder::Describe>::description();
        assert_eq!(kind_name(&d), "group");
        assert_eq!(keys(&d), vec!["timeout", "attempts"]);
        assert_eq!(field(&d, "attempts").description.notes().stricter, Some(Stricter::Higher));
        assert_eq!(field(&d, "timeout").description.notes().stricter, Some(Stricter::Lower));
    }
}

#[test]
fn case_37_stricter_says_which_way_a_setting_gets_stricter() {
    case_37_stricter_says_which_way_a_setting_gets_stricter::check();
}

// END GENERATED
