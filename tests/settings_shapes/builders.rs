// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Builders over the settings of the other modules, written as another crate would write them:
//! the settings' setters come from importing their traits, the built types are foreign to the
//! settings and some are generic, and a hand-written method sets a key below the top level.

use core::fmt;
use std::path::PathBuf;
use std::time::Duration;

use coqui_builder::builder::{BuildError, Holds};
use coqui_builder::{Builder, Configured};

use super::pipe::ListenerSettings;
use super::store::{TrustStoreSettings, WaitingLimits, WhenFull};
use super::tls::{PolicyError, TlsSettings};

/// A certificate and its key, borrowed by a builder for as long as it builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity(pub String);

/// How a server treats client certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientAuth {
    /// No client certificate.
    None,
    /// A certificate that a trust accepts.
    Required(Trust),
}

/// What decides whether a certificate is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trust(pub String);

/// How a client verifies the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerAuth {
    /// A trust.
    Trust(Trust),
}

impl From<Trust> for ServerAuth {
    fn from(trust: Trust) -> ServerAuth {
        ServerAuth::Trust(trust)
    }
}

/// A configuration the builders make: here a stand-in for a foreign type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub identity: String,
    pub client_auth: ClientAuth,
    pub alpn: Vec<Vec<u8>>,
    pub tls_versions: usize,
}

/// The client side of [`ServerConfig`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub server: ServerAuth,
    pub identity: Option<String>,
    pub alpn: Vec<Vec<u8>>,
    pub require_ems: bool,
}

/// A configuration together with the settings it was built from.
#[derive(Debug, Clone, PartialEq, Eq, Configured)]
pub struct Built<T> {
    pub config: T,
    #[configured]
    pub settings: TlsSettings,
}

/// The builders' own refusal.
#[derive(Debug, PartialEq, Eq)]
pub enum PeerError {
    Policy(String),
    Io(String),
}

impl fmt::Display for PeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeerError::Policy(text) => write!(f, "policy: {text}"),
            PeerError::Io(text) => write!(f, "io: {text}"),
        }
    }
}

impl core::error::Error for PeerError {}

impl From<PolicyError> for PeerError {
    fn from(error: PolicyError) -> PeerError {
        PeerError::Policy(error.0)
    }
}

fn checked(settings: &TlsSettings) -> Result<(), PeerError> {
    if settings.policy.tls_versions.is_empty() {
        return Err(PeerError::Policy("no TLS version is allowed".to_owned()));
    }
    Ok(())
}

// ---- a server: a borrowed identity and a required client-auth input ----

#[derive(Builder, Debug)]
#[builder(build = Built<ServerConfig>, name = "ServerBuilder", entry = "new", fallible)]
pub struct ServerParts<'a> {
    settings: TlsSettings,
    #[builder(input)]
    identity: &'a Identity,
    #[builder(input)]
    client_auth: ClientAuth,
}

impl TryFrom<ServerParts<'_>> for Built<ServerConfig> {
    type Error = PeerError;

    fn try_from(parts: ServerParts<'_>) -> Result<Self, PeerError> {
        checked(&parts.settings)?;
        Ok(Built {
            config: ServerConfig {
                identity: parts.identity.0.clone(),
                client_auth: parts.client_auth,
                alpn: parts.settings.alpn_bytes(),
                tls_versions: parts.settings.policy.tls_versions.len(),
            },
            settings: parts.settings,
        })
    }
}

impl ServerBuilder<'_> {
    /// A second terminal over the parts: the server's configuration and the client's that
    /// trusts it, from the one set of settings.
    pub fn configs(self) -> Result<(ServerConfig, ClientConfig), BuildError<PeerError>> {
        let parts = self.parts().map_err(BuildError::Missing)?;
        checked(&parts.settings).map_err(BuildError::Failed)?;
        let alpn = parts.settings.alpn_bytes();
        let server = ServerConfig {
            identity: parts.identity.0.clone(),
            client_auth: parts.client_auth,
            alpn: alpn.clone(),
            tls_versions: parts.settings.policy.tls_versions.len(),
        };
        let client = ClientConfig {
            server: ServerAuth::Trust(Trust(parts.identity.0.clone())),
            identity: None,
            alpn,
            require_ems: parts.settings.policy.require_ems,
        };
        Ok((server, client))
    }
}

// ---- a client: a required server input and an optional borrowed identity ----

#[derive(Builder, Debug)]
#[builder(build = Built<ClientConfig>, name = "ClientBuilder", entry = "new", fallible)]
pub struct ClientParts<'a> {
    settings: TlsSettings,
    #[builder(input)]
    server: ServerAuth,
    #[builder(input)]
    identity: Option<&'a Identity>,
}

impl TryFrom<ClientParts<'_>> for Built<ClientConfig> {
    type Error = PeerError;

    fn try_from(parts: ClientParts<'_>) -> Result<Self, PeerError> {
        checked(&parts.settings)?;
        Ok(Built {
            config: ClientConfig {
                server: parts.server,
                identity: parts.identity.map(|identity| identity.0.clone()),
                alpn: parts.settings.alpn_bytes(),
                require_ems: parts.settings.policy.require_ems,
            },
            settings: parts.settings,
        })
    }
}

// ---- a trust store: a required directory key, an optional input, I/O in build ----

/// The clock expiry reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clock(pub &'static str);

impl Clock {
    pub fn system() -> Clock {
        Clock("system")
    }
}

#[derive(Debug, PartialEq, Eq, Configured)]
pub struct TrustStore {
    pub root: PathBuf,
    #[configured]
    pub settings: TrustStoreSettings,
    pub clock: Clock,
}

#[derive(Builder, Debug)]
#[builder(build = TrustStore, fallible)]
pub struct TrustStoreParts {
    settings: TrustStoreSettings,
    #[builder(input)]
    clock: Option<Clock>,
}

impl TryFrom<TrustStoreParts> for TrustStore {
    type Error = PeerError;

    fn try_from(parts: TrustStoreParts) -> Result<TrustStore, PeerError> {
        let root = parts.settings.state_dir.join("peers");
        std::fs::create_dir_all(root.join("accepted"))
            .map_err(|error| PeerError::Io(error.kind().to_string()))?;
        Ok(TrustStore {
            root,
            clock: parts.clock.unwrap_or_else(Clock::system),
            settings: parts.settings,
        })
    }
}

impl TrustStoreBuilder {
    /// The bounds on the waiting queue.
    pub fn limits(mut self, limits: WaitingLimits) -> Self {
        self.partial_mut()
            .put("waiting.limits", limits)
            .expect("waiting.limits is a key of the settings");
        self
    }

    /// Group IPv6 sources by their first `bits` bits.
    pub fn ipv6_prefix(mut self, bits: u8) -> Self {
        self.partial_mut()
            .put("waiting.ipv6_prefix", bits)
            .expect("waiting.ipv6_prefix is a key of the settings");
        self
    }

    /// What a newcomer to a full source gets.
    pub fn when_full(mut self, when_full: WhenFull) -> Self {
        self.partial_mut()
            .put("waiting.when_full", when_full)
            .expect("waiting.when_full is a key of the settings");
        self
    }

    /// Record a new peer from one source at most once per `interval`.
    pub fn record_interval(mut self, interval: Duration) -> Self {
        self.partial_mut()
            .put("waiting.record_interval", interval)
            .expect("waiting.record_interval is a key of the settings");
        self
    }
}

// ---- a listener: a required input that is not a setting ----

/// Where a listener listens. Its text does not parse back for another host's pipe, so it is an
/// input, never a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeName(pub String);

impl From<&PipeName> for PipeName {
    fn from(name: &PipeName) -> PipeName {
        name.clone()
    }
}

#[derive(Debug, PartialEq, Eq, Configured)]
pub struct Listener {
    pub name: PipeName,
    #[configured]
    pub settings: ListenerSettings,
}

#[derive(Builder, Debug)]
#[builder(build = Listener)]
pub struct ListenerParts {
    settings: ListenerSettings,
    #[builder(input)]
    name: PipeName,
}

impl From<ListenerParts> for Listener {
    fn from(parts: ListenerParts) -> Listener {
        Listener {
            name: parts.name,
            settings: parts.settings,
        }
    }
}

// ---- a channel over a stream: generic inputs, and the terminal step takes the stream ----

use super::transport::stream::StreamChannelSettings;

/// How messages are framed on a stream.
pub trait Codec {
    fn name(&self) -> &'static str;
}

/// A stream that splits into a reading and a writing half.
pub trait Split {
    fn split(self) -> std::io::Result<(String, String)>;
}

#[derive(Debug, PartialEq, Eq, Configured)]
pub struct StreamChannel {
    pub codec: &'static str,
    pub halves: (String, String),
    #[configured]
    pub settings: StreamChannelSettings,
}

#[derive(Builder, Debug)]
#[builder(build = StreamChannel, fallible, entry = "new")]
pub struct StreamChannelParts<C: Codec, S: Split> {
    settings: StreamChannelSettings,
    #[builder(input)]
    codec: C,
    #[builder(input)]
    stream: S,
}

impl<C: Codec, S: Split> TryFrom<StreamChannelParts<C, S>> for StreamChannel {
    type Error = std::io::Error;

    fn try_from(parts: StreamChannelParts<C, S>) -> std::io::Result<StreamChannel> {
        Ok(StreamChannel {
            codec: parts.codec.name(),
            halves: parts.stream.split()?,
            settings: parts.settings,
        })
    }
}
