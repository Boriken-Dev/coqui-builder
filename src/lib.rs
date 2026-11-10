// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The README's snippets use the derive, so they are tests only where it is on.
#![cfg_attr(feature = "derive", doc = include_str!("../README.md"))]
#![cfg_attr(
    not(feature = "derive"),
    doc = "Describe the settings a builder takes as data: fill, check, report."
)]
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]
#![warn(
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

// The derive's output names this crate by `::coqui_builder`, which must resolve inside it too.
extern crate self as coqui_builder;

pub mod check;
pub mod datum;
pub mod describe;
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub mod env;
pub mod fill;
pub mod inspect;
pub mod non_default;
pub mod partial;
pub mod path;
pub mod profile;
pub mod report;
pub mod source;

pub use check::{Check, Checks};
#[cfg(feature = "derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use coqui_builder_derive::{Describe, Leaf};
pub use datum::{Datum, Shown};
pub use describe::{Describe, Description, Leaf};
#[cfg(feature = "std")]
pub use env::Env;
pub use fill::{Fill, FillError};
pub use inspect::{Entries, Inspect};
pub use non_default::{NonDefault, differences, non_defaults};
pub use partial::{Missing, Partial};
pub use path::{KeyPath, PathError, Segment};
pub use profile::{BindError, Expect, Profile};
pub use report::{Finding, Reason, Report};
pub use source::{Source, SourceError, Unknown};

/// An untagged enum holds at most one arm that is not a single value, and that arm is not
/// another untagged enum with a structured arm; both fail when the enum is compiled.
#[cfg(all(doctest, feature = "derive"))]
mod untagged_arms {
    /// Text arms and one structured arm compile.
    ///
    /// ```
    /// use coqui_builder::Describe;
    /// #[derive(Describe)]
    /// struct Table { x: u8 }
    /// #[derive(Describe)]
    /// #[map(untagged)]
    /// enum Either { Off, Flag(bool), Table(Table) }
    /// ```
    ///
    /// A second structured arm does not.
    ///
    /// ```compile_fail,E0080
    /// use coqui_builder::Describe;
    /// #[derive(Describe)]
    /// struct One { x: u8 }
    /// #[derive(Describe)]
    /// struct Two { y: u8 }
    /// #[derive(Describe)]
    /// #[map(untagged)]
    /// enum Either { One(One), Two(Two) }
    /// ```
    ///
    /// Neither does a structured arm that is itself an untagged enum with one.
    ///
    /// ```compile_fail,E0080
    /// use coqui_builder::Describe;
    /// #[derive(Describe)]
    /// struct Table { x: u8 }
    /// #[derive(Describe)]
    /// #[map(untagged)]
    /// enum Inner { Off, Table(Table) }
    /// #[derive(Describe)]
    /// #[map(untagged)]
    /// enum Outer { Flag(bool), Inner(Inner) }
    /// ```
    ///
    /// An arm holds a value, so it is not an `Option`: the field that holds the enum is.
    ///
    /// ```compile_fail,E0080
    /// use coqui_builder::Describe;
    /// #[derive(Describe)]
    /// #[map(untagged)]
    /// enum Either { Off, Flag(Option<bool>) }
    /// ```
    ///
    /// An enum whose only arm is structured is that arm.
    ///
    /// ```compile_fail,E0080
    /// use coqui_builder::Describe;
    /// #[derive(Describe)]
    /// struct Table { x: u8 }
    /// #[derive(Describe)]
    /// #[map(untagged)]
    /// enum Either { Table(Table) }
    /// ```
    struct Rules;
}

/// What generated code names; not part of the API.
#[doc(hidden)]
pub mod __private {
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;

    use crate::datum::Datum;
    use crate::describe::Codec;
    use crate::inspect::Entries;
    use crate::path::Segment;

    /// A converted value, read at `rest`.
    pub fn at_one<T, C: Codec<T>>(value: &T, rest: &[Segment]) -> Option<Datum> {
        rest.is_empty()
            .then(|| Datum::new(C::TYPE, C::to_text(value)))
    }

    /// A converted `Option`, read at `rest`.
    pub fn at_opt<T, C: Codec<T>>(value: &Option<T>, rest: &[Segment]) -> Option<Datum> {
        at_one::<T, C>(value.as_ref()?, rest)
    }

    /// An item of a list of converted items, read at `rest`.
    pub fn at_list<T, C: Codec<T>>(value: &[T], rest: &[Segment]) -> Option<Datum> {
        let (first, tail) = rest.split_first()?;
        at_one::<T, C>(value.get(first.as_index()?)?, tail)
    }

    /// An item of an `Option` of a list of converted items, read at `rest`.
    pub fn at_opt_list<T, C: Codec<T>>(value: &Option<Vec<T>>, rest: &[Segment]) -> Option<Datum> {
        at_list::<T, C>(value.as_ref()?, rest)
    }

    /// The length of a list of converted items, asked at `rest`.
    pub fn entries_list<T>(value: &[T], rest: &[Segment]) -> Option<Entries> {
        rest.is_empty().then_some(Entries::Len(value.len()))
    }

    /// The length of an `Option` of a list, asked at `rest`.
    pub fn entries_opt_list<T>(value: &Option<Vec<T>>, rest: &[Segment]) -> Option<Entries> {
        entries_list(value.as_ref()?, rest)
    }

    /// The value of a single-value arm of an untagged enum that reads `text`.
    pub fn parse_arm<T: crate::Describe>(text: &str) -> Option<T> {
        use crate::fill::Fill;
        let mut fill = <T::Fill as Default>::default();
        fill.set(&[], text).ok()?;
        fill.build(&crate::path::KeyPath::root(), &mut Vec::new())
    }

    /// Forgets everything set in `fill`, keeping the defaults it was given.
    pub fn forget<T, F: crate::fill::Fill<T>>(fill: &mut F) {
        forget_at::<T, F>(fill, &mut Vec::new());
    }

    fn forget_at<T, F: crate::fill::Fill<T>>(fill: &mut F, path: &mut Vec<Segment>) {
        if fill.clear(path).is_ok() {
            return;
        }
        if let Some(Entries::Keys(keys)) = fill.entries(path) {
            for key in keys {
                path.push(Segment::Key(key));
                forget_at::<T, F>(fill, path);
                path.pop();
            }
        }
    }

    /// The text of a single-value arm of an untagged enum.
    pub fn arm_text<T: crate::Inspect>(value: &T) -> String {
        value
            .at(&[])
            .map(|d| String::from(d.text()))
            .unwrap_or_default()
    }
}

/// Marks a struct of named fields: a type that `#[map(flatten)]` can take the fields of.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no named fields to flatten",
    label = "only a struct with named fields can be flattened",
    note = "derive `Describe` on a struct of named fields, or drop `#[map(flatten)]`"
)]
pub trait Fields: Describe {}
