// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `#[builder(setters_on = XBuilder)]`: the setters as inherent methods of a builder in the
//! same crate, called from a module that never imports the settings' trait.

#![allow(missing_debug_implementations)]

mod settings {
    use coqui_builder::Describe;

    /// Held by two builders, one of them generic.
    #[derive(Describe, Debug, Clone, PartialEq)]
    #[builder(setters_on = crate::builders::PlainBuilder)]
    #[builder(setters_on = <C: crate::builders::Tag> crate::builders::TaggedBuilder<C>)]
    pub struct Shared {
        #[map(default = 1)]
        pub port: u16,
        #[map(default)]
        pub name: String,
        #[map(default)]
        pub ratio: f32,
        pub retries: Option<u32>,
    }
}

mod builders {
    use coqui_builder::Builder;

    use crate::settings::Shared;

    pub trait Tag {
        fn tag(&self) -> &'static str;
    }

    pub struct Plain(pub Shared);

    #[derive(Builder)]
    #[builder(build = Plain)]
    pub struct PlainParts {
        settings: Shared,
    }

    impl From<PlainParts> for Plain {
        fn from(parts: PlainParts) -> Plain {
            Plain(parts.settings)
        }
    }

    pub struct Tagged {
        pub tag: &'static str,
        pub settings: Shared,
    }

    #[derive(Builder)]
    #[builder(build = Tagged, entry = "new")]
    pub struct TaggedParts<C: Tag> {
        settings: Shared,
        #[builder(input)]
        tag: C,
    }

    impl<C: Tag> From<TaggedParts<C>> for Tagged {
        fn from(parts: TaggedParts<C>) -> Tagged {
            Tagged {
                tag: parts.tag.tag(),
                settings: parts.settings,
            }
        }
    }
}

use builders::{Plain, Tag, Tagged, TaggedBuilder};

struct Blue;

impl Tag for Blue {
    fn tag(&self) -> &'static str {
        "blue"
    }
}

/// Never imports `SharedSetters`: the calls resolve to the inherent methods.
mod caller {
    use super::{Blue, Plain, Tagged, TaggedBuilder};

    pub fn plain() -> Plain {
        Plain::builder()
            .port(8)
            .name("plain")
            .ratio(0.5)
            .retries(Some(3))
            .build()
            .unwrap()
    }

    pub fn tagged() -> Tagged {
        TaggedBuilder::new()
            .port(9)
            .tag(Blue)
            .name("tagged")
            .build()
            .unwrap()
    }
}

#[test]
fn a_builder_in_the_crate_takes_the_setters_with_no_import() {
    let plain = caller::plain();
    assert_eq!(plain.0.port, 8);
    assert_eq!(plain.0.name, "plain");
    assert_eq!(plain.0.ratio, 0.5);
    assert_eq!(plain.0.retries, Some(3));
}

#[test]
fn a_generic_builder_and_a_second_builder_share_one_settings_type() {
    let tagged = caller::tagged();
    assert_eq!(tagged.tag, "blue");
    assert_eq!(tagged.settings.port, 9);
    assert_eq!(tagged.settings.name, "tagged");
}

#[test]
fn the_trait_stays_for_a_caller_that_imports_it() {
    use settings::SharedSetters;

    let plain = SharedSetters::port(Plain::builder(), 3).build().unwrap();
    assert_eq!(plain.0.port, 3);
}
