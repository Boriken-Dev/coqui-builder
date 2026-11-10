// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Untagged enums with text arms and one structured arm: the shared vectors, how the slot
//! switches arm, and how every walk (inspect, overlay, non-defaults, differences, profiles,
//! redaction) sees the two sides.

use std::time::Duration;

use coqui_builder::describe::Kind;
use coqui_builder::fill::FillErrorKind;
use coqui_builder::{
    Check, Datum, Describe, Entries, Env, Expect, Inspect, KeyPath, Partial, Profile, Reason,
    Segment, Shown, Source, SourceError, differences, non_defaults,
};
use serde_json::{Value, json};

// ---- the mirrors ----

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(deny_unknown_fields)]
struct CacheOptions {
    #[map(default = Duration::from_secs(60))]
    ttl: Duration,
    #[map(default = 1024)]
    size: u32,
    #[schema(sensitive)]
    token: Option<String>,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum CacheSetting {
    Enabled(bool),
    Tuned(CacheOptions),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Cached {
    #[map(default = CacheSetting::Enabled(true))]
    cache: CacheSetting,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Definition {
    name: String,
    #[map(default = 3)]
    level: u8,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged, rename_all = "lowercase")]
enum Switch {
    On,
    Off,
    Custom(Definition),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Vetted {
    vetting: Switch,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Preset {
    #[map(default = Switch::Custom(Definition { name: "d".into(), level: 5 }))]
    vetting: Switch,
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Many {
    switches: Vec<Switch>,
    maybe: Option<CacheSetting>,
}

fn key(text: &str) -> Vec<Segment> {
    text.parse::<KeyPath>().unwrap().segments().to_vec()
}

fn tuned(ttl: u64, size: u32) -> CacheSetting {
    CacheSetting::Tuned(CacheOptions {
        ttl: Duration::from_secs(ttl),
        size,
        token: None,
    })
}

// ---- a source written by hand over a JSON document ----

struct Json(Value);

impl Json {
    fn node(&self, at: &KeyPath) -> Option<&Value> {
        let mut node = &self.0;
        for segment in at.segments() {
            node = match segment {
                Segment::Key(key) => node.get(key.as_str())?,
                Segment::Index(index) => node.get(*index)?,
            };
        }
        Some(node)
    }

    fn walk(
        &self,
        at: &KeyPath,
        node: &Value,
        known: &dyn Fn(&KeyPath) -> bool,
        out: &mut Vec<KeyPath>,
    ) {
        if let Value::Object(map) = node {
            for (name, child) in map {
                let path = at.child(Segment::Key(name.clone()));
                if known(&path) {
                    self.walk(&path, child, known, out);
                } else {
                    out.push(path);
                }
            }
        }
    }
}

impl Source for Json {
    fn value(
        &mut self,
        at: &KeyPath,
        _leaf: &coqui_builder::describe::LeafInfo,
    ) -> Result<Option<String>, SourceError> {
        match self.node(at) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(Value::Bool(flag)) => Ok(Some(flag.to_string())),
            Some(Value::Number(number)) => Ok(Some(number.to_string())),
            Some(_) => Err(SourceError::new("a table where a text is expected")),
        }
    }

    fn entries(&mut self, at: &KeyPath) -> Result<Option<Entries>, SourceError> {
        Ok(match self.node(at) {
            Some(Value::Object(map)) => Some(Entries::Keys(map.keys().cloned().collect())),
            Some(Value::Array(items)) => Some(Entries::Len(items.len())),
            _ => None,
        })
    }

    fn unknown(&mut self, known: &dyn Fn(&KeyPath) -> bool) -> Vec<KeyPath> {
        let mut out = Vec::new();
        self.walk(&KeyPath::root(), &self.0, known, &mut out);
        out
    }
}

// ---- the vectors ----

fn render_cached(value: &Cached) -> Value {
    match &value.cache {
        CacheSetting::Enabled(flag) => json!({"enabled": flag}),
        CacheSetting::Tuned(options) => json!({"tuned": {
            "ttl_secs": options.ttl.as_secs(),
            "size": options.size,
            "token": options.token,
        }}),
    }
}

fn render_vetted(value: &Vetted) -> Value {
    match &value.vetting {
        Switch::On => json!("on"),
        Switch::Off => json!("off"),
        Switch::Custom(definition) => {
            json!({"custom": {"name": definition.name, "level": definition.level}})
        }
    }
}

/// The outcome of a case as JSON: `{"built": ..}`, `{"missing": [..]}` or `{"error": message}`.
fn run<T: Describe>(layers: &[Value], render: impl Fn(&T) -> Value) -> Value {
    let mut partial = Partial::<T>::new();
    for layer in layers {
        let outcome = if let Some(document) = layer.get("json") {
            partial.fill_from(&mut Json(document.clone()))
        } else {
            let vars = layer["env"]
                .as_object()
                .expect("an env layer is an object")
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()));
            partial.fill_from(&mut Env::from_vars("APP", vars))
        };
        if let Err(error) = outcome {
            return json!({"error": error.to_string()});
        }
    }
    match partial.build() {
        Ok(value) => json!({"built": render(&value)}),
        Err(missing) => {
            json!({"missing": missing.keys().iter().map(ToString::to_string).collect::<Vec<_>>()})
        }
    }
}

#[test]
fn the_vectors_hold() {
    let file: Value = serde_json::from_str(include_str!("vectors/union.json")).unwrap();
    let cases = file["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for case in cases {
        let layers = case["layers"].as_array().unwrap();
        let got = match case["root"].as_str().unwrap() {
            "Cached" => run::<Cached>(layers, render_cached),
            "Vetted" => run::<Vetted>(layers, render_vetted),
            other => panic!("no root type {other}"),
        };
        let want = &case["expect"];
        let ok = if let Some(text) = want.get("error").and_then(Value::as_str) {
            got["error"].as_str().is_some_and(|e| e.contains(text))
        } else {
            &got == want
        };
        if !ok {
            failures.push(format!("{}: got {got}, wanted {want}", case["name"]));
        }
    }
    println!("union vectors: {} cases", cases.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- the description ----

#[test]
fn a_union_describes_its_text_and_its_structured_arm() {
    let description = CacheSetting::description();
    let Kind::Union(union) = description.kind() else {
        panic!("{description:?}")
    };
    assert_eq!(union.text().ty(), "CacheSetting");
    assert_eq!(union.text().choices(), None);
    assert!(union.text().check("true").is_ok());
    assert!(union.text().check("maybe").is_err());
    let fields = union.structured().group_fields().unwrap();
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, ["ttl", "size", "token"]);

    let Kind::Union(switch) = Switch::description().kind().clone() else {
        panic!("a union")
    };
    assert_eq!(switch.text().choices(), Some(&["on", "off"][..]));
    assert!(switch.text().check("on").is_ok());
    assert!(switch.text().check("custom").is_err());
}

#[test]
fn an_enum_of_texts_alone_stays_a_leaf() {
    #[derive(Describe, Debug, PartialEq)]
    #[map(untagged)]
    enum Port {
        Auto,
        Number(u16),
    }
    #[derive(Describe, Debug, PartialEq)]
    #[map(untagged)]
    enum Wide {
        Inner(Port),
        Flag(bool),
    }
    assert!(matches!(Port::description().kind(), Kind::Leaf(_)));
    assert!(matches!(Wide::description().kind(), Kind::Leaf(_)));
    const { assert!(<Port as Describe>::LEAF) };
    const { assert!(<Wide as Describe>::LEAF) };
    const { assert!(!<CacheSetting as Describe>::LEAF) };
    const { assert!(!<Switch as Describe>::LEAF) };
    let mut p = Partial::<Wide>::new();
    p.set_at(&[], "auto").unwrap_err();
    p.set_at(&[], "Auto").unwrap();
    assert_eq!(p.build().unwrap(), Wide::Inner(Port::Auto));
    let mut p = Partial::<Wide>::new();
    p.set_at(&[], "8080").unwrap();
    assert_eq!(p.build().unwrap(), Wide::Inner(Port::Number(8080)));
    let mut p = Partial::<Wide>::new();
    p.set_at(&[], "true").unwrap();
    assert_eq!(p.build().unwrap(), Wide::Flag(true));
}

// ---- the slot ----

#[test]
fn setting_below_the_enum_switches_to_the_structured_arm_and_back() {
    let mut p = Partial::<Cached>::new();
    p.set("cache", "false").unwrap();
    assert_eq!(
        p.at(&key("cache")),
        Some(Datum::new("CacheSetting", "false"))
    );
    assert_eq!(p.entries(&key("cache")), None);

    p.set("cache.ttl", "5s").unwrap();
    assert_eq!(p.at(&key("cache")), None, "the text is dropped");
    assert!(matches!(p.entries(&key("cache")), Some(Entries::Keys(_))));
    p.set("cache.size", "3").unwrap();
    p.set("cache", "true").unwrap();
    assert_eq!(
        p.at(&key("cache")),
        Some(Datum::new("CacheSetting", "true"))
    );
    assert_eq!(
        p.at(&key("cache.size")),
        None,
        "the structured keys are dropped"
    );
    p.set("cache.size", "4").unwrap();
    assert_eq!(p.build().unwrap().cache, tuned(60, 4));
}

#[test]
fn a_refused_set_changes_nothing() {
    let mut p = Partial::<Cached>::new();
    p.set("cache", "false").unwrap();
    assert!(p.set("cache.bogus", "1").is_err());
    assert!(p.set("cache.ttl", "soon").is_err());
    assert_eq!(
        p.at(&key("cache")),
        Some(Datum::new("CacheSetting", "false"))
    );
    p.set("cache", "true").unwrap();
    assert!(p.set("cache", "maybe").is_err());
    assert_eq!(p.build().unwrap().cache, CacheSetting::Enabled(true));
}

#[test]
fn clearing_the_enum_forgets_both_sides() {
    let mut p = Partial::<Cached>::new();
    p.set("cache", "false").unwrap();
    p.clear("cache").unwrap();
    assert_eq!(p.build().unwrap().cache, CacheSetting::Enabled(true));
    let mut p = Partial::<Cached>::new();
    p.set("cache.size", "9").unwrap();
    p.clear("cache").unwrap();
    assert_eq!(p.build().unwrap().cache, CacheSetting::Enabled(true));
    let mut p = Partial::<Cached>::new();
    p.set("cache.size", "9").unwrap();
    p.clear("cache.size").unwrap();
    assert_eq!(p.build().unwrap().cache, tuned(60, 1024));
}

#[test]
fn an_empty_list_of_items_chooses_the_structured_arm() {
    let mut p = Partial::<Cached>::new();
    p.set("cache", "false").unwrap();
    p.set_items("cache", Vec::<String>::new()).unwrap();
    assert_eq!(p.build().unwrap().cache, tuned(60, 1024));
    let mut p = Partial::<Cached>::new();
    assert!(p.set_items("cache", ["a"]).is_err());
}

#[test]
fn a_required_union_nothing_set_is_missing_and_a_table_missing_a_key_says_which() {
    let missing = Partial::<Vetted>::new().build().unwrap_err();
    assert_eq!(missing.keys().len(), 1);
    assert_eq!(missing.keys()[0].to_string(), "vetting");
    let mut p = Partial::<Vetted>::new();
    p.set("vetting.level", "1").unwrap();
    let missing = p.build().unwrap_err();
    assert_eq!(missing.keys()[0].to_string(), "vetting.name");
}

#[test]
fn a_structured_default_keeps_its_keys_until_the_text_is_set() {
    let built = Partial::<Preset>::new().build().unwrap();
    assert_eq!(
        built.vetting,
        Switch::Custom(Definition {
            name: "d".into(),
            level: 5
        })
    );
    let mut p = Partial::<Preset>::new();
    p.set("vetting.level", "7").unwrap();
    assert_eq!(p.at(&key("vetting.level")), Some(Datum::new("u8", "7")));
    assert_eq!(
        p.build().unwrap().vetting,
        Switch::Custom(Definition {
            name: "d".into(),
            level: 7
        })
    );
    let mut p = Partial::<Preset>::new();
    assert_eq!(p.at(&key("vetting.name")), Some(Datum::new("String", "d")));
    p.set("vetting", "off").unwrap();
    assert_eq!(p.at(&key("vetting.name")), None, "off has no name");
    assert_eq!(p.build().unwrap().vetting, Switch::Off);
}

#[test]
fn a_union_inside_a_list_and_an_option() {
    let mut p = Partial::<Many>::new();
    p.set("switches[0]", "on").unwrap();
    p.set("switches[1].name", "n").unwrap();
    p.set("maybe.size", "2").unwrap();
    let built = p.build().unwrap();
    assert_eq!(
        built.switches,
        [
            Switch::On,
            Switch::Custom(Definition {
                name: "n".into(),
                level: 3
            })
        ]
    );
    assert_eq!(built.maybe, Some(tuned(60, 2)));
    let mut p = Partial::<Many>::new();
    p.set_items("switches", ["off"]).unwrap();
    p.set("maybe", "false").unwrap();
    let built = p.build().unwrap();
    assert_eq!(built.switches, [Switch::Off]);
    assert_eq!(built.maybe, Some(CacheSetting::Enabled(false)));
    let mut p = Partial::<Many>::new();
    p.set_items("switches", Vec::<String>::new()).unwrap();
    assert_eq!(p.build().unwrap().maybe, None);
}

// ---- reading a value ----

#[test]
fn a_built_value_answers_value_for_text_and_entries_for_structure() {
    let off = Cached {
        cache: CacheSetting::Enabled(false),
    };
    assert_eq!(
        off.at(&key("cache")),
        Some(Datum::new("CacheSetting", "false"))
    );
    assert_eq!(off.entries(&key("cache")), None);
    assert_eq!(off.at(&key("cache.ttl")), None);
    assert_eq!(off.to_string_for_datum(), "CacheSetting(false)");

    let on = Cached {
        cache: tuned(30, 8),
    };
    assert_eq!(on.at(&key("cache")), None);
    assert_eq!(
        on.at(&key("cache.ttl")),
        Some(Datum::new("Duration", "30s"))
    );
    assert!(matches!(on.entries(&key("cache")), Some(Entries::Keys(_))));
    assert_eq!(Switch::On.at(&[]), Some(Datum::new("Switch", "on")));
    assert_eq!(Switch::On.entries(&[]), None);
}

trait ShowCache {
    fn to_string_for_datum(&self) -> String;
}

impl ShowCache for Cached {
    fn to_string_for_datum(&self) -> String {
        self.at(&key("cache")).unwrap().to_string()
    }
}

// ---- overlay ----

#[test]
fn overlay_lays_text_over_structure_and_back() {
    let mut base = Partial::from(Cached {
        cache: tuned(30, 8),
    });
    base.overlay(Partial::from(Cached {
        cache: CacheSetting::Enabled(false),
    }));
    assert_eq!(base.build().unwrap().cache, CacheSetting::Enabled(false));

    let mut base = Partial::from(Cached {
        cache: CacheSetting::Enabled(false),
    });
    base.overlay(Partial::from(Cached {
        cache: tuned(30, 8),
    }));
    assert_eq!(base.build().unwrap().cache, tuned(30, 8));

    let mut base = Partial::from(Cached {
        cache: tuned(30, 8),
    });
    let mut later = Partial::<Cached>::new();
    later.set("cache.size", "99").unwrap();
    base.overlay(later);
    assert_eq!(base.build().unwrap().cache, tuned(30, 99));

    let mut base = Partial::from(Cached {
        cache: tuned(30, 8),
    });
    base.overlay(Partial::new());
    assert_eq!(
        base.build().unwrap().cache,
        tuned(30, 8),
        "nothing set changes nothing"
    );
}

// ---- non-defaults and differences ----

fn shown(finding: &coqui_builder::Finding) -> String {
    finding.value().to_string()
}

#[test]
fn non_defaults_for_each_pair_of_sides() {
    // The default is the text `true`.
    assert!(
        non_defaults(&Cached {
            cache: CacheSetting::Enabled(true)
        })
        .is_empty()
    );

    let text = non_defaults(&Cached {
        cache: CacheSetting::Enabled(false),
    });
    assert_eq!(text.len(), 1);
    assert_eq!(text[0].setting, "cache");
    assert_eq!(text[0].value.to_string(), "CacheSetting(false)");
    assert_eq!(text[0].default.to_string(), "CacheSetting(true)");

    let structured = non_defaults(&Cached {
        cache: tuned(30, 8),
    });
    assert_eq!(structured.len(), 1);
    assert_eq!(structured[0].setting, "cache");
    assert_eq!(
        structured[0].value.to_string(),
        "{ttl: Duration(30s), size: u32(8)}"
    );
    assert_eq!(structured[0].default.to_string(), "CacheSetting(true)");
    assert!(matches!(structured[0].value, Shown::Fields(_)));

    // A structured default: only the keys that differ are listed while the sides agree.
    assert!(
        non_defaults(&Preset {
            vetting: Switch::Custom(Definition {
                name: "d".into(),
                level: 5
            })
        })
        .is_empty()
    );
    let keys: Vec<String> = non_defaults(&Preset {
        vetting: Switch::Custom(Definition {
            name: "d".into(),
            level: 6,
        }),
    })
    .into_iter()
    .map(|n| n.setting)
    .collect();
    assert_eq!(keys, ["vetting.level"]);
    let off = non_defaults(&Preset {
        vetting: Switch::Off,
    });
    assert_eq!(off.len(), 1);
    assert_eq!(off[0].value.to_string(), "Switch(off)");
    assert_eq!(
        off[0].default.to_string(),
        "{name: String(d), level: u8(5)}"
    );
}

#[test]
fn differences_for_each_pair_of_sides() {
    let description = Cached::description();
    let a = Cached {
        cache: CacheSetting::Enabled(true),
    };
    let b = Cached {
        cache: CacheSetting::Enabled(false),
    };
    let c = Cached {
        cache: tuned(30, 8),
    };
    let d = Cached {
        cache: tuned(30, 9),
    };

    let report = differences(&description, &a, Some(&a));
    assert!(report.is_empty());
    let report = differences(&description, &a, Some(&b));
    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].key().to_string(), "cache");
    assert_eq!(shown(&report.findings()[0]), "CacheSetting(true)");
    assert_eq!(
        report.findings()[0].expected(),
        &Expect::Equal("false".into())
    );

    let report = differences(&description, &c, Some(&c));
    assert!(report.is_empty());
    let report = differences(&description, &c, Some(&d));
    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].key().to_string(), "cache.size");

    let report = differences(&description, &c, Some(&a));
    assert_eq!(report.len(), 1);
    assert_eq!(report.findings()[0].key().to_string(), "cache");
    assert_eq!(
        shown(&report.findings()[0]),
        "{ttl: Duration(30s), size: u32(8)}"
    );
    assert_eq!(
        report.findings()[0].expected(),
        &Expect::Equal("true".into())
    );
    assert_eq!(report.findings()[0].reason(), Some(&Reason::Differs));

    let report = differences(&description, &a, Some(&c));
    assert_eq!(report.len(), 1);
    assert_eq!(shown(&report.findings()[0]), "CacheSetting(true)");
    assert_eq!(report.findings()[0].expected(), &Expect::Required);
}

// ---- profiles ----

#[test]
fn a_profile_equal_on_the_text_side() {
    let profile = Profile::defaults::<Cached>();
    assert_eq!(
        profile.get(&"cache".parse().unwrap()),
        Some(&Expect::Equal("true".into()))
    );
    assert!(
        profile
            .verify(&Cached {
                cache: CacheSetting::Enabled(true)
            })
            .is_met()
    );

    let report = profile.verify(&Cached {
        cache: CacheSetting::Enabled(false),
    });
    assert_eq!(report.findings()[0].reason(), Some(&Reason::Differs));
    let report = profile.verify(&Cached {
        cache: tuned(30, 8),
    });
    assert_eq!(report.findings()[0].reason(), Some(&Reason::Differs));
    // The structured side is shown by its keys, as `differences` shows it, never as unset.
    assert!(matches!(report.findings()[0].value(), Shown::Fields(_)));
    assert_eq!(
        report.findings()[0].value().to_string(),
        "{ttl: Duration(30s), size: u32(8)}"
    );
    let requiring = Profile::new().expecting("cache", Expect::Required).unwrap();
    let report = requiring.verify(&Cached {
        cache: tuned(30, 8),
    });
    assert!(report.is_met());
    assert!(matches!(report.findings()[0].value(), Shown::Fields(_)));

    let wrote = Profile::new()
        .expecting("cache", Expect::Equal("false".into()))
        .unwrap();
    assert!(wrote.bind::<Cached>().is_ok());
    assert!(
        wrote
            .verify(&Cached {
                cache: CacheSetting::Enabled(false)
            })
            .is_met()
    );
    let refused = Profile::new()
        .expecting("cache", Expect::Equal("maybe".into()))
        .unwrap();
    assert!(refused.bind::<Cached>().is_err());
    assert!(
        !refused
            .verify(&Cached {
                cache: CacheSetting::Enabled(false)
            })
            .is_met()
    );
}

#[test]
fn a_profile_of_a_structured_value_expects_its_keys() {
    let base = Cached {
        cache: tuned(30, 8),
    };
    let profile = Profile::against(&base);
    assert_eq!(
        profile.get(&"cache.ttl".parse().unwrap()),
        Some(&Expect::Equal("30s".into()))
    );
    assert_eq!(
        profile.get(&"cache.size".parse().unwrap()),
        Some(&Expect::Equal("8".into()))
    );
    assert_eq!(
        profile.get(&"cache.token".parse().unwrap()),
        Some(&Expect::Absent)
    );
    assert!(profile.verify(&base).is_met());
    assert!(
        !profile
            .verify(&Cached {
                cache: tuned(30000, 8)
            })
            .is_met()
    );
    let report = profile.verify(&Cached {
        cache: CacheSetting::Enabled(false),
    });
    assert_eq!(
        report.unmet().count(),
        1,
        "one finding at the enum, not one per key"
    );
    let found = report.unmet().next().unwrap();
    assert_eq!(found.key().to_string(), "cache");
    assert_eq!(found.reason(), Some(&Reason::Differs));

    let text = Profile::against(&Cached {
        cache: CacheSetting::Enabled(false),
    });
    assert_eq!(
        text.get(&"cache".parse().unwrap()),
        Some(&Expect::Equal("false".into()))
    );
    assert_eq!(text.len(), 1);
}

#[test]
fn an_expectation_on_the_enum_speaks_for_the_keys_below_it() {
    let profile = Profile::new()
        .expecting("cache", Expect::Equal("false".into()))
        .unwrap()
        .expecting("cache.size", Expect::Equal("8".into()))
        .unwrap();
    let report = profile.verify(&Cached {
        cache: CacheSetting::Enabled(false),
    });
    assert_eq!(report.len(), 1, "the key below is not asked of a text");
    assert!(report.is_met());

    // Only a text expectation speaks for the keys below; a bare presence does not.
    let profile = Profile::new()
        .expecting("cache", Expect::Required)
        .unwrap()
        .expecting("cache.size", Expect::Equal("8".into()))
        .unwrap();
    let report = profile.verify(&Cached {
        cache: CacheSetting::Enabled(false),
    });
    assert!(!report.is_met());
    assert!(report.unmet().any(|f| f.key().to_string() == "cache"));
    assert!(
        profile
            .verify(&Cached {
                cache: tuned(30, 8)
            })
            .is_met()
    );
}

// ---- redaction ----

#[test]
fn a_secret_inside_the_structured_arm_is_never_shown() {
    let secret = "hunter2";
    let value = Cached {
        cache: CacheSetting::Tuned(CacheOptions {
            ttl: Duration::from_secs(60),
            size: 1024,
            token: Some(secret.into()),
        }),
    };
    let listed = non_defaults(&value);
    assert_eq!(listed.len(), 1);
    let line = listed[0].to_string();
    assert!(!line.contains(secret), "{line}");
    assert!(line.contains("token: String(***)"), "{line}");

    let report = differences(
        &Cached::description(),
        &value,
        Some(&Cached {
            cache: CacheSetting::Enabled(true),
        }),
    );
    assert!(!format!("{report:?}").contains(secret));

    let profile = Profile::new()
        .expecting("cache.token", Expect::Equal("other".into()))
        .unwrap();
    let report = profile.verify(&value);
    let text = format!("{report:?}");
    assert!(!text.contains(secret), "{text}");
    assert!(!text.contains("other"), "{text}");

    let against = Profile::against(&value);
    assert_eq!(
        against.get(&"cache.token".parse().unwrap()),
        Some(&Expect::Required)
    );
}

// ---- sources ----

#[test]
fn a_source_answering_a_value_and_a_table_is_refused_naming_the_path() {
    let mut p = Partial::<Cached>::new();
    let error = p
        .fill_from(&mut Json(json!({"cache": "x"})))
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(error.kind(), FillErrorKind::Leaf(_)));

    struct Both;
    impl Source for Both {
        fn value(
            &mut self,
            at: &KeyPath,
            _: &coqui_builder::describe::LeafInfo,
        ) -> Result<Option<String>, SourceError> {
            Ok((at.to_string() == "cache").then(|| "true".to_string()))
        }
        fn entries(&mut self, at: &KeyPath) -> Result<Option<Entries>, SourceError> {
            Ok((at.to_string() == "cache").then(|| Entries::Keys(vec!["ttl".into()])))
        }
        fn unknown(&mut self, _: &dyn Fn(&KeyPath) -> bool) -> Vec<KeyPath> {
            Vec::new()
        }
    }
    let error = Partial::<Cached>::new().fill_from(&mut Both).unwrap_err();
    assert!(matches!(error.kind(), FillErrorKind::Source(_)));
    assert_eq!(
        error.to_string(),
        "the source failed: cache is given both as a value and as keys below it"
    );
}

#[test]
fn a_real_failure_of_a_source_is_still_a_failure() {
    struct Broken;
    impl Source for Broken {
        fn value(
            &mut self,
            _: &KeyPath,
            _: &coqui_builder::describe::LeafInfo,
        ) -> Result<Option<String>, SourceError> {
            Err(SourceError::new("the file cannot be read"))
        }
        fn entries(&mut self, _: &KeyPath) -> Result<Option<Entries>, SourceError> {
            Ok(None)
        }
        fn unknown(&mut self, _: &dyn Fn(&KeyPath) -> bool) -> Vec<KeyPath> {
            Vec::new()
        }
    }
    let error = Partial::<Cached>::new().fill_from(&mut Broken).unwrap_err();
    assert_eq!(
        error.to_string(),
        "the source failed: the file cannot be read"
    );
}

#[test]
fn the_environment_variable_of_a_table_key_is_not_reported_unknown() {
    let mut env = Env::from_vars("APP", [("APP_CACHE__SIZE", "5")]);
    let mut p = Partial::<Cached>::new();
    let unknown = p.fill_from(&mut env).unwrap();
    assert!(unknown.is_empty());
    assert_eq!(p.build().unwrap().cache, tuned(60, 5));
    let mut env = Env::from_vars("APP", [("APP_CACHE__BOGUS", "5")]);
    let error = Partial::<Cached>::new().fill_from(&mut env).unwrap_err();
    assert!(matches!(error.kind(), FillErrorKind::UnknownKeys(_)));
}

#[test]
fn a_list_and_an_option_of_unions_are_filled_from_a_source() {
    let mut p = Partial::<Many>::new();
    p.fill_from(&mut Json(json!({
        "switches": ["on", {"name": "n", "level": 1}, "off"],
        "maybe": {"size": 2},
    })))
    .unwrap();
    let built = p.build().unwrap();
    assert_eq!(
        built.switches,
        [
            Switch::On,
            Switch::Custom(Definition {
                name: "n".into(),
                level: 1
            }),
            Switch::Off,
        ]
    );
    assert_eq!(built.maybe, Some(tuned(60, 2)));

    let mut base = Partial::from(built);
    let mut later = Partial::<Many>::new();
    later
        .fill_from(&mut Json(
            json!({"switches": [{"name": "z"}], "maybe": true}),
        ))
        .unwrap();
    base.overlay(later);
    let built = base.build().unwrap();
    assert_eq!(
        built.switches,
        [Switch::Custom(Definition {
            name: "z".into(),
            level: 3
        })],
        "a list the later layer sets replaces the list"
    );
    assert_eq!(built.maybe, Some(CacheSetting::Enabled(true)));
}

#[test]
fn a_structured_default_survives_a_text_and_a_clear() {
    let fresh = Partial::<Preset>::new().build().unwrap();
    let mut p = Partial::<Preset>::new();
    p.set("vetting.level", "7").unwrap();
    p.set("vetting", "off").unwrap();
    p.clear("vetting").unwrap();
    assert_eq!(p.at(&key("vetting.name")), Some(Datum::new("String", "d")));
    assert_eq!(p.build().unwrap(), fresh);

    let mut p = Partial::<Preset>::new();
    p.set("vetting.level", "7").unwrap();
    p.set("vetting", "off").unwrap();
    p.set("vetting.level", "8").unwrap();
    assert_eq!(
        p.build().unwrap().vetting,
        Switch::Custom(Definition {
            name: "d".into(),
            level: 8
        })
    );
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Peers {
    Off,
    Listed(Vec<String>),
}

#[derive(Describe, Debug, Clone, PartialEq)]
struct Meshed {
    #[map(default = Peers::Listed(vec!["a".to_string()]))]
    peers: Peers,
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Labels {
    None,
    Named(std::collections::BTreeMap<String, String>),
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(tag = "kind")]
enum Auth {
    Password { user: String },
    Token { id: String },
}

#[derive(Describe, Debug, Clone, PartialEq)]
#[map(untagged)]
enum Login {
    Anonymous,
    With(Auth),
}

#[test]
fn a_list_arm_with_a_default_is_not_a_difference_from_itself() {
    let same = Meshed {
        peers: Peers::Listed(vec!["a".into()]),
    };
    assert!(non_defaults(&same).is_empty());
    assert!(Profile::defaults::<Meshed>().verify(&same).is_met());
    assert!(differences(&Meshed::description(), &Partial::<Meshed>::new(), None).is_empty());
    assert_eq!(
        Partial::<Meshed>::new().at(&key("peers[0]")),
        Some(Datum::new("String", "a"))
    );
    let other = Meshed {
        peers: Peers::Listed(vec!["b".into()]),
    };
    assert_eq!(non_defaults(&other).len(), 1);
    assert!(!Profile::defaults::<Meshed>().verify(&other).is_met());
    let off = non_defaults(&Meshed { peers: Peers::Off });
    assert_eq!(off.len(), 1);
    assert_eq!(off[0].value.to_string(), "Peers(Off)");
    assert_eq!(off[0].default.to_string(), "{[0]: String(a)}");

    let mut p = Partial::<Meshed>::new();
    p.set_items("peers", ["x", "y"]).unwrap();
    assert_eq!(
        p.build().unwrap().peers,
        Peers::Listed(vec!["x".into(), "y".into()])
    );
    let mut p = Partial::<Meshed>::new();
    p.set("peers", "Off").unwrap();
    assert_eq!(p.build().unwrap().peers, Peers::Off);
    let mut p = Partial::<Meshed>::new();
    p.fill_from(&mut Json(json!({"peers": ["q"]}))).unwrap();
    assert_eq!(p.build().unwrap().peers, Peers::Listed(vec!["q".into()]));
}

#[test]
fn a_map_arm_and_a_tagged_arm() {
    #[derive(Describe, Debug, Clone, PartialEq)]
    struct Tagged {
        labels: Labels,
        login: Login,
    }
    let mut p = Partial::<Tagged>::new();
    p.fill_from(&mut Json(json!({
        "labels": {"a": "1", "b": "2"},
        "login": {"kind": "Token", "id": "t"},
    })))
    .unwrap();
    let built = p.build().unwrap();
    assert_eq!(
        built.labels,
        Labels::Named([("a".into(), "1".into()), ("b".into(), "2".into())].into())
    );
    assert_eq!(built.login, Login::With(Auth::Token { id: "t".into() }));

    let mut p = Partial::<Tagged>::new();
    p.fill_from(&mut Json(json!({"labels": "None", "login": "Anonymous"})))
        .unwrap();
    let text = p.build().unwrap();
    assert_eq!(
        (text.labels.clone(), text.login.clone()),
        (Labels::None, Login::Anonymous)
    );

    let mut base = Partial::from(built.clone());
    base.overlay(Partial::from(text.clone()));
    assert_eq!(base.build().unwrap(), text);
    let mut base = Partial::from(text.clone());
    base.overlay(Partial::from(built.clone()));
    assert_eq!(base.build().unwrap(), built);

    let listed = differences(&Tagged::description(), &text, Some(&built));
    let keys: Vec<String> = listed
        .findings()
        .iter()
        .map(|f| f.key().to_string())
        .collect();
    assert_eq!(keys, ["labels", "login"]);
    let same = differences(&Tagged::description(), &built, Some(&built));
    assert!(same.is_empty());
    let mut other = built.clone();
    other.login = Login::With(Auth::Token { id: "u".into() });
    let report = differences(&Tagged::description(), &other, Some(&built));
    assert_eq!(report.findings()[0].key().to_string(), "login.id");
}
