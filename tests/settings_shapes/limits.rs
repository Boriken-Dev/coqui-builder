// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A bound that is a count or no bound at all, held in a newtype that is a leaf by hand, and a
//! settings type made of such bounds.

use core::fmt;

use coqui_builder::describe::LeafError;
use coqui_builder::{Describe, Leaf};

/// At most a count, or no bound at all.
///
/// As configuration it is a number, or `unbounded`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Limit(Option<u32>);

impl Limit {
    /// No bound.
    pub const UNBOUNDED: Self = Self(None);

    /// At most `count`.
    pub const fn at(count: u32) -> Self {
        Self(Some(count))
    }
}

impl fmt::Debug for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(most) => write!(f, "Limit({most})"),
            None => f.write_str("Limit(unbounded)"),
        }
    }
}

/// The text of a limit is its count, or `unbounded`. `Display` stays the human wording.
impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(most) => write!(f, "at most {most}"),
            None => f.write_str("unbounded"),
        }
    }
}

impl Leaf for Limit {
    const TYPE: &'static str = "Limit";

    fn to_text(&self) -> String {
        match self.0 {
            Some(most) => most.to_string(),
            None => "unbounded".to_owned(),
        }
    }

    fn from_text(text: &str) -> Result<Self, LeafError> {
        if text == "unbounded" {
            return Ok(Limit::UNBOUNDED);
        }
        text.parse().map(Limit::at).map_err(|_| LeafError::Invalid {
            type_name: Self::TYPE,
        })
    }
}

/// The bounds of a role authorizer. Each has a default the caller can change or lift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct RoleLimits {
    /// The most roles.
    #[map(default = Limit::at(256))]
    pub roles: Limit,
    /// The most actions one role grants.
    #[map(default = Limit::at(256))]
    pub actions_per_role: Limit,
    /// The most rules in all.
    #[map(default = Limit::at(10_000))]
    pub rules: Limit,
    /// The most kinds of scope.
    #[map(default = Limit::at(64))]
    pub scope_kinds: Limit,
    /// The longest name, in bytes.
    #[map(default = Limit::at(256))]
    pub name_bytes: Limit,
    /// The longest chain of ancestors.
    #[map(default = Limit::at(64))]
    pub ancestors: Limit,
}

impl Default for RoleLimits {
    fn default() -> Self {
        Self {
            roles: Limit::at(256),
            actions_per_role: Limit::at(256),
            rules: Limit::at(10_000),
            scope_kinds: Limit::at(64),
            name_bytes: Limit::at(256),
            ancestors: Limit::at(64),
        }
    }
}
