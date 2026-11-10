// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What a generated builder shares: the link to its settings that the settings' typed setters
//! are written against.

use crate::describe::Describe;
use crate::partial::Partial;

/// A builder that holds a [`Partial`] of the settings `S`.
///
/// `#[derive(Describe)]` on a settings type writes a trait of typed setters (`<S>Setters`)
/// that every `Holds<S>` has once the trait is in scope. Every builder `#[derive(Builder)]`
/// writes holds its settings' partial, and so does a hand-written one that implements this.
pub trait Holds<S: Describe> {
    /// The settings being filled.
    fn partial_mut(&mut self) -> &mut Partial<S>;
}
