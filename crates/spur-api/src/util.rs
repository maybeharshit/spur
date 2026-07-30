// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Conversions from protobuf well-known types to the plain scalars the JSON
//! and YAML schemas expose.

use prost_types::{Duration, Timestamp};

/// Epoch seconds, or `None` when the timestamp is absent.
pub(crate) fn ts_secs(ts: Option<&Timestamp>) -> Option<i64> {
    ts.map(|t| t.seconds)
}

pub(crate) fn seconds(d: Option<&Duration>) -> Option<i64> {
    d.map(|d| d.seconds)
}

/// Whole minutes, the granularity Slurm reports time limits in.
pub(crate) fn minutes(d: Option<&Duration>) -> Option<i64> {
    d.map(|d| d.seconds / 60)
}

pub(crate) fn duration_secs(d: Option<&Duration>) -> i64 {
    d.map(|d| d.seconds).unwrap_or(0)
}

/// Split a comma-joined proto field back into a list.
///
/// Several proto messages flatten string lists for display convenience, but the
/// published schema exposes them as arrays. Account, group, QOS, and user names
/// cannot contain commas, so the round trip is lossless.
pub(crate) fn csv_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_truncates_toward_zero() {
        let d = Duration {
            seconds: 119,
            nanos: 0,
        };
        assert_eq!(minutes(Some(&d)), Some(1));
        assert_eq!(minutes(None), None);
    }

    #[test]
    fn duration_secs_defaults_to_zero_when_absent() {
        assert_eq!(duration_secs(None), 0);
    }

    #[test]
    fn csv_list_drops_blanks_and_trims() {
        assert_eq!(csv_list("a, b ,,c"), vec!["a", "b", "c"]);
        assert!(csv_list("").is_empty());
        assert!(csv_list("  ,  ").is_empty());
    }

    #[test]
    fn ts_secs_preserves_absence() {
        assert_eq!(ts_secs(None), None);
        let t = Timestamp {
            seconds: 5,
            nanos: 0,
        };
        assert_eq!(ts_secs(Some(&t)), Some(5));
    }
}
