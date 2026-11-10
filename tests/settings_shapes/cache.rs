// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A setting written as a flag or as a table: an untagged enum of a text arm and a group.

use core::time::Duration;

use coqui_builder::Describe;

/// [`CacheOptions::ttl`] by default: 5 s.
pub const DEFAULT_TTL: Duration = Duration::from_secs(5);
/// [`CacheOptions::negative_ttl`] by default: 1 s.
pub const DEFAULT_NEGATIVE_TTL: Duration = Duration::from_secs(1);
/// [`CacheOptions::max_entries`] by default.
pub const DEFAULT_MAX_ENTRIES: u64 = 10_000;

/// How long answers are kept, and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(crate = ::coqui_builder)]
#[non_exhaustive]
pub struct CacheOptions {
    /// How long a person found or a grant is kept.
    #[map(default = DEFAULT_TTL)]
    pub ttl: Duration,
    /// How long "no such person" or a denial is kept.
    #[map(default = DEFAULT_NEGATIVE_TTL)]
    pub negative_ttl: Duration,
    /// The most answers kept of each kind; `0` keeps none.
    #[map(default = DEFAULT_MAX_ENTRIES)]
    pub max_entries: u64,
}

impl Default for CacheOptions {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_TTL,
            negative_ttl: DEFAULT_NEGATIVE_TTL,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }
}

/// A cache as configuration: `true` (the defaults), `false` (off), or [`CacheOptions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(crate = ::coqui_builder, untagged)]
pub enum CacheSetting {
    /// On with the defaults, or off.
    Enabled(bool),
    /// On with these options.
    Tuned(CacheOptions),
}

impl Default for CacheSetting {
    fn default() -> Self {
        Self::Enabled(true)
    }
}

/// A provider's settings: the cache is one key among others.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
pub struct ProviderSettings {
    /// The directory the provider reads.
    pub directory: String,
    /// The cache in front of the provider.
    #[map(default)]
    pub cache: CacheSetting,
}
