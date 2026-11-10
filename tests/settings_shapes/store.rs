// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A digest, the bounds of a waiting queue and the settings of a trust store: unit enums with a
//! rename, plain numbers and durations, nested groups, and a directory held as a required key.

use std::path::PathBuf;
use std::time::Duration;

use coqui_builder::Describe;

/// A digest algorithm. New algorithms add variants here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, coqui_builder::Leaf)]
#[map(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Digest {
    /// SHA-256, 32 bytes. The default.
    Sha256,
    /// SHA-384, 48 bytes.
    Sha384,
    /// SHA-512, 64 bytes.
    Sha512,
    /// SHA-1, 20 bytes. Legacy interoperability only.
    Sha1,
}

/// Bounds on the waiting queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct WaitingLimits {
    /// Entries one source may hold.
    #[map(default = WaitingLimits::default().per_source)]
    pub per_source: usize,
    /// Entries the whole directory may hold; past it nothing is recorded.
    #[map(default = WaitingLimits::default().total)]
    pub total: usize,
    /// How long the same peer from the same source is not recorded again.
    #[map(default = WaitingLimits::default().rerecord)]
    pub rerecord: Duration,
}

impl Default for WaitingLimits {
    fn default() -> Self {
        Self {
            per_source: 16,
            total: 1_000,
            rerecord: Duration::from_secs(60),
        }
    }
}

/// What a source at its limit does with a newcomer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, coqui_builder::Leaf)]
#[map(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum WhenFull {
    /// Refuses the newcomer and leaves those waiting alone.
    #[default]
    Refuse,
    /// Replaces the source's oldest entry.
    ReplaceOldest,
}

/// The least time between two new records from one source.
pub const DEFAULT_RECORD_INTERVAL: Duration = Duration::from_secs(1);

/// What a waiting queue is built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct WaitingSettings {
    /// The bounds on entries.
    #[map(default = WaitingSettings::default().limits)]
    pub limits: WaitingLimits,
    /// The IPv6 prefix length sources are grouped by: 64. 128 counts each address alone.
    #[map(default = WaitingSettings::default().ipv6_prefix)]
    pub ipv6_prefix: u8,
    /// What a source at its limit does with a newcomer.
    #[map(default = WaitingSettings::default().when_full)]
    pub when_full: WhenFull,
    /// The least time between two new records from one source; zero is no limit.
    #[map(default = WaitingSettings::default().record_interval)]
    pub record_interval: Duration,
}

impl Default for WaitingSettings {
    fn default() -> Self {
        Self {
            limits: WaitingLimits::default(),
            ipv6_prefix: 64,
            when_full: WhenFull::default(),
            record_interval: DEFAULT_RECORD_INTERVAL,
        }
    }
}

/// How an accepted peer's key is replaced by its successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct RotationOptions {
    /// How long the old key stays accepted once its successor is promoted. Default 24 hours.
    #[map(default = RotationOptions::default().retire_after)]
    pub retire_after: Duration,
    /// Whether a successor waits for an approval. Default `false`.
    #[map(default = RotationOptions::default().require_approval)]
    pub require_approval: bool,
    /// Whether a successor nobody proved may be promoted. Default `false`.
    #[map(default = RotationOptions::default().allow_unproven)]
    pub allow_unproven: bool,
}

impl Default for RotationOptions {
    fn default() -> Self {
        Self {
            retire_after: Duration::from_secs(24 * 3600),
            require_approval: false,
            allow_unproven: false,
        }
    }
}

/// What a trust store is built from: the directory it lives in, and its other settings.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct TrustStoreSettings {
    /// The directory the store keeps its peers under. Required.
    pub state_dir: PathBuf,
    /// The waiting queue: its limits, how sources are grouped and what a full source does.
    #[map(default = WaitingSettings::default())]
    pub waiting: WaitingSettings,
    /// How an accepted peer's key is replaced by its successor.
    #[map(default = RotationOptions::default())]
    pub rotation: RotationOptions,
}

/// The characters a pairing code draws from, and its length.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct CodeAlphabet {
    /// The characters, each at most once.
    pub chars: String,
    /// How many characters a code has.
    pub len: usize,
}

impl CodeAlphabet {
    /// The alphabet of a PIN: six digits.
    pub fn pin() -> Self {
        Self {
            chars: "0123456789".to_owned(),
            len: 6,
        }
    }

    /// The alphabet of a user code: eight consonants.
    pub fn user_code() -> Self {
        Self {
            chars: "BCDFGHJKLMNPQRSTVWXZ".to_owned(),
            len: 8,
        }
    }
}

/// Which pairing methods are open, and how long a code lives.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct PairingOptions {
    /// Whether joiners may pair with an invitation. Default `true`.
    #[map(default = PairingOptions::default().invitation)]
    pub invitation: bool,
    /// Whether joiners may pair with a PIN. Default `true`.
    #[map(default = PairingOptions::default().pin)]
    pub pin: bool,
    /// Whether a joiner may show a PIN for an operator to type into this side, which holds its
    /// run until the operator answers or `pin_ttl` passes. Turn it off where nobody is there
    /// to answer. Default `true`.
    #[map(default = PairingOptions::default().pin_shown)]
    pub pin_shown: bool,
    /// Whether joiners may pair by the device flow. Default `true`.
    #[map(default = PairingOptions::default().device)]
    pub device: bool,
    /// The alphabet and length of a PIN the desk shows. Default `0123456789` x 6.
    #[map(default = PairingOptions::default().pin_alphabet)]
    pub pin_alphabet: CodeAlphabet,
    /// How long a shown PIN, or a slot waiting for a joiner's PIN, lasts; `never` lasts for
    /// good. Default 10 minutes.
    #[map(default = PairingOptions::default().pin_ttl, none = "never")]
    pub pin_ttl: Option<Duration>,
    /// How many runs may try a shown PIN before it is burned. Default 1.
    #[map(default = PairingOptions::default().pin_attempts)]
    pub pin_attempts: u32,
    /// The alphabet and length of a device-flow user code. Default `BCDFGHJKLMNPQRSTVWXZ` x 8.
    #[map(default = PairingOptions::default().user_code_alphabet)]
    pub user_code_alphabet: CodeAlphabet,
    /// How long a user code waits for approval; `never` waits for good. Default 10 minutes.
    #[map(default = PairingOptions::default().device_ttl, none = "never")]
    pub device_ttl: Option<Duration>,
}

impl Default for PairingOptions {
    fn default() -> Self {
        Self {
            invitation: true,
            pin: true,
            pin_shown: true,
            device: true,
            pin_alphabet: CodeAlphabet::pin(),
            pin_ttl: Some(Duration::from_secs(600)),
            pin_attempts: 1,
            user_code_alphabet: CodeAlphabet::user_code(),
            device_ttl: Some(Duration::from_secs(600)),
        }
    }
}

/// The bounds that keep strangers from filling a pairing desk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct DeskLimits {
    /// Pairing exchanges in progress at once. Default 32.
    #[map(default = DeskLimits::default().pending)]
    pub pending: usize,
    /// Pairing exchanges in progress at once from one source. Default 4.
    #[map(default = DeskLimits::default().pending_per_source)]
    pub pending_per_source: usize,
    /// PIN runs one source may start per PIN lifetime. Default 8.
    #[map(default = DeskLimits::default().pin_runs_per_source)]
    pub pin_runs_per_source: usize,
    /// Wrong user codes one approver may enter per `approver_window`. Default 5.
    #[map(default = DeskLimits::default().wrong_codes_per_approver)]
    pub wrong_codes_per_approver: usize,
    /// The window of `wrong_codes_per_approver`. Default 1 minute.
    #[map(default = DeskLimits::default().approver_window)]
    pub approver_window: Duration,
    /// Wrong user codes all approvers together may enter per user-code lifetime. Default 60.
    #[map(default = DeskLimits::default().wrong_codes_total)]
    pub wrong_codes_total: usize,
    /// The IPv6 prefix length sources are grouped by. Default 64.
    #[map(default = DeskLimits::default().ipv6_prefix)]
    pub ipv6_prefix: u8,
}

impl Default for DeskLimits {
    fn default() -> Self {
        Self {
            pending: 32,
            pending_per_source: 4,
            pin_runs_per_source: 8,
            wrong_codes_per_approver: 5,
            approver_window: Duration::from_secs(60),
            wrong_codes_total: 60,
            ipv6_prefix: 64,
        }
    }
}

/// The settings of a pairing desk: the options, and the limits that bound strangers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct PairingDeskSettings {
    /// Which methods are open and how long a code lives.
    #[map(default = PairingDeskSettings::default().options)]
    pub options: PairingOptions,
    /// The bounds that keep strangers from filling the desk.
    #[map(default = PairingDeskSettings::default().limits)]
    pub limits: DeskLimits,
}
