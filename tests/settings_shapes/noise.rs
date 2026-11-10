// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Two settings types that share two keys, one of them with an optional timeout whose default
//! is `Some` and whose written `None` is `never`.

use core::time::Duration;

use coqui_builder::Describe;

/// What one key may carry in one direction by default.
pub const DEFAULT_KEY_LIMIT: u64 = 1 << 38;
/// How long a responder waits for the joiner's first message by default.
pub const DEFAULT_FIRST_MESSAGE_TIMEOUT: Duration = Duration::from_secs(60);

/// What a method takes whole through its settings.
///
/// Identities, trust and the random source are not settings: they hold keys and code, so the
/// builder takes them as inputs. Every key is optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct NoiseSettings {
    /// What one key may carry in one direction, each message counting its length and 32.
    /// There is no floor: a limit below what a message costs refuses that message.
    #[map(default = NoiseSettings::default().key_limit)]
    pub key_limit: u64,
    /// Whether this side changes its sending key by itself at the key limit, which it does by
    /// default.
    #[map(default = NoiseSettings::default().rekey)]
    pub rekey: bool,
}

impl Default for NoiseSettings {
    fn default() -> Self {
        Self {
            key_limit: DEFAULT_KEY_LIMIT,
            rekey: true,
        }
    }
}

/// What a pairing method takes whole through its settings: the [`NoiseSettings`] keys and how
/// long a responder waits for a joiner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct PairingSettings {
    /// As [`NoiseSettings::key_limit`].
    #[map(default = PairingSettings::default().key_limit)]
    pub key_limit: u64,
    /// As [`NoiseSettings::rekey`].
    #[map(default = PairingSettings::default().rekey)]
    pub rekey: bool,
    /// How long a responder waits for the joiner's first message. `never` leaves the
    /// transport's own timeout as the only bound.
    #[map(default = Some(DEFAULT_FIRST_MESSAGE_TIMEOUT), none = "never")]
    pub first_message_timeout: Option<Duration>,
}

impl Default for PairingSettings {
    fn default() -> Self {
        Self {
            key_limit: DEFAULT_KEY_LIMIT,
            rekey: true,
            first_message_timeout: Some(DEFAULT_FIRST_MESSAGE_TIMEOUT),
        }
    }
}
