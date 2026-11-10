// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `#[derive(Builder)]`: a settings type, a parts struct, layers then `build`, and the typed
//! setters called from another module as another crate would.

use std::time::Duration;

use coqui_builder::builder::BuildError;
use coqui_builder::{Builder, Env, Missing, Partial};

mod settings {
    use std::time::Duration;

    use coqui_builder::Describe;

    /// Caps on what a server accepts.
    #[derive(Describe, Debug, Clone, PartialEq)]
    pub struct Limits {
        #[map(default = 10)]
        pub total: u32,
        #[map(default)]
        pub per_source: u32,
    }

    #[derive(Describe, Debug, Clone, PartialEq)]
    pub struct ServerSettings {
        /// The name to listen on.
        pub host: String,
        #[map(default = 8080)]
        pub port: u16,
        #[map(default)]
        pub names: Vec<String>,
        #[map(default = Some(Duration::from_secs(30)), none = "never")]
        pub idle: Option<Duration>,
        pub limits: Limits,
        #[allow(clippy::non_minimal_cfg)]
        #[cfg(all())]
        #[map(default)]
        pub verbose: bool,
        #[cfg(any())]
        #[map(default)]
        pub gone: bool,
        #[builder(setter = "with_new")]
        #[map(default)]
        pub new: bool,
    }

    #[derive(Describe, Debug, Clone, PartialEq)]
    pub struct ClientSettings {
        #[map(default = 3)]
        pub retries: u8,
        #[map(default = "client".to_owned())]
        pub name: String,
    }
}

use settings::{
    ClientSettings, ClientSettingsSetters, Limits, ServerSettings, ServerSettingsSetters,
};

#[derive(Debug, Clone, PartialEq)]
struct Identity(String);

impl From<&str> for Identity {
    fn from(text: &str) -> Identity {
        Identity(text.to_owned())
    }
}

#[derive(Debug, PartialEq)]
struct Server {
    pub settings: ServerSettings,
    pub identity: Identity,
    pub label: Option<String>,
}

#[derive(Builder)]
#[builder(build = Server)]
struct ServerParts {
    settings: ServerSettings,
    /// Who the server says it is.
    #[builder(input)]
    identity: Identity,
    #[builder(input)]
    label: Option<String>,
}

impl From<ServerParts> for Server {
    fn from(parts: ServerParts) -> Server {
        Server {
            settings: parts.settings,
            identity: parts.identity,
            label: parts.label,
        }
    }
}

#[derive(Debug, PartialEq)]
struct Client {
    pub settings: ClientSettings,
    pub identity: Identity,
}

#[derive(Debug, PartialEq)]
struct Refused(String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused: {}", self.0)
    }
}

impl std::error::Error for Refused {}

#[derive(Builder)]
#[builder(build = Client, fallible, entry = "new")]
struct ClientParts<'a> {
    settings: ClientSettings,
    #[builder(input)]
    identity: &'a Identity,
}

impl TryFrom<ClientParts<'_>> for Client {
    type Error = Refused;

    fn try_from(parts: ClientParts<'_>) -> Result<Client, Refused> {
        if parts.settings.retries == 0 {
            return Err(Refused("no retries".into()));
        }
        Ok(Client {
            settings: parts.settings,
            identity: parts.identity.clone(),
        })
    }
}

struct Built<T>(T);

#[derive(Builder)]
#[builder(build = Built<ClientSettings>, name = "ConfigBuilder", entry = "new")]
struct ConfigParts {
    settings: ClientSettings,
}

impl From<ConfigParts> for Built<ClientSettings> {
    fn from(parts: ConfigParts) -> Built<ClientSettings> {
        Built(parts.settings)
    }
}

#[test]
fn a_named_builder_builds_a_generic_type_through_new() {
    let Built(settings) = ConfigBuilder::new().retries(9u8).build().unwrap();
    assert_eq!(settings.retries, 9);
    assert_eq!(settings.name, "client");
}

fn identity() -> Identity {
    Identity::from("srv")
}

fn text(pairs: &[(&str, &str)]) -> Partial<ServerSettings> {
    let mut partial = Partial::new();
    for (key, value) in pairs {
        partial.set(key, value).unwrap();
    }
    partial
}

#[test]
fn every_setting_and_input_reaches_the_built_value() {
    let server = Server::builder()
        .host("example")
        .port(9000u16)
        .names(vec!["a".to_owned(), "b".to_owned()])
        .idle(Duration::from_secs(5))
        .limits(Limits {
            total: 7,
            per_source: 2,
        })
        .verbose(true)
        .with_new(true)
        .identity("srv")
        .label("main")
        .build()
        .unwrap();
    assert_eq!(
        server,
        Server {
            settings: ServerSettings {
                host: "example".into(),
                port: 9000,
                names: vec!["a".into(), "b".into()],
                idle: Some(Duration::from_secs(5)),
                limits: Limits {
                    total: 7,
                    per_source: 2
                },
                verbose: true,
                new: true,
            },
            identity: identity(),
            label: Some("main".into()),
        }
    );
}

#[test]
fn an_optional_input_stays_none_and_a_setter_takes_the_inner_type() {
    let server = Server::builder().host("h").identity("srv").build().unwrap();
    assert_eq!(server.label, None);
    assert_eq!(server.settings.port, 8080);
    assert_eq!(server.settings.idle, Some(Duration::from_secs(30)));
    // `None` is a value of an optional setting, and an `Option` converts into itself.
    let server = Server::builder()
        .host("h")
        .idle(None::<Duration>)
        .identity("srv")
        .build()
        .unwrap();
    assert_eq!(server.settings.idle, None);
}

#[test]
fn layers_apply_in_the_owners_order_and_the_last_write_of_a_key_wins() {
    // A file, then the environment, then flags, then code.
    let file = text(&[("host", "from-file"), ("port", "1"), ("limits.total", "5")]);
    let mut env = Env::from_vars(
        "APP",
        [
            ("APP_PORT", "2"),
            ("APP_LIMITS__PER_SOURCE", "3"),
            ("APP_IDLE", "never"),
        ],
    );
    let flags = text(&[("port", "3"), ("names[0]", "flag")]);
    let server = Server::builder()
        .fill(file)
        .fill_from(&mut env)
        .unwrap()
        .fill(flags)
        .identity("srv")
        .verbose(true)
        .build()
        .unwrap();
    assert_eq!(
        server.settings.host, "from-file",
        "an unset key keeps the earlier one"
    );
    assert_eq!(server.settings.port, 3, "a later layer wins");
    assert_eq!(server.settings.limits.total, 5);
    assert_eq!(server.settings.limits.per_source, 3);
    assert_eq!(server.settings.idle, None);
    assert_eq!(server.settings.names, ["flag"]);
    assert!(server.settings.verbose);

    // A setter after a layer wins over it, and the last call of a setter wins.
    let server = Server::builder()
        .fill(text(&[("host", "x"), ("port", "1")]))
        .port(2u16)
        .port(4u16)
        .identity("srv")
        .build()
        .unwrap();
    assert_eq!(server.settings.port, 4);
}

#[test]
fn an_empty_builder_lists_every_required_key_and_input_at_once() {
    let missing: Missing = Server::builder().build().unwrap_err();
    let keys: Vec<String> = missing.keys().iter().map(ToString::to_string).collect();
    assert_eq!(keys, ["host"]);
    assert_eq!(missing.inputs(), ["identity"]);
    assert!(!missing.is_empty());
    assert_eq!(
        missing.to_string(),
        "missing settings: host; missing inputs: identity"
    );

    // Only inputs missing: the settings clause is left out, and nothing is ever a value.
    let missing = Server::builder().host("h").build().unwrap_err();
    assert_eq!(missing.to_string(), "missing inputs: identity");
    let missing = Server::builder().identity("srv").build().unwrap_err();
    assert_eq!(missing.to_string(), "missing settings: host");
}

#[test]
fn from_a_partial_equals_builder_fill() {
    let one = Server::builder()
        .fill(text(&[("host", "h"), ("port", "5")]))
        .identity("srv")
        .build()
        .unwrap();
    let two = ServerBuilder::from(text(&[("host", "h"), ("port", "5")]))
        .identity("srv")
        .build()
        .unwrap();
    assert_eq!(one, two);
}

#[test]
fn a_fill_from_drops_unknown_keys_and_a_filled_partial_sees_them() {
    let mut env = Env::from_vars("APP", [("APP_HOST", "h"), ("APP_NOPE", "x")]);
    let built = Server::builder()
        .fill_from(&mut env)
        .unwrap()
        .identity("srv")
        .build();
    assert!(built.is_ok());
    let mut partial = Partial::<ServerSettings>::new();
    let mut env = Env::from_vars("APP", [("APP_HOST", "h"), ("APP_NOPE", "x")]);
    let unknown = partial.fill_from(&mut env).unwrap();
    assert_eq!(unknown.keys().len(), 1);
}

#[test]
fn a_fallible_build_passes_the_crates_error_through() {
    let me = identity();
    let client = ClientBuilder::new().identity(&me).build().unwrap();
    assert_eq!(client.settings.retries, 3);
    assert_eq!(client.identity, me);

    let refused = ClientBuilder::new()
        .identity(&me)
        .retries(0u8)
        .build()
        .unwrap_err();
    assert!(matches!(&refused, BuildError::Failed(Refused(why)) if why == "no retries"));
    assert_eq!(refused.to_string(), "refused: no retries");

    let missing = ClientBuilder::new().build().unwrap_err();
    assert!(matches!(&missing, BuildError::Missing(m) if m.inputs() == ["identity"]));
    assert_eq!(missing.to_string(), "missing inputs: identity");
}

#[test]
fn a_builders_debug_shows_which_inputs_are_set_and_never_a_value() {
    let builder = Server::builder()
        .identity("secret-identity")
        .host("secret-host");
    let shown = format!("{builder:?}");
    assert!(shown.contains("identity: \"set\""), "{shown}");
    assert!(shown.contains("label: \"unset\""), "{shown}");
    assert!(!shown.contains("secret"), "{shown}");
}

#[test]
fn a_setting_named_like_a_builder_method_is_set_through_its_renamed_setter() {
    let server = Server::builder()
        .host("h")
        .identity("srv")
        .with_new(true)
        .build()
        .unwrap();
    assert!(server.settings.new);
}

#[derive(Debug)]
struct Outer(std::io::Error);

impl std::fmt::Display for Outer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("outer")
    }
}

impl std::error::Error for Outer {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn a_refused_builds_source_is_the_conversions_source_not_the_conversion_itself() {
    use std::error::Error;
    let inner = Outer(std::io::Error::other("disk"));
    let failed: BuildError<Outer> = BuildError::Failed(inner);
    assert_eq!(failed.to_string(), "outer");
    assert_eq!(failed.source().unwrap().to_string(), "disk");
    let missing: BuildError<Outer> = BuildError::Missing(Missing::new(Vec::new(), vec!["id"]));
    assert!(missing.source().is_none());
    assert_eq!(missing.to_string(), "missing inputs: id");
}
