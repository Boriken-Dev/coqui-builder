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

pub mod datum;
pub mod path;

pub use datum::{Datum, Shown};
pub use path::{KeyPath, PathError, Segment};

/// What generated code names; not part of the API.
