// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Settings types of several shapes, and builders over them: each is described with the keys
//! its fields are, filled from the keys and texts a configuration uses, compared with its
//! defaults, and checked against a profile.
//!
//! The types are written as an application writes them: each keeps the keys, defaults and
//! attributes of the type it copies (a few doc comments are shortened), and the derive is the
//! only thing that describes them.

#[path = "settings_shapes/acl.rs"]
mod acl;
#[path = "settings_shapes/builders.rs"]
mod builders;
#[path = "settings_shapes/cache.rs"]
mod cache;
#[path = "settings_shapes/limits.rs"]
mod limits;
#[path = "settings_shapes/noise.rs"]
mod noise;
#[path = "settings_shapes/pipe.rs"]
mod pipe;
#[path = "settings_shapes/store.rs"]
mod store;
#[path = "settings_shapes/tls.rs"]
mod tls;
#[path = "settings_shapes/transport.rs"]
mod transport;

use std::time::Duration;

use coqui_builder::describe::{Kind, LeafError};
use coqui_builder::{
    Check, Configured, Describe, Description, Env, Expect, Leaf, NonDefault, Partial, Profile,
    Reason, non_defaults,
};

use acl::{Acl, Action, Net, PortRange, Rule};
use builders::{
    ClientAuth, ClientBuilder, Clock, Identity, Listener, PeerError, PipeName, ServerAuth,
    ServerBuilder, Trust, TrustStore,
};
use cache::{CacheOptions, CacheSetting, ProviderSettings};
use noise::{NoiseSettings, PairingSettings};
use pipe::{ConnectSettings, ListenerSettings};
use store::{Digest, TrustStoreSettings, WaitingLimits, WhenFull};
use tls::{
    Alpn, CipherProfile, Crypto, PairConfig, Policy, PolicyWithoutAcl, TlsSettings, TlsVersion,
};

// Setters are traits of the settings' module, imported where a builder is used.
use pipe::ListenerSettingsSetters;
use store::TrustStoreSettingsSetters;
use tls::TlsSettingsSetters;

fn keys<T: Describe>() -> Vec<String> {
    let description: Description = T::description();
    match description.kind() {
        Kind::Group(group) => group.fields.iter().map(|f| f.key.clone()).collect(),
        other => panic!("not a group: {other:?}"),
    }
}

fn partial<T: Describe>(pairs: &[(&str, &str)]) -> Partial<T> {
    let mut partial = Partial::<T>::new();
    for (key, text) in pairs {
        partial
            .set(key, text)
            .unwrap_or_else(|error| panic!("{key} = {text}: {error}"));
    }
    partial
}

fn built<T: Describe>(pairs: &[(&str, &str)]) -> T {
    partial::<T>(pairs).build().expect("every key is given")
}

fn names(found: &[NonDefault]) -> Vec<&str> {
    found.iter().map(|n| n.setting.as_str()).collect()
}

fn shown(found: &[NonDefault], setting: &str) -> (String, String) {
    let found = found
        .iter()
        .find(|n| n.setting == setting)
        .unwrap_or_else(|| panic!("{setting} is not listed in {:?}", names(found)));
    (found.value.to_string(), found.default.to_string())
}

fn texts(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn env<'a>(prefix: &str, vars: impl IntoIterator<Item = (&'a str, &'a str)>) -> Env {
    Env::from_vars(
        prefix,
        vars.into_iter().map(|(k, v)| (k.to_owned(), v.to_owned())),
    )
}

// ---- the keys are the sources', in their order ----

#[test]
fn every_description_has_the_keys_of_its_source_in_their_order() {
    assert_eq!(
        keys::<Policy>(),
        [
            "tls_versions",
            "cipher_suites",
            "kx_groups",
            "cipher_profiles",
            "crypto",
            "pin_digests",
            "check_validity",
            "allow_chain_when_pinned",
            "allow_resumption",
            "require_ems",
            "waiting",
            "acl",
        ]
    );
    // The field behind a `cfg` that is false is no key, with no feature added.
    assert_eq!(keys::<PolicyWithoutAcl>(), ["tls_versions", "waiting"]);
    assert_eq!(keys::<TlsSettings>(), ["alpn", "policy"]);
    assert_eq!(keys::<PairConfig>(), ["policy", "alpn", "timeout"]);
    assert_eq!(keys::<WaitingLimits>(), ["per_source", "total", "rerecord"]);
    assert_eq!(
        keys::<TrustStoreSettings>(),
        ["state_dir", "waiting", "rotation"]
    );
    assert_eq!(
        keys::<ListenerSettings>(),
        [
            "remote_clients",
            "security_descriptor",
            "first_instance",
            "max_instances",
            "buffer_size",
            "socket_mode",
            "socket_group",
            "shared_directory",
            "remove_stale",
            "backlog",
            "read_timeout",
            "write_timeout",
        ]
    );
    assert_eq!(
        keys::<ConnectSettings>(),
        ["timeout", "impersonation", "read_timeout", "write_timeout"]
    );
    assert_eq!(keys::<NoiseSettings>(), ["key_limit", "rekey"]);
    assert_eq!(
        keys::<PairingSettings>(),
        ["key_limit", "rekey", "first_message_timeout"]
    );
    assert_eq!(keys::<ProviderSettings>(), ["directory", "cache"]);
    assert_eq!(
        keys::<CacheOptions>(),
        ["ttl", "negative_ttl", "max_entries"]
    );
    // The list of rules sits under the key `rule`.
    assert_eq!(keys::<Acl>(), ["rule", "default"]);
}

#[test]
fn an_empty_partial_builds_the_default_of_each_type() {
    assert_eq!(built::<Policy>(&[]), Policy::default());
    assert_eq!(built::<TlsSettings>(&[]), TlsSettings::default());
    assert_eq!(built::<PairConfig>(&[]), PairConfig::default());
    assert_eq!(built::<ListenerSettings>(&[]), ListenerSettings::default());
    assert_eq!(built::<ConnectSettings>(&[]), ConnectSettings::default());
    assert_eq!(built::<NoiseSettings>(&[]), NoiseSettings::default());
    assert_eq!(built::<PairingSettings>(&[]), PairingSettings::default());
    assert_eq!(built::<Acl>(&[]), Acl::default());
    assert_eq!(built::<WaitingLimits>(&[]), WaitingLimits::default());
    for found in [
        non_defaults(&Policy::default()),
        non_defaults(&PairConfig::default()),
        non_defaults(&ListenerSettings::default()),
        non_defaults(&ConnectSettings::default()),
        non_defaults(&PairingSettings::default()),
        non_defaults(&Acl::default()),
        non_defaults(&ProviderSettings {
            directory: String::new(),
            cache: CacheSetting::default(),
        })
        .into_iter()
        .filter(|n| n.setting != "directory")
        .collect(),
    ] {
        assert!(found.is_empty(), "{found:?}");
    }
}

// ---- a transport-security policy ----

#[test]
fn a_policy_fills_from_the_keys_and_texts_a_configuration_uses() {
    let mut p = partial::<Policy>(&[("waiting.per_source", "32"), ("crypto", "aws-lc")]);
    p.set_items("tls_versions", ["1.3"]).unwrap();
    p.set_items("cipher_profiles", ["mozilla", "cnsa-1.0"])
        .unwrap();
    p.set_items("cipher_suites", ["TLS13_AES_256_GCM_SHA384"])
        .unwrap();
    p.set_items("pin_digests", ["sha384"]).unwrap();
    p.set("require_ems", "false").unwrap();
    let policy = p.build().unwrap();
    assert_eq!(policy.tls_versions, [TlsVersion::V1_3]);
    assert_eq!(
        policy.cipher_profiles,
        [CipherProfile::Mozilla, CipherProfile::Cnsa10]
    );
    assert_eq!(
        policy.cipher_suites,
        Some(texts(&["TLS13_AES_256_GCM_SHA384"]))
    );
    assert_eq!(policy.kx_groups, None);
    assert_eq!(policy.crypto, Crypto::AwsLc);
    assert_eq!(policy.pin_digests, [Digest::Sha384]);
    assert!(!policy.require_ems);
    assert_eq!(policy.waiting.per_source, 32);
    assert_eq!(policy.waiting.total, 1_000, "the rest keeps its default");
    assert_eq!(policy.acl, None);
}

#[test]
fn a_policy_refuses_what_its_values_refuse() {
    let mut p = Partial::<Policy>::new();
    // A version before 1.2 is an error, never ignored; the text of the refusal is the value's.
    let error = p.set_items("tls_versions", ["1.1"]).unwrap_err();
    assert!(error.to_string().contains("tls_versions"), "{error}");
    assert!(p.set_items("tls_versions", ["1.4"]).is_err());
    assert!(p.set("crypto", "openssl").is_err());
    assert!(p.set_items("pin_digests", ["md5"]).is_err());
    assert!(p.set("waiting.total", "-1").is_err());
    assert!(p.set("no_such_key", "1").is_err());
    assert_eq!(
        p.build().unwrap(),
        Policy::default(),
        "a refusal sets nothing"
    );
}

#[test]
fn a_policy_reads_the_environment_and_a_later_layer_wins() {
    let mut p = Partial::<Policy>::new();
    p.set("waiting.total", "5").unwrap();
    let mut source = env(
        "TLS",
        [
            ("TLS_TLS_VERSIONS__0", "1.3"),
            ("TLS_WAITING__TOTAL", "7"),
            ("TLS_PIN_DIGESTS__0", "sha256"),
            ("TLS_PIN_DIGESTS__1", "sha512"),
        ],
    );
    p.fill_from(&mut source).unwrap();
    let policy = p.build().unwrap();
    assert_eq!(policy.tls_versions, [TlsVersion::V1_3]);
    assert_eq!(policy.pin_digests, [Digest::Sha256, Digest::Sha512]);
    assert_eq!(policy.waiting.total, 7);
}

#[test]
fn the_non_defaults_of_a_policy_are_the_settings_its_tests_expect() {
    assert!(non_defaults(&Policy::intermediate()).is_empty());

    // `modern` reports the versions, shown as a list of the type and its text.
    let found = non_defaults(&Policy::modern());
    assert_eq!(names(&found), ["tls_versions"]);
    assert_eq!(
        shown(&found, "tls_versions"),
        (
            "[TlsVersion(1.3)]".to_owned(),
            "[TlsVersion(1.2), TlsVersion(1.3)]".to_owned()
        )
    );

    // `legacy` reports its digests and the extended master secret.
    let found = non_defaults(&Policy::legacy());
    assert_eq!(names(&found), ["pin_digests", "require_ems"]);
    assert_eq!(
        shown(&found, "require_ems"),
        ("bool(false)".to_owned(), "bool(true)".to_owned())
    );

    // Resumption is an opt-in reported as a non-default: the value was the text `true` in an
    // earlier rendering and is `bool(true)` as a datum (see the test named
    // `resumption_is_an_opt_in_reported_as_a_non_default`).
    let mut policy = Policy::modern();
    policy.allow_resumption = true;
    let found = non_defaults(&policy);
    assert_eq!(names(&found), ["allow_resumption", "tls_versions"]);
    assert_eq!(
        shown(&found, "allow_resumption"),
        ("bool(true)".to_owned(), "bool(false)".to_owned())
    );

    let mut policy = Policy::default();
    policy.waiting.total = 7;
    assert_eq!(names(&non_defaults(&policy)), ["waiting.total"]);
}

#[test]
fn an_access_list_inside_a_policy_is_one_group_that_is_present_or_not() {
    let policy = Policy {
        acl: Some(Acl {
            rules: vec![Rule::new(Action::Permit, Net::V4([10, 0, 0, 0].into(), 8))],
            default: Action::Permit,
        }),
        ..Policy::default()
    };
    let found = non_defaults(&policy);
    // A group present where the default has none is one place, at the group, with no value;
    // the list's own settings are read from the list (see the access list tests below).
    assert_eq!(names(&found), ["acl"]);
    assert_eq!(
        non_defaults(policy.acl.as_ref().unwrap())
            .iter()
            .map(|n| n.setting.as_str())
            .collect::<Vec<_>>(),
        ["default", "rule[0]"]
    );

    // The whole list reads from keys under `acl`, and the group is present once one is given.
    let policy = built::<Policy>(&[
        ("acl.default", "permit"),
        ("acl.rule[0].action", "deny"),
        ("acl.rule[0].source", "192.168.0.0/16"),
        ("acl.rule[0].fingerprint", "sha256:00"),
    ]);
    let acl = policy.acl.unwrap();
    assert_eq!(acl.default, Action::Permit);
    assert_eq!(acl.rules[0].fingerprint.as_deref(), Some("sha256:00"));
}

// ---- the settings of the configuration builders and of pairing ----

#[test]
fn the_application_protocols_are_names_of_one_to_255_bytes() {
    let settings = built::<TlsSettings>(&[("alpn[0]", "h2"), ("alpn[1]", "app/1")]);
    assert_eq!(settings.alpn_bytes(), [b"h2".to_vec(), b"app/1".to_vec()]);
    let mut p = Partial::<TlsSettings>::new();
    assert!(p.set("alpn[0]", "").is_err());
    assert!(p.set("alpn[0]", &"x".repeat(256)).is_err());
    assert!(p.set("alpn[0]", &"x".repeat(255)).is_ok());
    assert!(matches!(Alpn::description().kind(), Kind::Leaf(_)));
}

#[test]
fn a_pairing_alpn_is_bytes_held_as_text_by_a_module_of_converters() {
    let config = built::<PairConfig>(&[("alpn", "other/2")]);
    assert_eq!(config.alpn, b"other/2");
    let mut p = Partial::<PairConfig>::new();
    assert!(p.set("alpn", "").is_err());
    assert!(p.set("alpn", &"x".repeat(256)).is_err());
    assert_eq!(
        tls::alpn_text::from_text(""),
        Err(LeafError::Refused(
            "a protocol name of 1 to 255 bytes".to_owned()
        ))
    );
    let found = non_defaults(&config);
    assert_eq!(names(&found), ["alpn"]);
    assert_eq!(
        shown(&found, "alpn"),
        (
            "Alpn(other/2)".to_owned(),
            "Alpn(example-pair/1)".to_owned()
        )
    );
}

#[test]
fn a_timeout_that_waited_forever_as_the_largest_duration_is_written_never() {
    assert_eq!(
        built::<PairConfig>(&[]).timeout,
        Some(Duration::from_secs(900))
    );
    assert_eq!(built::<PairConfig>(&[("timeout", "never")]).timeout, None);
    assert_eq!(
        built::<PairConfig>(&[("timeout", "20")]).timeout,
        Some(Duration::from_secs(20))
    );
    // The largest duration is no longer a way to say it.
    let mut p = Partial::<PairConfig>::new();
    assert!(p.set("timeout", "forever").is_err());
    let unbounded = PairConfig {
        timeout: None,
        ..PairConfig::default()
    };
    let found = non_defaults(&unbounded);
    assert_eq!(names(&found), ["timeout"]);
    assert_eq!(
        shown(&found, "timeout"),
        ("Duration(never)".to_owned(), "Duration(900s)".to_owned())
    );
}

// ---- a trust store ----

#[test]
fn the_digest_is_its_lowercase_name_and_a_full_queue_its_kebab_case_one() {
    let settings = built::<TrustStoreSettings>(&[
        ("state_dir", "/var/lib/peers"),
        ("waiting.when_full", "replace-oldest"),
        ("waiting.limits.per_source", "32"),
        ("waiting.limits.rerecord", "2m"),
        ("rotation.retire_after", "1h"),
    ]);
    assert_eq!(settings.waiting.when_full, WhenFull::ReplaceOldest);
    assert_eq!(settings.waiting.limits.per_source, 32);
    assert_eq!(settings.waiting.limits.rerecord, Duration::from_secs(120));
    assert_eq!(settings.rotation.retire_after, Duration::from_secs(3600));
    assert_eq!(settings.waiting.ipv6_prefix, 64);
    let mut p = Partial::<TrustStoreSettings>::new();
    assert!(p.set("waiting.when_full", "ReplaceOldest").is_err());
    assert!(p.set("waiting.limits.total", "ten").is_err());
    assert_eq!(Digest::Sha384.to_text(), "sha384");
}

#[test]
fn a_trust_store_without_its_directory_is_missing_that_key() {
    let missing = Partial::<TrustStoreSettings>::new().build().unwrap_err();
    assert_eq!(missing.to_string(), "missing settings: state_dir");
}

#[test]
fn the_non_defaults_of_a_trust_store_are_named_by_their_paths() {
    let settings = built::<TrustStoreSettings>(&[
        ("state_dir", "/var/lib/peers"),
        ("waiting.limits.total", "5"),
        ("waiting.when_full", "replace-oldest"),
        ("rotation.require_approval", "true"),
    ]);
    let found = non_defaults(&settings);
    assert_eq!(
        names(&found),
        [
            "rotation.require_approval",
            "state_dir",
            "waiting.limits.total",
            "waiting.when_full"
        ]
    );
    assert_eq!(
        shown(&found, "waiting.limits.total"),
        ("usize(5)".to_owned(), "usize(1000)".to_owned())
    );
    assert_eq!(
        shown(&found, "waiting.when_full"),
        (
            "WhenFull(replace-oldest)".to_owned(),
            "WhenFull(refuse)".to_owned()
        )
    );
    assert_eq!(
        shown(&found, "state_dir").1,
        "unset",
        "a required key has no default"
    );
}

// ---- a listener and a connection ----

#[test]
fn a_timeout_reads_seconds_a_unit_or_never() {
    for (text, want) in [
        ("2", Some(Duration::from_secs(2))),
        ("0.25", Some(Duration::from_millis(250))),
        ("never", None),
    ] {
        let settings = built::<ListenerSettings>(&[("backlog", "4"), ("read_timeout", text)]);
        assert_eq!(settings.read_timeout, want, "{text}");
        assert_eq!(settings.backlog, 4);
        assert_eq!(settings.write_timeout, Some(Duration::from_secs(30)));
    }
    let mut p = Partial::<ConnectSettings>::new();
    assert!(p.set("read_timeout", "Never").is_err());
    assert!(p.set("write_timeout", "-1").is_err());
}

#[test]
fn the_non_defaults_of_a_listener_and_a_connection_are_named_by_key() {
    let settings = built::<ConnectSettings>(&[("read_timeout", "never"), ("timeout", "3s")]);
    let found = non_defaults(&settings);
    assert_eq!(names(&found), ["read_timeout", "timeout"]);
    assert_eq!(
        shown(&found, "read_timeout"),
        ("Duration(never)".to_owned(), "Duration(30s)".to_owned())
    );
    assert_eq!(
        shown(&found, "timeout"),
        ("Duration(3s)".to_owned(), "Duration(5s)".to_owned())
    );

    // The mode is a plain number, 384 in text, and another mode is listed, never refused.
    let settings = built::<ListenerSettings>(&[
        ("socket_mode", "432"),
        ("socket_group", "staff"),
        ("read_timeout", "5"),
        ("write_timeout", "5"),
    ]);
    let found = non_defaults(&settings);
    assert_eq!(
        names(&found),
        [
            "read_timeout",
            "socket_group",
            "socket_mode",
            "write_timeout"
        ]
    );
    assert_eq!(
        shown(&found, "socket_mode"),
        ("u32(432)".to_owned(), "u32(384)".to_owned())
    );
    assert_eq!(shown(&found, "socket_group").1, "unset");
}

// ---- the two pairing settings of a noise-protocol method ----

#[test]
fn an_optional_timeout_defaults_to_some_and_reads_never_as_none() {
    assert_eq!(
        PairingSettings::default().first_message_timeout,
        Some(Duration::from_secs(60))
    );
    // A document that leaves the key out reads the default, not `None`.
    assert_eq!(
        built::<PairingSettings>(&[]).first_message_timeout,
        Some(Duration::from_secs(60))
    );
    assert_eq!(
        built::<PairingSettings>(&[("first_message_timeout", "never")]).first_message_timeout,
        None
    );
    for (text, want) in [("30", 30_000), ("0.25", 250), ("1500ms", 1500)] {
        assert_eq!(
            built::<PairingSettings>(&[("first_message_timeout", text)]).first_message_timeout,
            Some(Duration::from_millis(want)),
            "{text}"
        );
    }
    assert!(
        Partial::<PairingSettings>::new()
            .set("first_message_timeout", "soon")
            .is_err()
    );

    // Reported under its key when it differs, and as `never` when there is none.
    let found = non_defaults(&PairingSettings {
        first_message_timeout: Some(Duration::from_secs(30)),
        ..PairingSettings::default()
    });
    assert_eq!(names(&found), ["first_message_timeout"]);
    assert_eq!(
        shown(&found, "first_message_timeout"),
        ("Duration(30s)".to_owned(), "Duration(60s)".to_owned())
    );
    let found = non_defaults(&PairingSettings {
        first_message_timeout: None,
        ..PairingSettings::default()
    });
    assert_eq!(shown(&found, "first_message_timeout").0, "Duration(never)");
}

#[test]
fn the_noise_key_limit_and_rekeying_are_plain_numbers_and_flags() {
    let found = non_defaults(&NoiseSettings {
        key_limit: 1000,
        rekey: false,
    });
    assert_eq!(names(&found), ["key_limit", "rekey"]);
    assert_eq!(
        shown(&found, "key_limit"),
        (
            "u64(1000)".to_owned(),
            format!("u64({})", noise::DEFAULT_KEY_LIMIT)
        )
    );
    assert_eq!(
        shown(&found, "rekey"),
        ("bool(false)".to_owned(), "bool(true)".to_owned())
    );
}

// ---- a setting written as a flag or as a table ----

#[test]
fn a_cache_is_a_flag_or_a_table() {
    let settings = built::<ProviderSettings>(&[("directory", "d")]);
    assert_eq!(settings.cache, CacheSetting::Enabled(true));

    let settings = built::<ProviderSettings>(&[("directory", "d"), ("cache", "false")]);
    assert_eq!(settings.cache, CacheSetting::Enabled(false));

    let settings = built::<ProviderSettings>(&[("directory", "d"), ("cache.ttl", "30s")]);
    assert_eq!(
        settings.cache,
        CacheSetting::Tuned(CacheOptions {
            ttl: Duration::from_secs(30),
            ..CacheOptions::default()
        })
    );

    // Through the environment, the pair `APP_CACHE=false` and `APP_CACHE__TTL=30s`.
    let mut flag = partial::<ProviderSettings>(&[("directory", "d")]);
    flag.fill_from(&mut env("APP", [("APP_CACHE", "false")]))
        .unwrap();
    assert_eq!(flag.build().unwrap().cache, CacheSetting::Enabled(false));

    let mut table = partial::<ProviderSettings>(&[("directory", "d")]);
    table
        .fill_from(&mut env("APP", [("APP_CACHE__TTL", "30s")]))
        .unwrap();
    assert!(matches!(
        table.build().unwrap().cache,
        CacheSetting::Tuned(options) if options.ttl == Duration::from_secs(30)
    ));

    let mut both = partial::<ProviderSettings>(&[("directory", "d")]);
    assert!(
        both.fill_from(&mut env(
            "APP",
            [("APP_CACHE", "false"), ("APP_CACHE__TTL", "30s")]
        ))
        .is_err(),
        "a flag and a table together are refused"
    );
}

#[test]
fn the_non_defaults_of_a_cache_name_the_arm_that_differs() {
    let off = ProviderSettings {
        directory: "d".to_owned(),
        cache: CacheSetting::Enabled(false),
    };
    let found = non_defaults(&off);
    assert_eq!(names(&found), ["cache", "directory"]);
    assert_eq!(
        shown(&found, "cache"),
        (
            "CacheSetting(false)".to_owned(),
            "CacheSetting(true)".to_owned()
        )
    );

    let tuned = ProviderSettings {
        directory: "d".to_owned(),
        cache: CacheSetting::Tuned(CacheOptions {
            max_entries: 5,
            ..CacheOptions::default()
        }),
    };
    let found = non_defaults(&tuned);
    assert_eq!(names(&found), ["cache", "directory"]);
}

// ---- an ordered rule list ----

#[test]
fn a_list_of_rules_reads_under_the_key_rule() {
    let acl = built::<Acl>(&[
        ("rule[0].action", "permit"),
        ("rule[0].source", "10.0.0.0/8"),
        ("rule[0].port", "80-90"),
        ("rule[0].log", "true"),
        ("rule[1].action", "deny"),
        ("default", "permit"),
    ]);
    assert_eq!(acl.default, Action::Permit);
    assert_eq!(acl.rules.len(), 2);
    assert_eq!(acl.rules[0].source, Net::V4([10, 0, 0, 0].into(), 8));
    assert_eq!(acl.rules[0].port, Some(PortRange { start: 80, end: 90 }));
    assert!(acl.rules[0].log);
    assert_eq!(
        acl.rules[1].source,
        Net::Any,
        "a rule's source defaults to any"
    );
    assert_eq!(acl.rules[1].destination, None);

    let mut p = Partial::<Acl>::new();
    assert!(p.set("rule[0].port", "90-80").is_err());
    assert!(p.set("rule[0].source", "10.0.0.0/33").is_err());
    assert!(p.set("rule[0].action", "allow").is_err());
    assert!(
        p.set("rules[0].action", "permit").is_err(),
        "the key is `rule`"
    );
    assert!(
        built_err::<Acl>(&[("rule[0].source", "10.0.0.0/8")]).contains("rule[0].action"),
        "a rule without its action is missing it"
    );
}

fn built_err<T: Describe>(pairs: &[(&str, &str)]) -> String {
    partial::<T>(pairs)
        .build()
        .err()
        .expect("a key is missing")
        .to_string()
}

#[test]
fn the_non_defaults_of_a_list_are_the_default_action_and_the_rules_given() {
    assert!(non_defaults(&Acl::default()).is_empty());
    let open = Acl {
        default: Action::Permit,
        ..Acl::default()
    };
    let found = non_defaults(&open);
    assert_eq!(names(&found), ["default"]);
    assert_eq!(
        shown(&found, "default"),
        ("Action(permit)".to_owned(), "Action(deny)".to_owned())
    );
}

// ---- a profile over a transport-security policy ----

fn modern_profile() -> Profile {
    Profile::new()
        .expecting("tls_versions", Expect::SubsetOf(texts(&["1.3"])))
        .unwrap()
        .expecting("cipher_profiles", Expect::ItemsEqual(texts(&["mozilla"])))
        .unwrap()
}

#[test]
fn a_hand_written_modern_profile_is_met_by_the_modern_preset_and_missed_by_the_other() {
    let profile = modern_profile();
    profile.bind::<Policy>().unwrap();
    assert!(profile.verify(&Policy::modern()).is_met());

    let report = profile.verify(&Policy::intermediate());
    assert!(!report.is_met());
    let unmet: Vec<_> = report.unmet().collect();
    assert_eq!(unmet.len(), 1);
    assert_eq!(unmet[0].key().to_string(), "tls_versions");
    assert!(matches!(unmet[0].reason(), Some(Reason::NotSubset)));

    // Legacy allows 1.2 as well, and a profile of another profile list misses.
    assert!(!profile.verify(&Policy::legacy()).is_met());
    let mut policy = Policy::modern();
    policy.cipher_profiles = vec![CipherProfile::Cnsa10];
    let report = profile.verify(&policy);
    assert_eq!(
        report
            .unmet()
            .map(|f| f.key().to_string())
            .collect::<Vec<_>>(),
        ["cipher_profiles"]
    );
}

#[test]
fn a_profile_naming_a_value_the_type_refuses_is_refused_when_bound() {
    let profile = Profile::new()
        .expecting("tls_versions", Expect::SubsetOf(texts(&["1.1"])))
        .unwrap();
    assert!(profile.bind::<Policy>().is_err());
}

// ---- the builders ----

fn identity() -> Identity {
    Identity("srv".to_owned())
}

fn unique_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "coqui-builder-shapes-{}-{label}",
        std::process::id()
    ))
}

fn missing_of<T: core::fmt::Debug>(
    result: Result<T, coqui_builder::builder::BuildError<PeerError>>,
) -> coqui_builder::Missing {
    match result {
        Err(coqui_builder::builder::BuildError::Missing(missing)) => missing,
        other => panic!("expected Missing, got {other:?}"),
    }
}

#[test]
fn every_setter_and_input_of_a_server_builder_reaches_build() {
    let identity = identity();
    let built = ServerBuilder::new()
        .identity(&identity)
        .client_auth(ClientAuth::Required(Trust("pins".to_owned())))
        .alpn(vec!["h2".parse::<Alpn>().unwrap()])
        .policy(Policy::modern())
        .build()
        .unwrap();
    assert_eq!(built.config.identity, "srv");
    assert_eq!(
        built.config.client_auth,
        ClientAuth::Required(Trust("pins".to_owned()))
    );
    assert_eq!(built.config.alpn, [b"h2".to_vec()]);
    assert_eq!(built.config.tls_versions, 1);
    assert_eq!(built.settings.policy, Policy::modern());
}

#[test]
fn an_empty_server_builder_names_every_required_input() {
    let missing = missing_of(ServerBuilder::new().build());
    assert!(missing.keys().is_empty());
    assert_eq!(missing.inputs(), ["identity", "client_auth"]);
    assert_eq!(missing.to_string(), "missing inputs: identity, client_auth");
}

#[test]
fn a_second_terminal_over_parts_agrees_with_build() {
    let identity = identity();
    let make = || {
        ServerBuilder::new()
            .identity(&identity)
            .client_auth(ClientAuth::Required(Trust("pins".to_owned())))
            .alpn(vec!["h2".parse::<Alpn>().unwrap()])
            .policy(Policy::modern())
    };
    let built = make().build().unwrap();
    let (server, client) = make().configs().unwrap();
    assert_eq!(server, built.config);
    assert_eq!(client.alpn, built.config.alpn);
    assert_eq!(client.require_ems, built.settings.policy.require_ems);

    let missing = missing_of(ServerBuilder::new().configs());
    assert_eq!(missing, missing_of(ServerBuilder::new().build()));
}

#[test]
fn a_server_builder_layers_a_partial_then_the_environment_then_setters() {
    let identity = identity();
    let mut file = Partial::<TlsSettings>::new();
    file.set("alpn[0]", "h2").unwrap();
    file.set("policy.waiting.total", "9").unwrap();
    file.set("policy.require_ems", "true").unwrap();
    let mut variables = env("TLS", [("TLS_POLICY__REQUIRE_EMS", "false")]);
    let built = ServerBuilder::new()
        .fill(file)
        .fill_from(&mut variables)
        .unwrap()
        .identity(&identity)
        .client_auth(ClientAuth::None)
        .build()
        .unwrap();
    assert_eq!(built.config.alpn, [b"h2".to_vec()]);
    assert_eq!(built.settings.policy.waiting.total, 9);
    assert!(
        !built.settings.policy.require_ems,
        "the environment is the later layer"
    );

    // A setter after a layer wins over it: the policy is replaced whole.
    let mut file = Partial::<TlsSettings>::new();
    file.set("policy.waiting.total", "9").unwrap();
    let built = ServerBuilder::new()
        .fill(file)
        .policy(Policy::modern())
        .identity(&identity)
        .client_auth(ClientAuth::None)
        .build()
        .unwrap();
    assert_eq!(built.settings.policy.waiting.total, 1_000);
}

#[test]
fn a_server_builder_keeps_the_error_of_the_built_type() {
    let identity = identity();
    let error = ServerBuilder::new()
        .identity(&identity)
        .client_auth(ClientAuth::None)
        .policy(Policy {
            tls_versions: Vec::new(),
            ..Policy::default()
        })
        .build()
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        PeerError::Policy("no TLS version is allowed".to_owned()).to_string()
    );
    assert!(matches!(
        error,
        coqui_builder::builder::BuildError::Failed(PeerError::Policy(_))
    ));
}

#[test]
fn every_setter_and_input_of_a_client_builder_reaches_build() {
    let identity = identity();
    let built = ClientBuilder::new()
        .server(Trust("roots".to_owned()))
        .identity(&identity)
        .alpn(vec!["app/1".parse::<Alpn>().unwrap()])
        .policy(Policy::legacy())
        .build()
        .unwrap();
    assert_eq!(
        built.config.server,
        ServerAuth::Trust(Trust("roots".to_owned()))
    );
    assert_eq!(built.config.identity.as_deref(), Some("srv"));
    assert_eq!(built.config.alpn, [b"app/1".to_vec()]);
    assert!(!built.config.require_ems);

    // The identity is optional: a client may present none.
    let built = ClientBuilder::new()
        .server(Trust("roots".to_owned()))
        .build()
        .unwrap();
    assert_eq!(built.config.identity, None);
    assert!(built.config.require_ems);

    let missing = missing_of(ClientBuilder::new().build());
    assert_eq!(
        missing.inputs(),
        ["server"],
        "an optional input is not missing"
    );
}

#[test]
fn the_non_defaults_of_a_builder_are_those_of_what_it_built() {
    let identity = identity();
    let mut policy = Policy::modern();
    policy.allow_resumption = true;
    let builder = ServerBuilder::new()
        .identity(&identity)
        .client_auth(ClientAuth::None)
        .alpn(vec!["h2".parse::<Alpn>().unwrap()])
        .policy(policy);
    let seen = builder.non_defaults();
    // The settings are `alpn` and `policy`: the paths gain the key of the group.
    assert_eq!(
        names(&seen),
        ["alpn", "policy.allow_resumption", "policy.tls_versions"]
    );
    assert_eq!(
        shown(&seen, "policy.allow_resumption"),
        ("bool(true)".to_owned(), "bool(false)".to_owned())
    );
    let built = builder.build().unwrap();
    assert_eq!(built.non_defaults(), seen);
    assert_eq!(Configured::non_defaults(&built), seen);

    // A fresh builder has none.
    assert!(ServerBuilder::new().non_defaults().is_empty());
}

#[test]
fn a_profile_checks_a_builder_as_it_checks_what_it_built() {
    let profile = Profile::new()
        .expecting("policy.tls_versions", Expect::SubsetOf(texts(&["1.3"])))
        .unwrap();
    let identity = identity();
    let builder = ServerBuilder::new()
        .identity(&identity)
        .client_auth(ClientAuth::None)
        .policy(Policy::intermediate());
    let judge = |subject: &dyn Fn(&mut coqui_builder::Report)| {
        let mut report = coqui_builder::Report::new(&TlsSettings::description());
        subject(&mut report);
        report
    };
    let report = judge(&|report| profile.check(&builder, report));
    let unmet: Vec<String> = report.unmet().map(|f| f.key().to_string()).collect();
    assert_eq!(unmet, ["policy.tls_versions"]);

    let modern = builder.policy(Policy::modern());
    assert!(judge(&|report| profile.check(&modern, report)).is_met());
    let built = modern.build().unwrap();
    assert!(judge(&|report| profile.check(&built, report)).is_met());
}

#[test]
fn a_trust_store_builder_takes_its_directory_as_a_key_and_a_clock_as_an_option() {
    let dir = unique_dir("store");
    let store = TrustStore::builder()
        .state_dir(dir.clone())
        .rotation(store::RotationOptions {
            require_approval: true,
            ..store::RotationOptions::default()
        })
        .clock(Clock("fixed"))
        .build()
        .unwrap();
    assert_eq!(store.root, dir.join("peers"));
    assert_eq!(store.clock, Clock("fixed"));
    assert!(store.settings.rotation.require_approval);
    assert!(dir.join("peers").join("accepted").is_dir());

    let default_clock = TrustStore::builder()
        .state_dir(dir.clone())
        .build()
        .unwrap();
    assert_eq!(default_clock.clock, Clock::system());
    std::fs::remove_dir_all(&dir).unwrap();

    let missing = missing_of(TrustStore::builder().build());
    assert_eq!(missing.to_string(), "missing settings: state_dir");
    assert!(missing.inputs().is_empty(), "the clock is optional");
}

#[test]
fn the_nested_setters_of_a_trust_store_write_the_keys_below_the_top_level() {
    let dir = unique_dir("nested");
    let builder = TrustStore::builder()
        .state_dir(dir.clone())
        .limits(WaitingLimits {
            per_source: 32,
            total: 50,
            rerecord: Duration::from_secs(5),
        })
        .ipv6_prefix(48)
        .when_full(WhenFull::ReplaceOldest)
        .record_interval(Duration::from_millis(250));
    let seen = builder.non_defaults();
    assert_eq!(
        names(&seen),
        [
            "state_dir",
            "waiting.ipv6_prefix",
            "waiting.limits.per_source",
            "waiting.limits.rerecord",
            "waiting.limits.total",
            "waiting.record_interval",
            "waiting.when_full",
        ]
    );
    let store = builder.build().unwrap();
    let waiting = store.settings.waiting;
    assert_eq!(waiting.limits.per_source, 32);
    assert_eq!(waiting.ipv6_prefix, 48);
    assert_eq!(waiting.when_full, WhenFull::ReplaceOldest);
    assert_eq!(waiting.record_interval, Duration::from_millis(250));
    assert_eq!(store.non_defaults(), seen);
    std::fs::remove_dir_all(&dir).unwrap();

    // The last write of a key wins, whichever way it was written.
    let store = TrustStore::builder()
        .state_dir(dir.clone())
        .limits(WaitingLimits::default())
        .waiting(store::WaitingSettings {
            ipv6_prefix: 56,
            ..store::WaitingSettings::default()
        })
        .ipv6_prefix(40)
        .build()
        .unwrap();
    assert_eq!(store.settings.waiting.ipv6_prefix, 40);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_trust_store_builder_reports_the_error_of_its_directory() {
    let file = unique_dir("a-file");
    std::fs::write(&file, b"x").unwrap();
    let error = TrustStore::builder()
        .state_dir(file.join("below"))
        .build()
        .unwrap_err();
    assert!(
        matches!(
            error,
            coqui_builder::builder::BuildError::Failed(PeerError::Io(_))
        ),
        "{error}"
    );
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn a_listener_builder_takes_the_name_as_an_input_and_every_setting_as_a_setter() {
    let name = PipeName("local".to_owned());
    let listener = Listener::builder()
        .name(&name)
        .socket_mode(0o660u32)
        .read_timeout(None::<Duration>)
        .write_timeout(Duration::from_secs(2))
        .backlog(8)
        .build()
        .unwrap();
    assert_eq!(listener.name, name);
    assert_eq!(listener.settings.socket_mode, 0o660);
    assert_eq!(listener.settings.read_timeout, None);
    assert_eq!(
        listener.settings.write_timeout,
        Some(Duration::from_secs(2))
    );
    assert_eq!(listener.settings.backlog, 8);
    assert_eq!(
        names(&listener.non_defaults()),
        ["backlog", "read_timeout", "socket_mode", "write_timeout"]
    );
    let missing = Listener::builder().build().unwrap_err();
    assert_eq!(missing.inputs(), ["name"]);
}

#[test]
fn a_partial_read_from_a_file_is_a_builders_whole_settings() {
    let name = PipeName("local".to_owned());
    let file = partial::<ListenerSettings>(&[("remove_stale", "false")]);
    let listener = Listener::builder().fill(file).name(&name).build().unwrap();
    assert!(!listener.settings.remove_stale);
}

// ---- a channel over a stream, and the settings of a WebSocket ----

struct Lines;

impl builders::Codec for Lines {
    fn name(&self) -> &'static str {
        "lines"
    }
}

struct Pipe(&'static str);

impl builders::Split for Pipe {
    fn split(self) -> std::io::Result<(String, String)> {
        if self.0.is_empty() {
            return Err(std::io::Error::other("nothing to split"));
        }
        Ok((format!("{}-read", self.0), format!("{}-write", self.0)))
    }
}

#[test]
fn a_stream_channel_builder_takes_its_codec_and_stream_as_generic_inputs() {
    use transport::stream::{
        DEFAULT_MAX_MESSAGE, StreamChannelSettings, StreamChannelSettingsSetters,
    };

    let channel = builders::StreamChannelBuilder::<Lines, Pipe>::new()
        .codec(Lines)
        .stream(Pipe("tcp"))
        .max_message(1024usize)
        .linger(Duration::ZERO)
        .build()
        .unwrap();
    assert_eq!(channel.codec, "lines");
    assert_eq!(channel.halves.0, "tcp-read");
    assert_eq!(channel.settings.max_message, 1024);
    assert_eq!(channel.settings.pending, 64);
    assert_eq!(channel.settings.linger, Duration::ZERO);
    assert_eq!(names(&channel.non_defaults()), ["linger", "max_message"]);
    assert_eq!(
        shown(&channel.non_defaults(), "max_message"),
        (
            "usize(1024)".to_owned(),
            format!("usize({DEFAULT_MAX_MESSAGE})")
        )
    );

    let whole = StreamChannelSettings {
        pending: 4,
        ..StreamChannelSettings::new()
    };
    let channel = builders::StreamChannelBuilder::<Lines, Pipe>::new()
        .codec(Lines)
        .stream(Pipe("tcp"))
        .fill(Partial::from(whole))
        .build()
        .unwrap();
    assert_eq!(channel.settings.pending, 4);

    // The step that failed is the stream's, in the crate's own error.
    let error = builders::StreamChannelBuilder::<Lines, Pipe>::new()
        .codec(Lines)
        .stream(Pipe(""))
        .build()
        .unwrap_err();
    assert!(matches!(
        error,
        coqui_builder::builder::BuildError::Failed(_)
    ));

    let missing = match builders::StreamChannelBuilder::<Lines, Pipe>::new().build() {
        Err(coqui_builder::builder::BuildError::Missing(missing)) => missing,
        other => panic!("expected Missing, got {:?}", other.map(|_| ())),
    };
    assert_eq!(missing.inputs(), ["codec", "stream"]);
}

#[test]
fn generic_inputs_are_inferred_with_no_turbofish_and_settings_set_first() {
    use transport::stream::StreamChannelSettingsSetters;

    // The settings come before the inputs, so the type parameters are open until the end.
    let channel = builders::StreamChannelBuilder::new()
        .max_message(2048usize)
        .linger(Duration::from_secs(1))
        .codec(Lines)
        .stream(Pipe("unix"))
        .build()
        .unwrap();
    assert_eq!(channel.codec, "lines");
    assert_eq!(channel.halves.1, "unix-write");
    assert_eq!(channel.settings.max_message, 2048);

    // Inputs first, then a setting.
    let channel = builders::StreamChannelBuilder::new()
        .stream(Pipe("tcp"))
        .codec(Lines)
        .pending(8usize)
        .build()
        .unwrap();
    assert_eq!(channel.settings.pending, 8);
}

#[test]
fn the_settings_of_a_websocket_fill_from_the_keys_a_file_uses() {
    use transport::web::{DialerSettings, Frames, Limits, ListenerSettings, Signaling};

    assert_eq!(
        keys::<DialerSettings>(),
        [
            "frames",
            "datagrams",
            "limits",
            "plain_beyond_loopback",
            "local_addr",
            "webrtc"
        ]
    );
    let dialer = built::<DialerSettings>(&[
        ("frames", "binary"),
        ("limits.handshake", "2s"),
        ("local_addr", "127.0.0.1:0"),
        ("webrtc.signaling", "by-caller"),
        ("webrtc.public_address", "192.0.2.1:4433"),
    ]);
    assert_eq!(dialer.frames, Frames::Binary);
    assert_eq!(dialer.limits.handshake, Duration::from_secs(2));
    assert_eq!(dialer.limits.pending, 64);
    assert_eq!(dialer.local_addr, Some("127.0.0.1:0".parse().unwrap()));
    let rtc = dialer
        .webrtc
        .expect("the group is present once a key below it is given");
    assert_eq!(rtc.signaling, Signaling::ByCaller);
    assert!(rtc.enabled, "the rest of the group keeps its defaults");
    assert_eq!(built::<DialerSettings>(&[]).webrtc, None);

    let listener = built::<ListenerSettings>(&[
        ("origins[0]", "https://a.example"),
        ("origins[1]", "https://b.example"),
        ("limits.idle", "2m"),
    ]);
    assert_eq!(listener.origins, ["https://a.example", "https://b.example"]);
    assert_eq!(listener.limits.idle, Duration::from_secs(120));
    assert_eq!(
        listener.limits,
        Limits {
            idle: Duration::from_secs(120),
            ..Limits::default()
        }
    );

    let mut p = Partial::<DialerSettings>::new();
    assert!(p.set("frames", "Binary").is_err());
    assert!(p.set("local_addr", "not an address").is_err());
    assert!(p.set("webrtc.signaling", "onchannel").is_err());
}

#[test]
fn the_non_defaults_of_a_websocket_dialer_name_the_nested_keys() {
    use transport::web::{DialerSettings, Frames, Limits, Settings};

    assert!(non_defaults(&DialerSettings::default()).is_empty());
    let settings = DialerSettings {
        frames: Frames::Binary,
        limits: Limits {
            send: Duration::from_secs(5),
            ..Limits::default()
        },
        local_addr: Some("127.0.0.1:0".parse().unwrap()),
        webrtc: Some(Settings::default()),
        ..DialerSettings::default()
    };
    let found = non_defaults(&settings);
    assert_eq!(
        names(&found),
        ["frames", "limits.send", "local_addr", "webrtc"]
    );
    assert_eq!(
        shown(&found, "frames"),
        ("Frames(binary)".to_owned(), "Frames(text)".to_owned())
    );
    assert_eq!(
        shown(&found, "local_addr"),
        ("SocketAddr(127.0.0.1:0)".to_owned(), "unset".to_owned())
    );
}

#[test]
fn a_group_defaulted_by_an_expression_gives_each_key_its_value_from_that_default() {
    use store::{CodeAlphabet, PairingDeskSettings, PairingOptions};

    let desk = built::<PairingDeskSettings>(&[]);
    assert_eq!(desk.options.pin_alphabet, CodeAlphabet::pin());
    assert_eq!(desk, PairingDeskSettings::default());
    assert!(non_defaults(&desk).is_empty());

    // The group's keys are required inside the group, yet giving one keeps the other's
    // default: a table that names only `len` is not missing `chars`.
    let desk = built::<PairingDeskSettings>(&[("options.pin_alphabet.len", "4")]);
    assert_eq!(desk.options.pin_alphabet.chars, "0123456789");
    assert_eq!(desk.options.pin_alphabet.len, 4);
    assert_eq!(
        desk.options.user_code_alphabet,
        CodeAlphabet::user_code(),
        "the other group is untouched"
    );

    let changed = PairingDeskSettings {
        options: PairingOptions {
            pin_alphabet: CodeAlphabet {
                chars: "ab".to_owned(),
                len: 3,
            },
            ..PairingOptions::default()
        },
        ..PairingDeskSettings::default()
    };
    let found = non_defaults(&changed);
    assert_eq!(
        names(&found),
        ["options.pin_alphabet.chars", "options.pin_alphabet.len"]
    );
    assert_eq!(
        shown(&found, "options.pin_alphabet.len"),
        ("usize(3)".to_owned(), "usize(6)".to_owned())
    );
}

fn non_defaults_against<T: Describe + coqui_builder::Inspect>(
    value: &T,
    baseline: &T,
) -> Vec<NonDefault> {
    coqui_builder::differences(&T::description(), value, Some(baseline))
        .findings()
        .iter()
        .map(NonDefault::from)
        .collect()
}

#[test]
fn a_policy_is_compared_with_a_baseline_that_is_not_the_default() {
    // `modern` against the default reports the versions; the default against `modern` reports
    // them the other way round, as the value and the baseline's default.
    let found = non_defaults_against(&Policy::default(), &Policy::modern());
    assert_eq!(names(&found), ["tls_versions"]);
    assert_eq!(
        shown(&found, "tls_versions"),
        (
            "[TlsVersion(1.2), TlsVersion(1.3)]".to_owned(),
            "[TlsVersion(1.3)]".to_owned()
        )
    );
    assert_eq!(
        found[0].to_string(),
        "tls_versions = [TlsVersion(1.2), TlsVersion(1.3)] (default [TlsVersion(1.3)])"
    );
    assert!(non_defaults_against(&Policy::modern(), &Policy::modern()).is_empty());

    let mut weakened = Policy::modern();
    weakened.allow_chain_when_pinned = true;
    weakened.waiting.total = 9;
    let found = non_defaults_against(&weakened, &Policy::modern());
    assert_eq!(names(&found), ["allow_chain_when_pinned", "waiting.total"]);
}

// ---- a bound that is a count or no bound ----

#[test]
fn a_limit_is_a_leaf_by_hand_and_a_bound_is_lifted_with_unbounded() {
    use limits::{Limit, RoleLimits};

    coqui_builder::describe::assert_round_trip(&[Limit::at(0), Limit::at(256), Limit::UNBOUNDED]);
    assert_eq!(built::<RoleLimits>(&[]), RoleLimits::default());
    let lifted = built::<RoleLimits>(&[("roles", "unbounded"), ("rules", "50")]);
    assert_eq!(lifted.roles, Limit::UNBOUNDED);
    assert_eq!(lifted.rules, Limit::at(50));
    assert_eq!(lifted.ancestors, Limit::at(64));
    let mut p = Partial::<RoleLimits>::new();
    assert!(p.set("roles", "null").is_err());
    assert!(p.set("roles", "-1").is_err());
    assert!(p.set("roles", "none").is_err());

    // A lifted bound is listed against its default, shown by its own text.
    let found = non_defaults(&lifted);
    assert_eq!(names(&found), ["roles", "rules"]);
    assert_eq!(
        shown(&found, "roles"),
        ("Limit(unbounded)".to_owned(), "Limit(256)".to_owned())
    );
}

#[test]
fn a_byte_string_held_as_text_reads_back_as_itself_whatever_its_bytes() {
    use tls::alpn_text::{from_text, to_text};

    for bytes in [
        b"h2".to_vec(),
        b"example-pair/1".to_vec(),
        vec![0x68, 0xff, 0x20, b'%'],
        vec![0, 1, 2, 0x7f, 0x80],
        "naive".as_bytes().to_vec(),
        "na\u{ef}ve".as_bytes().to_vec(),
    ] {
        assert_eq!(from_text(&to_text(&bytes)).unwrap(), bytes, "{bytes:?}");
    }
    assert_eq!(to_text(b"h2"), "h2", "a plain name is its own text");
    assert_eq!(to_text(&[0x68, 0xff, 0x20]), "h%FF%20");
    // The text of a name that was valid before still reads, unless it holds an escape.
    assert_eq!(from_text("na\u{ef}ve").unwrap(), "na\u{ef}ve".as_bytes());
    for bad in ["%", "%4", "%zz", "%4g", "%+1"] {
        assert!(from_text(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_ttl_that_lasted_for_good_as_the_largest_duration_is_written_never() {
    use store::{PairingDeskSettings, PairingOptions};

    let desk = built::<PairingDeskSettings>(&[("options.pin_ttl", "never")]);
    assert_eq!(desk.options.pin_ttl, None);
    assert_eq!(desk.options.device_ttl, Some(Duration::from_secs(600)));
    let desk = built::<PairingDeskSettings>(&[("options.device_ttl", "90s")]);
    assert_eq!(desk.options.device_ttl, Some(Duration::from_secs(90)));
    assert_eq!(
        keys::<PairingOptions>(),
        [
            "invitation",
            "pin",
            "pin_shown",
            "device",
            "pin_alphabet",
            "pin_ttl",
            "pin_attempts",
            "user_code_alphabet",
            "device_ttl"
        ]
    );
    assert_eq!(
        keys::<store::DeskLimits>(),
        [
            "pending",
            "pending_per_source",
            "pin_runs_per_source",
            "wrong_codes_per_approver",
            "approver_window",
            "wrong_codes_total",
            "ipv6_prefix"
        ]
    );
    let found = non_defaults(&desk);
    assert_eq!(names(&found), ["options.device_ttl"]);
    let forever = PairingDeskSettings {
        options: PairingOptions {
            pin_ttl: None,
            ..PairingOptions::default()
        },
        ..PairingDeskSettings::default()
    };
    assert_eq!(
        shown(&non_defaults(&forever), "options.pin_ttl"),
        ("Duration(never)".to_owned(), "Duration(600s)".to_owned())
    );
}

#[test]
fn two_tuned_caches_differ_by_the_keys_that_differ() {
    let tuned = |ttl: u64, max_entries: u64| ProviderSettings {
        directory: "d".to_owned(),
        cache: CacheSetting::Tuned(CacheOptions {
            ttl: Duration::from_secs(ttl),
            max_entries,
            ..CacheOptions::default()
        }),
    };
    let report = coqui_builder::differences(
        &ProviderSettings::description(),
        &tuned(30, 5),
        Some(&tuned(30, 8)),
    );
    let found: Vec<NonDefault> = report.findings().iter().map(NonDefault::from).collect();
    assert_eq!(names(&found), ["cache.max_entries"]);
    assert_eq!(
        shown(&found, "cache.max_entries"),
        ("u64(5)".to_owned(), "u64(8)".to_owned())
    );

    // Against a flag, the whole cache is one place, shown as the keys the tuned side holds.
    let report = coqui_builder::differences(
        &ProviderSettings::description(),
        &tuned(30, 5),
        Some(&ProviderSettings {
            directory: "d".to_owned(),
            cache: CacheSetting::Enabled(false),
        }),
    );
    let found: Vec<NonDefault> = report.findings().iter().map(NonDefault::from).collect();
    assert_eq!(names(&found), ["cache"]);
    assert!(
        found[0]
            .value
            .to_string()
            .starts_with("{ttl: Duration(30s)"),
        "{}",
        found[0].value
    );
}
