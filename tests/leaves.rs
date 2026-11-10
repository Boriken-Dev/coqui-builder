// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Each shipped leaf: its type name, its text, the round trip and its order.

use core::cmp::Ordering;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

use coqui_builder::describe::{Kind, LeafError, LeafInfo, assert_round_trip};
use coqui_builder::{Datum, Describe, Leaf, Partial, Shown};

#[test]
fn every_shipped_leaf_reads_back_what_it_writes() {
    assert_round_trip(&[true, false]);
    assert_round_trip(&[i8::MIN, -1, 0, i8::MAX]);
    assert_round_trip(&[0u16, 80, u16::MAX]);
    assert_round_trip(&[i128::MIN, 0, i128::MAX]);
    assert_round_trip(&[0u128, u128::MAX]);
    assert_round_trip(&[0usize, usize::MAX]);
    assert_round_trip(&[
        0.0f64,
        -0.0,
        0.1,
        1e21,
        1e-7,
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
    ]);
    assert_round_trip(&[0.5f32, 16_777_217.0, f32::MAX]);
    assert_round_trip(&[String::new(), "a b\n".to_string(), "é".to_string()]);
    assert_round_trip(&['x', 'é']);
    assert_round_trip(&[Ipv4Addr::new(10, 0, 0, 1)]);
    assert_round_trip(&["::1".parse::<IpAddr>().unwrap()]);
    assert_round_trip(&["127.0.0.1:80".parse::<SocketAddr>().unwrap()]);
}

#[test]
fn a_leaf_names_its_type() {
    assert_eq!(<bool as Leaf>::TYPE, "bool");
    assert_eq!(<u16 as Leaf>::TYPE, "u16");
    assert_eq!(<u128 as Leaf>::TYPE, "u128");
    assert_eq!(<f64 as Leaf>::TYPE, "f64");
    assert_eq!(<String as Leaf>::TYPE, "String");
    assert_eq!(<core::time::Duration as Leaf>::TYPE, "Duration");
    assert_eq!(<SocketAddr as Leaf>::TYPE, "SocketAddr");
}

#[test]
fn a_boolean_reads_the_spellings_the_standard_names() {
    for yes in ["true", "1", "yes", "On", "TRUE"] {
        assert_eq!(bool::from_text(yes), Ok(true), "{yes}");
    }
    for no in ["false", "0", "no", "OFF"] {
        assert_eq!(bool::from_text(no), Ok(false), "{no}");
    }
    assert!(bool::from_text("maybe").is_err());
    assert_eq!(true.to_text(), "true");
}

#[test]
fn an_integer_is_held_to_its_type_and_says_which_refusal() {
    assert_eq!(u8::from_text("255"), Ok(255));
    assert_eq!(
        u8::from_text("256"),
        Err(LeafError::OutOfRange { type_name: "u8" })
    );
    assert!(u8::from_text("-1").is_err());
    assert_eq!(
        u32::from_text("1.5"),
        Err(LeafError::Invalid { type_name: "u32" })
    );
    assert!(u32::from_text("").is_err());
    assert_eq!(
        u128::from_text("340282366920938463463374607431768211455"),
        Ok(u128::MAX)
    );
}

#[test]
fn a_float_is_finite_and_written_shortest() {
    assert_eq!(f64::from_text("0.25"), Ok(0.25));
    assert_eq!(f64::from_text("1e3"), Ok(1000.0));
    assert_eq!(f64::from_text("inf"), Err(LeafError::NotFinite));
    assert_eq!(f64::from_text("NaN"), Err(LeafError::NotFinite));
    assert_eq!(f32::from_text("1e60"), Err(LeafError::NotFinite));
    assert_eq!(0.1f64.to_text(), "0.1");
    assert_eq!(0.5f32.to_text(), "0.5");
}

#[test]
fn numbers_and_durations_declare_an_order_and_text_does_not() {
    assert_eq!(u32::order(&1, &2), Some(Ordering::Less));
    assert_eq!(f64::order(&2.0, &1.5), Some(Ordering::Greater));
    assert_eq!(f64::order(&f64::NAN, &1.0), None);
    assert_eq!(
        core::time::Duration::order(
            &core::time::Duration::from_secs(1),
            &core::time::Duration::from_millis(1000)
        ),
        Some(Ordering::Equal)
    );
    const { assert!(<u32 as Leaf>::ORDERED && <f64 as Leaf>::ORDERED) };
    const { assert!(!<String as Leaf>::ORDERED && !<bool as Leaf>::ORDERED) };
    assert_eq!(String::order(&"a".to_string(), &"b".to_string()), None);
}

#[test]
fn a_char_is_one_character() {
    assert_eq!(char::from_text("é"), Ok('é'));
    assert!(char::from_text("ab").is_err());
    assert!(char::from_text("").is_err());
}

#[test]
fn an_address_is_text_in_its_standard_spelling() {
    assert_eq!(
        Ipv4Addr::from_text("10.0.0.1"),
        Ok(Ipv4Addr::new(10, 0, 0, 1))
    );
    assert!(Ipv4Addr::from_text("10.0.0").is_err());
    assert!(SocketAddr::from_text("80").is_err());
}

#[cfg(feature = "std")]
#[test]
fn a_path_is_text() {
    use std::path::PathBuf;
    assert_round_trip(&[PathBuf::from("a/b")]);
    assert_eq!(<PathBuf as Leaf>::TYPE, "Path");
}

#[test]
fn a_leaf_is_described_by_its_info_and_compares_typed_values() {
    let Kind::Leaf(info) = <u16 as Describe>::description().kind().clone() else {
        panic!("a leaf")
    };
    assert_eq!(info.ty(), "u16");
    assert!(info.is_ordered());
    assert_eq!(info.eq("0080", "80"), Ok(true));
    assert_eq!(info.order("9", "10"), Ok(Some(Ordering::Less)));
    assert!(info.check("70000").is_err());
    assert_eq!(info.choices(), None);
    assert_eq!(LeafInfo::of::<String>().order("a", "b"), Ok(None));
}

#[test]
fn a_datum_shows_its_type_and_its_text_and_masks_a_secret() {
    let d = Datum::new("Duration", "30s");
    assert_eq!(d.to_string(), "Duration(30s)");
    assert_eq!(d.clone().sensitive().to_string(), "Duration(***)");
    assert!(!format!("{:?}", d.clone().sensitive()).contains("30s"));
    assert_eq!((d.ty(), d.text()), ("Duration", "30s"));
    assert_eq!(Shown::Unset.to_string(), "unset");
    assert_eq!(Shown::One(d.clone()).to_string(), "Duration(30s)");
    assert_eq!(
        Shown::Items(vec![
            Datum::new("String", "1.2"),
            Datum::new("String", "1.3")
        ])
        .to_string(),
        "[String(1.2), String(1.3)]"
    );
    assert_eq!(
        Shown::Items(vec![Datum::new("String", "k")])
            .sensitive()
            .to_string(),
        "[String(***)]"
    );
}

#[test]
fn a_partial_of_a_leaf_sets_its_text() {
    let mut p = Partial::<u16>::new();
    assert!(p.set("", "80").is_err());
    p.set_at(&[], "80").unwrap();
    assert!(p.set_at(&[], "x").is_err());
    assert_eq!(p.build().unwrap(), 80);
}

#[test]
fn equality_parses_both_texts_and_never_compares_the_raw_input() {
    let duration = LeafInfo::of::<core::time::Duration>();
    assert_eq!(duration.eq("30s", "30000ms"), Ok(true));
    assert_eq!(duration.eq("30", "30s"), Ok(true));
    assert_eq!(duration.eq("30s", "31s"), Ok(false));
    let float = LeafInfo::of::<f64>();
    assert_eq!(float.eq("0.5", "0.50"), Ok(true));
    assert_eq!(float.eq("5e-1", "0.5"), Ok(true));
    // The override: a negative zero is the same setting as zero though its text differs.
    assert_ne!(<f64 as Leaf>::to_text(&-0.0), <f64 as Leaf>::to_text(&0.0));
    assert_eq!(float.eq("-0", "0"), Ok(true));
    assert!(f64::same(&-0.0, &0.0));
    // A text no spelling of the type is an error, not an inequality.
    assert!(float.eq("0.5", "half").is_err());
    let int = LeafInfo::of::<u16>();
    assert_eq!(int.eq("0080", "80"), Ok(true));
}
