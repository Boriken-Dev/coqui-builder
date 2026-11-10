// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! An ordered rule list: a list of groups under a renamed key, a unit enum, and fields held as
//! text through their `Display` and `FromStr`.

use core::fmt;
use core::str::FromStr;

use coqui_builder::Describe;

/// What a rule does to a connection it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, coqui_builder::Leaf)]
#[map(rename_all = "lowercase")]
pub enum Action {
    /// Let the connection through.
    Permit,
    /// Close it.
    #[default]
    Deny,
}

/// A malformed network or port range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(&'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// An IPv4 network, or `any`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Net {
    /// Every address.
    #[default]
    Any,
    /// A network: the address and the prefix length.
    V4(core::net::Ipv4Addr, u8),
}

impl fmt::Display for Net {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Net::Any => f.write_str("any"),
            Net::V4(addr, 32) => write!(f, "{addr}"),
            Net::V4(addr, bits) => write!(f, "{addr}/{bits}"),
        }
    }
}

impl FromStr for Net {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Net, ParseError> {
        if text == "any" {
            return Ok(Net::Any);
        }
        let (addr, bits) = text.split_once('/').unwrap_or((text, "32"));
        let addr = addr
            .parse()
            .map_err(|_| ParseError("expected an address"))?;
        let bits: u8 = bits
            .parse()
            .map_err(|_| ParseError("expected a prefix length"))?;
        if bits > 32 {
            return Err(ParseError("a prefix is at most 32 bits"));
        }
        Ok(Net::V4(addr, bits))
    }
}

/// A port, or an inclusive range of ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortRange {
    /// The first port.
    pub start: u16,
    /// The last port.
    pub end: u16,
}

impl fmt::Display for PortRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

impl FromStr for PortRange {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<PortRange, ParseError> {
        let port = |part: &str| part.parse().map_err(|_| ParseError("expected a port"));
        let (start, end) = match text.split_once('-') {
            Some((start, end)) => (port(start)?, port(end)?),
            None => (port(text)?, port(text)?),
        };
        if start > end {
            return Err(ParseError("a range runs from low to high"));
        }
        Ok(PortRange { start, end })
    }
}

/// One line of an ACL.
///
/// `source` is matched against the connecting address; `destination`, `port` and `fingerprint`
/// narrow the rule and are ignored when absent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Describe)]
#[map(deny_unknown_fields)]
pub struct Rule {
    /// What to do on a match.
    pub action: Action,
    /// The connecting address.
    #[map(default, as_text)]
    pub source: Net,
    /// The local address connected to. `None` is any.
    #[map(as_text)]
    pub destination: Option<Net>,
    /// The local port.
    #[map(as_text)]
    pub port: Option<PortRange>,
    /// The peer's certificate fingerprint in canonical text. A rule with one is decided only
    /// after the handshake.
    pub fingerprint: Option<String>,
    /// A hint, returned in the decision, that a host should log a match.
    #[map(default)]
    pub log: bool,
}

impl Rule {
    /// A rule for `source` with no further conditions.
    pub fn new(action: Action, source: Net) -> Self {
        Self {
            action,
            source,
            destination: None,
            port: None,
            fingerprint: None,
            log: false,
        }
    }
}

/// An ordered rule list and the action when none matches.
#[derive(Debug, Clone, PartialEq, Eq, Default, Describe)]
#[map(deny_unknown_fields)]
pub struct Acl {
    /// Rules, top to bottom.
    #[map(default, rename = "rule")]
    pub rules: Vec<Rule>,
    /// The action when no rule matches. `Deny` unless set.
    #[map(default)]
    pub default: Action,
}
