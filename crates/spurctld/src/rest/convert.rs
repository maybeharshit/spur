// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Request-side parsing for REST query parameters.
//!
//! Response-side conversion lives in [`spur_api`]: handlers map their domain
//! types to proto with the same helpers the gRPC service uses, then hand the
//! proto messages to the shared views.

use spur_core::job::JobState;

pub fn parse_states_query(s: &str) -> Result<Vec<JobState>, String> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(Vec::new());
    }

    let tokens: Vec<&str> = trimmed
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        return Err("Invalid job state specified: (empty)".into());
    }

    let mut states = Vec::with_capacity(tokens.len());
    for token in tokens {
        let core = JobState::from_code_or_name(token)
            .ok_or_else(|| format!("Invalid job state specified: {token}"))?;
        states.push(core);
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_states_query_accepts_valid_tokens() {
        let states = parse_states_query("RUNNING,PD").unwrap();
        assert_eq!(states.len(), 2);
    }

    #[test]
    fn parse_states_query_rejects_unknown() {
        assert!(parse_states_query("BOGUS").is_err());
        assert!(parse_states_query("R,BOGUS").is_err());
    }

    #[test]
    fn parse_states_query_all_means_no_filter() {
        assert!(parse_states_query("all").unwrap().is_empty());
    }
}
