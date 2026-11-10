// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The settings of a listener and of a connection: platform-specific keys, optional text,
//! and timeouts whose written `never` is the absence of a bound.

use core::time::Duration;

use coqui_builder::Describe;

/// The default bound on one read and one write of a stream.
pub const DEFAULT_IO_TIMEOUT: Duration = Duration::from_secs(30);
/// The default bound on connecting.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// The default size of each direction's buffer of a pipe instance on Windows, in bytes.
pub const DEFAULT_BUFFER_SIZE: u32 = 64 * 1024;
/// The default queue of connections a socket holds for `accept` on Unix.
pub const DEFAULT_BACKLOG: i32 = 128;
/// The default mode of a socket on Unix: its owner only.
pub const DEFAULT_SOCKET_MODE: u32 = 0o600;
/// The most instances Windows lets a pipe have, which it treats as no limit of its own.
pub const UNLIMITED_INSTANCES: u32 = 255;

/// What a listener takes whole through its settings.
///
/// A setting for the other platform has no effect there. Every key is optional.
#[derive(Clone, Debug, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct ListenerSettings {
    /// Windows: accepts clients from other hosts. Off by default.
    #[map(default = ListenerSettings::new().remote_clients)]
    pub remote_clients: bool,
    /// Windows: this descriptor, in SDDL, in place of the default.
    pub security_descriptor: Option<String>,
    /// Windows: refuses a name that already exists. On by default.
    #[map(default = ListenerSettings::new().first_instance)]
    pub first_instance: bool,
    /// Windows: the most instances of this pipe at once, from 2 to 254; 255, the default,
    /// leaves it to the system.
    #[map(default = ListenerSettings::new().max_instances)]
    pub max_instances: u32,
    /// Windows: the size of each direction's buffer of an instance, in bytes.
    #[map(default = ListenerSettings::new().buffer_size)]
    pub buffer_size: u32,
    /// Unix: the socket's mode, any of `0` to `0o7777`; `0o600` (384 as a number in text) by
    /// default. Another is reported, never refused.
    #[map(default = ListenerSettings::new().socket_mode)]
    pub socket_mode: u32,
    /// Unix: the socket's group: an id, or a name looked up when binding. In text, digits
    /// alone are an id.
    pub socket_group: Option<String>,
    /// Unix: binds even when others can write a directory on the way to the socket. Off by
    /// default.
    #[map(default = ListenerSettings::new().shared_directory)]
    pub shared_directory: bool,
    /// Unix: removes a socket left at the path once nothing listens on it. On by default.
    #[map(default = ListenerSettings::new().remove_stale)]
    pub remove_stale: bool,
    /// Unix: how many connections wait for `accept`, up to the system's own limit.
    #[map(default = ListenerSettings::new().backlog)]
    pub backlog: i32,
    /// The bound on each read of an accepted stream; `never` waits forever.
    #[map(default = Some(DEFAULT_IO_TIMEOUT), none = "never")]
    pub read_timeout: Option<Duration>,
    /// The bound on each write of an accepted stream; `never` waits forever.
    #[map(default = Some(DEFAULT_IO_TIMEOUT), none = "never")]
    pub write_timeout: Option<Duration>,
}

impl Default for ListenerSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl ListenerSettings {
    /// Every setting at its default.
    pub const fn new() -> Self {
        Self {
            remote_clients: false,
            security_descriptor: None,
            first_instance: true,
            max_instances: UNLIMITED_INSTANCES,
            buffer_size: DEFAULT_BUFFER_SIZE,
            socket_mode: DEFAULT_SOCKET_MODE,
            socket_group: None,
            shared_directory: false,
            remove_stale: true,
            backlog: DEFAULT_BACKLOG,
            read_timeout: Some(DEFAULT_IO_TIMEOUT),
            write_timeout: Some(DEFAULT_IO_TIMEOUT),
        }
    }
}

/// What a connection takes whole through its settings. Read as [`ListenerSettings`] is.
#[derive(Clone, Debug, PartialEq, Eq, Describe)]
#[map(deny_unknown_fields)]
#[non_exhaustive]
pub struct ConnectSettings {
    /// How long to wait for a free instance or a place in the queue.
    #[map(default = ConnectSettings::new().timeout)]
    pub timeout: Duration,
    /// Windows: lets the server act as this client. Off by default.
    #[map(default = ConnectSettings::new().impersonation)]
    pub impersonation: bool,
    /// The bound on each read of the stream; `never` waits forever.
    #[map(default = Some(DEFAULT_IO_TIMEOUT), none = "never")]
    pub read_timeout: Option<Duration>,
    /// The bound on each write of the stream; `never` waits forever.
    #[map(default = Some(DEFAULT_IO_TIMEOUT), none = "never")]
    pub write_timeout: Option<Duration>,
}

impl Default for ConnectSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectSettings {
    /// Every setting at its default.
    pub const fn new() -> Self {
        Self {
            timeout: DEFAULT_CONNECT_TIMEOUT,
            impersonation: false,
            read_timeout: Some(DEFAULT_IO_TIMEOUT),
            write_timeout: Some(DEFAULT_IO_TIMEOUT),
        }
    }
}
