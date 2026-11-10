// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A library keeps no process-wide state: no `static` item, no `OnceLock`, no `LazyLock` and
//! no `thread_local!` in the source of either crate.

use std::fs;
use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The code of a line: comments cut off, and the `'static` lifetime taken out.
fn code(line: &str) -> String {
    let line = line.split("//").next().unwrap_or_default();
    line.replace("'static", "")
}

fn offences(text: &str) -> Vec<String> {
    text.lines()
        .map(code)
        .filter(|line| {
            line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|word| matches!(word, "static" | "OnceLock" | "LazyLock"))
                || line.contains("thread_local!")
        })
        .collect()
}

#[test]
fn the_sources_of_both_crates_hold_no_process_wide_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    sources(&root.join("crates/coqui-builder-derive/src"), &mut files);
    assert!(files.len() > 10, "found {} files", files.len());
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        let found = offences(&text);
        assert!(found.is_empty(), "{}: {found:?}", file.display());
    }
}

#[test]
fn the_scan_sees_what_it_looks_for_and_not_the_static_lifetime() {
    assert!(!offences("static COUNT: u8 = 0;").is_empty());
    assert!(!offences("static mut X: u8 = 0;").is_empty());
    assert!(!offences("use std::sync::OnceLock;").is_empty());
    assert!(!offences("let x: LazyLock<u8>;").is_empty());
    assert!(!offences("thread_local! { }").is_empty());
    assert!(offences("fn f() -> &'static str { \"\" }").is_empty());
    assert!(offences("// a static item is refused").is_empty());
    assert!(offences("let statics = 1;").is_empty());
}
