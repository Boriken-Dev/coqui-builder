// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Durations: every form read, every refusal, one form written.

use core::time::Duration;

use coqui_builder::Leaf;
use coqui_builder::describe::{LeafError, assert_round_trip};

fn read(text: &str) -> Result<Duration, LeafError> {
    Duration::from_text(text)
}

#[test]
fn a_duration_reads_a_number_with_an_optional_unit() {
    assert_eq!(read("30"), Ok(Duration::from_secs(30)));
    assert_eq!(read("30s"), Ok(Duration::from_secs(30)));
    assert_eq!(read("0.25"), Ok(Duration::from_millis(250)));
    assert_eq!(read("250ms"), Ok(Duration::from_millis(250)));
    assert_eq!(read("1.5h"), Ok(Duration::from_secs(5400)));
    assert_eq!(read("2m"), Ok(Duration::from_secs(120)));
    assert_eq!(read("7us"), Ok(Duration::from_micros(7)));
    assert_eq!(read("9ns"), Ok(Duration::from_nanos(9)));
    assert_eq!(read("1.000000001"), Ok(Duration::new(1, 1)));
}

#[test]
fn a_duration_is_read_exactly_so_equal_spellings_are_equal() {
    assert_eq!(read("30s"), read("30000ms"));
    assert_eq!(read("0.5"), read("500ms"));
    assert_eq!(read("0.250000000"), read("0.25"));
    assert_eq!(read("1.50"), read("1.5"));
    assert_eq!(read("0"), Ok(Duration::ZERO));
    assert_eq!(read("0ms"), Ok(Duration::ZERO));
}

#[test]
fn a_duration_refuses_what_it_cannot_hold() {
    assert_eq!(read("-1"), Err(LeafError::Negative));
    assert_eq!(read("0.0000000001"), Err(LeafError::TooFine));
    assert_eq!(read("0.5ns"), Err(LeafError::TooFine));
    for bad in [
        "", ".5", "5.", "1e3", "+1", "1 s", " 1", "1.2.3", "abc", "never", "1m30s", "5d", "[30, 0]",
    ] {
        assert!(read(bad).is_err(), "{bad:?}");
    }
    assert!(read("99999999999999999999999999999999999999999").is_err());
    assert!(read("18446744073709551616s").is_err());
}

#[test]
fn a_duration_is_written_exactly_in_the_largest_unit_that_holds_it() {
    assert_eq!(Duration::from_secs(30).to_text(), "30s");
    assert_eq!(Duration::from_millis(250).to_text(), "250ms");
    assert_eq!(Duration::from_micros(7).to_text(), "7us");
    assert_eq!(Duration::new(1, 1).to_text(), "1000000001ns");
    assert_eq!(Duration::ZERO.to_text(), "0s");
    assert_eq!(Duration::from_millis(1500).to_text(), "1500ms");
}

#[test]
fn a_duration_reads_back_what_it_writes_at_every_edge() {
    assert_round_trip(&[
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_nanos(999_999_999),
        Duration::new(1, 1),
        Duration::new(0, 1_000),
        Duration::new(59, 999_999_999),
        Duration::from_secs(u64::MAX),
        Duration::MAX,
        Duration::new(u64::MAX, 1),
    ]);
}
