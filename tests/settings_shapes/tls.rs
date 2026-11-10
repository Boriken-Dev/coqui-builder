// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A transport-security policy: lists of values written as text per item, optional lists, a
//! digest list, nested groups, an optional rule list behind a `cfg`, and the settings of a
//! pairing exchange with an optional timeout and a byte string held as text.

use core::fmt;
use core::str::FromStr;
use core::time::Duration;

use coqui_builder::Describe;

use super::acl::Acl;
use super::store::{Digest, WaitingLimits};

/// A refusal of a policy value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError(pub String);

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for PolicyError {}

/// A TLS protocol version. Only 1.2 and 1.3 can be offered; asking for another is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TlsVersion {
    /// TLS 1.2, text `"1.2"`.
    V1_2,
    /// TLS 1.3, text `"1.3"`.
    V1_3,
}

impl fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::V1_2 => "1.2",
            Self::V1_3 => "1.3",
        })
    }
}

impl FromStr for TlsVersion {
    type Err = PolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "1.2" => Ok(Self::V1_2),
            "1.3" => Ok(Self::V1_3),
            "1.0" | "1.1" | "ssl3" | "SSL3" => {
                Err(PolicyError("TLS before 1.2 is not supported".to_owned()))
            }
            _ => Err(PolicyError(
                "unknown TLS version; supported: \"1.2\", \"1.3\"".to_owned(),
            )),
        }
    }
}

/// A set of algorithms a policy allows.
///
/// Text form: `mozilla`, `fips-140-3-compliant`, `cnsa-1.0`, `cnsa-2.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CipherProfile {
    /// `mozilla`: the intermediate server-side guideline. The default.
    Mozilla,
    /// `fips-140-3-compliant`: the approved algorithms only.
    Fips1403Compliant,
    /// `cnsa-1.0`.
    Cnsa10,
    /// `cnsa-2.0`.
    Cnsa20,
}

impl fmt::Display for CipherProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Mozilla => "mozilla",
            Self::Fips1403Compliant => "fips-140-3-compliant",
            Self::Cnsa10 => "cnsa-1.0",
            Self::Cnsa20 => "cnsa-2.0",
        })
    }
}

impl FromStr for CipherProfile {
    type Err = PolicyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim() {
            "mozilla" => Ok(Self::Mozilla),
            "fips-140-3-compliant" => Ok(Self::Fips1403Compliant),
            "cnsa-1.0" => Ok(Self::Cnsa10),
            "cnsa-2.0" => Ok(Self::Cnsa20),
            _ => Err(PolicyError("unknown cipher profile".to_owned())),
        }
    }
}

/// The crypto provider a policy's configurations run on. Text form: `ring`, `aws-lc`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Crypto {
    /// `ring`. The default.
    #[default]
    Ring,
    /// `aws-lc`.
    AwsLc,
}

impl fmt::Display for Crypto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ring => "ring",
            Self::AwsLc => "aws-lc",
        })
    }
}

impl FromStr for Crypto {
    type Err = PolicyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim() {
            "ring" => Ok(Self::Ring),
            "aws-lc" => Ok(Self::AwsLc),
            _ => Err(PolicyError(
                "unknown crypto provider; known: ring, aws-lc".to_owned(),
            )),
        }
    }
}

/// The longest application protocol name, in bytes.
const ALPN_MAX: usize = 255;

/// An application protocol name of 1 to 255 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, coqui_builder::Leaf)]
pub struct Alpn(String);

impl Alpn {
    /// The name as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Display for Alpn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Alpn {
    type Err = PolicyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() || text.len() > ALPN_MAX {
            return Err(PolicyError("a protocol name of 1 to 255 bytes".to_owned()));
        }
        Ok(Alpn(text.to_owned()))
    }
}

/// What a TLS configuration and a trust decision are allowed to do.
///
/// Start from [`Policy::modern`], [`Policy::intermediate`], [`Policy::legacy`] or a policy of
/// your own, then change any field. [`Default`] is intermediate.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct Policy {
    /// TLS versions offered and accepted. Non-empty.
    #[map(default = base().tls_versions, as_text)]
    pub tls_versions: Vec<TlsVersion>,
    /// An allow-list of cipher suites by IANA name. `None` allows every suite the cipher
    /// profiles give.
    pub cipher_suites: Option<Vec<String>>,
    /// An allow-list of key-exchange groups by name; `None` allows all the profiles give.
    pub kx_groups: Option<Vec<String>>,
    /// The algorithm profiles allowed. Every preset allows `[CipherProfile::Mozilla]`.
    #[map(default = base().cipher_profiles, as_text)]
    pub cipher_profiles: Vec<CipherProfile>,
    /// The crypto provider; `Crypto::Ring` in every preset.
    #[map(default = base().crypto, as_text)]
    pub crypto: Crypto,
    /// Digests a pin may use.
    #[map(default = base().pin_digests)]
    pub pin_digests: Vec<Digest>,
    /// Whether a pinned certificate must be inside its validity dates.
    #[map(default = base().check_validity)]
    pub check_validity: bool,
    /// Whether a pinned peer may present a chain; strict refuses one.
    #[map(default = base().allow_chain_when_pinned)]
    pub allow_chain_when_pinned: bool,
    /// Whether TLS sessions may be resumed. A resumed handshake skips the certificate
    /// verifiers. Every preset leaves it off.
    #[map(default = base().allow_resumption)]
    pub allow_resumption: bool,
    /// Whether a TLS 1.2 connection must use the extended master secret.
    #[map(default = base().require_ems)]
    pub require_ems: bool,
    /// The bounds of a store-backed pin set's waiting queue: entries per source, entries in
    /// all, and how long before the same certificate from the same source is recorded again.
    #[map(default = base().waiting)]
    pub waiting: WaitingLimits,
    /// An ordered access control list for connecting clients. `None` for none.
    #[allow(clippy::non_minimal_cfg)]
    #[cfg(all())]
    pub acl: Option<Acl>,
}

/// The same policy as built without the access control list: its field is behind a `cfg` that
/// is false.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
pub struct PolicyWithoutAcl {
    /// TLS versions offered and accepted. Non-empty.
    #[map(default = base().tls_versions, as_text)]
    pub tls_versions: Vec<TlsVersion>,
    /// The waiting queue's bounds.
    #[map(default = base().waiting)]
    pub waiting: WaitingLimits,
    /// An ordered access control list for connecting clients. `None` for none.
    #[cfg(any())]
    pub acl: Option<Acl>,
}

fn base() -> Policy {
    Policy::intermediate()
}

impl Default for Policy {
    fn default() -> Self {
        Self::intermediate()
    }
}

impl Policy {
    /// The intermediate preset, the default: TLS 1.2 and 1.3; pins SHA-256, SHA-384 or SHA-512;
    /// validity dates checked; no chain on a pinned path; no session resumption.
    pub fn intermediate() -> Self {
        Self {
            tls_versions: vec![TlsVersion::V1_2, TlsVersion::V1_3],
            cipher_suites: None,
            kx_groups: None,
            cipher_profiles: vec![CipherProfile::Mozilla],
            crypto: Crypto::Ring,
            pin_digests: vec![Digest::Sha256, Digest::Sha384, Digest::Sha512],
            check_validity: true,
            allow_chain_when_pinned: false,
            allow_resumption: false,
            require_ems: true,
            waiting: WaitingLimits::default(),
            #[allow(clippy::non_minimal_cfg)]
            #[cfg(all())]
            acl: None,
        }
    }

    /// The modern preset: intermediate with TLS 1.3 only.
    pub fn modern() -> Self {
        Self {
            tls_versions: vec![TlsVersion::V1_3],
            ..Self::intermediate()
        }
    }

    /// The legacy preset: intermediate, and SHA-1 pins and TLS 1.2 without the extended master
    /// secret accepted, for interoperability with old peers only.
    pub fn legacy() -> Self {
        let mut policy = Self::intermediate();
        policy.pin_digests.push(Digest::Sha1);
        policy.require_ems = false;
        policy
    }
}

/// The settings of the configuration builders: the application protocols and the policy.
///
/// Identities, trust and verifiers are not settings: they hold keys and code, so a builder
/// takes them as inputs. Every key is optional; an absent one is the default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct TlsSettings {
    /// The application protocols offered, most preferred first, as text (`h2`, `app/1`),
    /// each 1 to 255 bytes.
    #[map(default)]
    pub alpn: Vec<Alpn>,
    /// The policy configurations are built under.
    #[map(default)]
    pub policy: Policy,
}

impl TlsSettings {
    /// The protocols as the bytes a configuration carries.
    pub fn alpn_bytes(&self) -> Vec<Vec<u8>> {
        self.alpn
            .iter()
            .map(|name| name.as_bytes().to_vec())
            .collect()
    }
}

/// The default ALPN of a pairing connection.
pub const PAIR_ALPN: &[u8] = b"example-pair/1";

/// `#[map(with = ..)]` for the pairing ALPN, bytes held as text of 1 to 255 bytes.
pub mod alpn_text {
    use coqui_builder::describe::LeafError;

    /// The name a datum shows.
    pub const TYPE: &str = "Alpn";

    /// The bytes as text: a byte outside `!` to `~`, and `%`, is written `%XX`, so every byte
    /// string reads back as itself and a name such as `h2` is its own text.
    pub fn to_text(alpn: &[u8]) -> String {
        let mut text = String::new();
        for &byte in alpn {
            if (0x21..=0x7e).contains(&byte) && byte != b'%' {
                text.push(char::from(byte));
            } else {
                text.push_str(&format!("%{byte:02X}"));
            }
        }
        text
    }

    /// The text as bytes, 1 to 255 of them: `%XX` is a byte, any other character is its UTF-8.
    pub fn from_text(text: &str) -> Result<Vec<u8>, LeafError> {
        let refused = || LeafError::Refused("a protocol name of 1 to 255 bytes".to_owned());
        let digit = |d: u8| char::from(d).to_digit(16).ok_or_else(refused);
        let mut bytes = Vec::new();
        let mut rest = text.as_bytes();
        while let Some((&first, tail)) = rest.split_first() {
            if first == b'%' {
                let [high, low, tail @ ..] = tail else {
                    return Err(refused());
                };
                bytes.push(u8::try_from(digit(*high)? * 16 + digit(*low)?).map_err(|_| refused())?);
                rest = tail;
            } else {
                bytes.push(first);
                rest = tail;
            }
        }
        if bytes.is_empty() || bytes.len() > super::ALPN_MAX {
            return Err(refused());
        }
        Ok(bytes)
    }
}

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The TLS side of pairing: the device's own policy and the ALPN that marks a pairing
/// connection.
#[derive(Debug, Clone, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct PairConfig {
    /// The policy both pairing configurations are built with.
    #[map(default)]
    pub policy: Policy,
    /// The pairing ALPN; only a connection that negotiated it carries a pairing exchange.
    #[map(default = PAIR_ALPN.to_vec(), with = alpn_text)]
    pub alpn: Vec<u8>,
    /// How long one side's whole exchange may take, the handshake included. `never` waits
    /// forever. Default 15 minutes.
    #[map(default = Some(DEFAULT_TIMEOUT), none = "never")]
    pub timeout: Option<Duration>,
}

impl Default for PairConfig {
    fn default() -> Self {
        Self {
            policy: Policy::default(),
            alpn: PAIR_ALPN.to_vec(),
            timeout: Some(DEFAULT_TIMEOUT),
        }
    }
}
