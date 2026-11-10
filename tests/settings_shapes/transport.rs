// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The limits of a stream channel, and the settings of a WebSocket dialer and listener: numbers
//! and durations with finite defaults, unit enums, optional addresses, a list of text, and an
//! optional group behind a `cfg`.

use core::net::SocketAddr;
use core::time::Duration;

use coqui_builder::Describe;

/// The settings of a channel over a byte stream.
pub mod stream {
    use super::*;

    /// The largest message by default.
    pub const DEFAULT_MAX_MESSAGE: usize = 1024 * 1024;

    /// What a stream channel takes whole through its settings.
    ///
    /// Every limit is a setting with a finite default; none has a floor beyond what makes it
    /// mean anything. Every key is optional.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Describe)]
    #[map(deny_unknown_fields)]
    #[non_exhaustive]
    pub struct StreamChannelSettings {
        /// The largest message, both ways, until a call changes it.
        #[map(default = StreamChannelSettings::new().max_message)]
        pub max_message: usize,
        /// How many messages the reader thread holds ahead of `receive` before it stops
        /// reading. 0 is 1.
        #[map(default = StreamChannelSettings::new().pending)]
        pub pending: usize,
        /// The most bytes queued to send, each message counting its frame and 64 bytes more.
        #[map(default = StreamChannelSettings::new().queued_bytes)]
        pub queued_bytes: usize,
        /// How long a frame being written may go with the stream taking none of it.
        #[map(default = StreamChannelSettings::new().send_limit)]
        pub send_limit: Duration,
        /// The most bytes one read takes from the stream. 0 is 1.
        #[map(default = StreamChannelSettings::new().read_buffer)]
        pub read_buffer: usize,
        /// How long `close` waits after the peer last sent anything for the peer's end.
        #[map(default = StreamChannelSettings::new().linger)]
        pub linger: Duration,
    }

    impl StreamChannelSettings {
        /// Every setting at its default.
        pub const fn new() -> Self {
            Self {
                max_message: DEFAULT_MAX_MESSAGE,
                pending: 64,
                queued_bytes: 64 * 1024 * 1024,
                send_limit: Duration::from_secs(30),
                read_buffer: 64 * 1024,
                linger: Duration::from_millis(500),
            }
        }
    }

    impl Default for StreamChannelSettings {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// The settings of a WebSocket dialer and listener.
pub mod web {
    use super::*;

    /// The bounds a peer cannot exceed. Every value has a finite default.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Describe)]
    #[map(deny_unknown_fields)]
    #[non_exhaustive]
    pub struct Limits {
        /// The largest message accepted or sent, in bytes. Larger ends the channel.
        #[map(default = Limits::default().max_message)]
        pub max_message: usize,
        /// Messages read and held for a slow reader. 0 is 1.
        #[map(default = Limits::default().pending)]
        pub pending: usize,
        /// How long opening may take in total.
        #[map(default = Limits::default().handshake)]
        pub handshake: Duration,
        /// Bytes queued to send, past which `send` answers that no progress was made.
        #[map(default = Limits::default().queued_bytes)]
        pub queued_bytes: usize,
        /// How long a peer may take nothing while there is something to send.
        #[map(default = Limits::default().send)]
        pub send: Duration,
        /// Silence after which a peer is pinged, and twice this after which it is gone.
        #[map(default = Limits::default().idle)]
        pub idle: Duration,
        /// Datagrams held for a slow reader, past which new ones are discarded and counted.
        #[map(default = Limits::default().pending_datagrams)]
        pub pending_datagrams: usize,
        /// The largest datagram over a WebSocket, in bytes, whichever way it goes.
        #[map(default = Limits::default().websocket_datagram)]
        pub websocket_datagram: usize,
    }

    impl Default for Limits {
        fn default() -> Self {
            Self {
                max_message: super::stream::DEFAULT_MAX_MESSAGE,
                pending: 64,
                handshake: Duration::from_secs(10),
                queued_bytes: 64 * 1024 * 1024,
                send: Duration::from_secs(30),
                idle: Duration::from_secs(60),
                pending_datagrams: 256,
                websocket_datagram: 64 * 1024,
            }
        }
    }

    /// How messages appear on the wire.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, coqui_builder::Leaf)]
    #[map(rename_all = "lowercase")]
    #[non_exhaustive]
    pub enum Frames {
        /// One UTF-8 text message per WebSocket message.
        #[default]
        Text,
        /// Binary WebSocket messages.
        Binary,
    }

    /// How an offer and its answer reach the other end.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, coqui_builder::Leaf)]
    #[map(rename_all = "kebab-case")]
    #[non_exhaustive]
    pub enum Signaling {
        /// On the WebSocket itself.
        #[default]
        OnChannel,
        /// By the caller.
        ByCaller,
    }

    /// How a channel climbs onto a data channel.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Describe)]
    #[map(deny_unknown_fields)]
    #[non_exhaustive]
    pub struct Settings {
        /// Whether to try at all.
        #[map(default = Settings::default().enabled)]
        pub enabled: bool,
        /// How long a data channel may take to open before the channel stays as it is.
        #[map(default = Settings::default().attempt)]
        pub attempt: Duration,
        /// The largest IP packet the path is known to carry. It caps a data channel's datagrams.
        #[map(default = Settings::default().path_mtu)]
        pub path_mtu: usize,
        /// The address a port names in its one candidate. Unset, it names the address the
        /// connection arrived at.
        pub public_address: Option<SocketAddr>,
        /// Lets the other end's path be at another address than the WebSocket's peer. Off by
        /// default.
        #[map(default = Settings::default().any_candidate)]
        pub any_candidate: bool,
        /// How offers and answers travel.
        #[map(default = Settings::default().signaling)]
        pub signaling: Signaling,
        /// Data channels held at once, past which an offer is refused.
        #[map(default = Settings::default().max_sessions)]
        pub max_sessions: usize,
        /// Relays held at once, each a socket. A new source past this replaces the oldest.
        #[map(default = Settings::default().max_relays)]
        pub max_relays: usize,
        /// Sources, and data channels, one IP address holds at once.
        #[map(default = Settings::default().max_per_address)]
        pub max_per_address: usize,
        /// Silence after which a relay ends.
        #[map(default = Settings::default().relay_idle)]
        pub relay_idle: Duration,
    }

    impl Default for Settings {
        fn default() -> Self {
            Self {
                enabled: true,
                attempt: Duration::from_secs(5),
                path_mtu: 1280,
                public_address: None,
                any_candidate: false,
                signaling: Signaling::OnChannel,
                max_sessions: 256,
                max_relays: 512,
                max_per_address: 64,
                relay_idle: Duration::from_secs(120),
            }
        }
    }

    /// What a dialer takes whole through its settings.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Describe)]
    #[map(deny_unknown_fields)]
    #[non_exhaustive]
    pub struct DialerSettings {
        /// How messages appear on the wire.
        #[map(default = DialerSettings::default().frames)]
        pub frames: Frames,
        /// Asks the listener to agree to datagrams on this channel.
        #[map(default = DialerSettings::default().datagrams)]
        pub datagrams: bool,
        /// The bounds of channels this dialer opens.
        #[map(default = DialerSettings::default().limits)]
        pub limits: Limits,
        /// Lets this dialer open `ws://` beyond loopback. Off by default.
        #[map(default = DialerSettings::default().plain_beyond_loopback)]
        pub plain_beyond_loopback: bool,
        /// Binds each connection's socket to this address before it connects.
        pub local_addr: Option<SocketAddr>,
        /// Asks for datagrams that climb onto a data channel on the same address when they can.
        #[allow(clippy::non_minimal_cfg)]
        #[cfg(all())]
        pub webrtc: Option<Settings>,
    }

    /// What a listener takes whole through its settings.
    #[derive(Clone, Debug, Default, PartialEq, Eq, Describe)]
    #[map(deny_unknown_fields)]
    #[non_exhaustive]
    pub struct ListenerSettings {
        /// How messages appear on the wire.
        #[map(default = ListenerSettings::default().frames)]
        pub frames: Frames,
        /// The bounds of channels this listener accepts.
        #[map(default = ListenerSettings::default().limits)]
        pub limits: Limits,
        /// Refuses a browser whose `Origin` is not listed. An empty list checks nothing.
        #[map(default = ListenerSettings::default().origins)]
        pub origins: Vec<String>,
        /// Serves plain WebSocket beyond loopback. Off by default.
        #[map(default = ListenerSettings::default().plain_beyond_loopback)]
        pub plain_beyond_loopback: bool,
    }
}
