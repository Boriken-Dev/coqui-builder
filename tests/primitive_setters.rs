// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A setter for a primitive scalar takes it as itself, so a bare literal infers; every other
//! type keeps `impl Into<T>`.

use coqui_builder::{Builder, Describe};

#[derive(Describe, Debug, Clone, PartialEq)]
struct Knobs {
    #[map(default = 1)]
    port: u16,
    #[map(default)]
    max: usize,
    #[map(default)]
    ratio: f32,
    #[map(default)]
    on: bool,
    #[map(default)]
    offset: i32,
    #[map(default)]
    initial: char,
    #[map(default)]
    name: String,
    retries: Option<u32>,
}

#[derive(Debug, PartialEq)]
struct Rig {
    knobs: Knobs,
    depth: u8,
    weight: f64,
    armed: bool,
    spare: Option<u32>,
}

#[derive(Builder)]
#[builder(build = Rig)]
struct RigParts {
    settings: Knobs,
    #[builder(input)]
    depth: u8,
    #[builder(input)]
    weight: f64,
    #[builder(input)]
    armed: bool,
    #[builder(input)]
    spare: Option<u32>,
}

impl From<RigParts> for Rig {
    fn from(parts: RigParts) -> Rig {
        Rig {
            knobs: parts.settings,
            depth: parts.depth,
            weight: parts.weight,
            armed: parts.armed,
            spare: parts.spare,
        }
    }
}

use coqui_builder::Partial;

#[test]
fn a_literal_sets_a_primitive_setting_with_no_suffix() {
    let rig = Rig::builder()
        .port(8)
        .max(8)
        .ratio(0.5)
        .on(true)
        .offset(-3)
        .initial('x')
        .name("text")
        .retries(Some(8))
        .depth(2)
        .weight(1.5)
        .armed(true)
        .build()
        .unwrap();
    assert_eq!(rig.knobs.port, 8);
    assert_eq!(rig.knobs.max, 8);
    assert_eq!(rig.knobs.ratio, 0.5);
    assert!(rig.knobs.on);
    assert_eq!(rig.knobs.offset, -3);
    assert_eq!(rig.knobs.initial, 'x');
    assert_eq!(rig.knobs.name, "text");
    assert_eq!(rig.knobs.retries, Some(8));
    assert_eq!((rig.depth, rig.weight, rig.armed), (2, 1.5, true));
    assert_eq!(rig.spare, None);
}

#[test]
fn an_option_setting_still_takes_the_inner_value_or_an_option() {
    let none = Rig::builder()
        .retries(None::<u32>)
        .depth(1)
        .weight(0.0)
        .armed(false)
        .build()
        .unwrap();
    assert_eq!((none.knobs.retries, none.spare), (None, None));
    let some = Rig::builder()
        .retries(8u32)
        .spare(5u32)
        .depth(1)
        .weight(0.0)
        .armed(false)
        .build()
        .unwrap();
    assert_eq!((some.knobs.retries, some.spare), (Some(8), Some(5)));
}

#[test]
fn a_string_setting_still_converts() {
    let rig = Rig::builder()
        .name(String::from("owned"))
        .depth(1)
        .weight(0.0)
        .armed(false)
        .build()
        .unwrap();
    assert_eq!(rig.knobs.name, "owned");
    let _: Partial<Knobs> = Partial::new();
}
