// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The response envelope shared by the REST API and the CLI's structured output.

use serde::Serialize;

/// Slurm's `openapi` response wrapper: a `meta` block, error and warning
/// arrays, and the entity payload flattened alongside them so that a jobs
/// response reads `{"meta": {...}, "jobs": [...]}`.
#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub meta: Meta,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ApiError>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(flatten)]
    pub data: T,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(data: T) -> Self {
        Self {
            meta: Meta::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
            data,
        }
    }

    /// Record the argv that produced this document. Slurm populates
    /// `meta.command` for CLI output and leaves it absent over HTTP.
    pub fn with_command<I, S>(mut self, argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.meta.command = argv.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_errors(mut self, errors: Vec<ApiError>) -> Self {
        self.errors = errors;
        self
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Meta {
    #[serde(rename = "Slurm")]
    pub slurm: SlurmMeta,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SlurmMeta {
    pub version: VersionInfo,
    pub release: String,
}

impl Default for SlurmMeta {
    fn default() -> Self {
        Self {
            version: VersionInfo::default(),
            release: format!("spur {}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// The Slurm plugin API version this schema targets. It tracks the
/// `/slurm/v0.0.42` REST route prefix, not Spur's own release, which is
/// reported separately as `release`.
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub micro: u32,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 0,
            micro: 42,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_number: Option<i32>,
}

impl ApiError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            error: msg.into(),
            error_number: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::JobsPayload;

    #[test]
    fn envelope_flattens_payload_next_to_meta() {
        let doc = serde_json::to_value(Envelope::new(JobsPayload { jobs: Vec::new() })).unwrap();
        assert!(doc.get("meta").is_some());
        assert_eq!(doc["jobs"], serde_json::json!([]));
        // Payload must not be nested under a `data` key.
        assert!(doc.get("data").is_none());
    }

    #[test]
    fn empty_error_and_warning_arrays_are_omitted() {
        let doc = serde_json::to_value(Envelope::new(JobsPayload { jobs: Vec::new() })).unwrap();
        assert!(doc.get("errors").is_none());
        assert!(doc.get("warnings").is_none());
    }

    #[test]
    fn errors_appear_when_populated() {
        let doc = serde_json::to_value(
            Envelope::new(serde_json::json!({})).with_errors(vec![ApiError::new("boom")]),
        )
        .unwrap();
        assert_eq!(doc["errors"][0]["error"], "boom");
        assert!(doc["errors"][0].get("error_number").is_none());
    }

    #[test]
    fn command_is_absent_unless_recorded() {
        let bare = serde_json::to_value(Envelope::new(JobsPayload { jobs: Vec::new() })).unwrap();
        assert!(bare["meta"].get("command").is_none());

        let with_argv = serde_json::to_value(
            Envelope::new(JobsPayload { jobs: Vec::new() }).with_command(["squeue", "--json"]),
        )
        .unwrap();
        assert_eq!(
            with_argv["meta"]["command"],
            serde_json::json!(["squeue", "--json"])
        );
    }

    #[test]
    fn release_reports_the_spur_version_not_the_plugin_version() {
        let doc = serde_json::to_value(Envelope::new(serde_json::json!({}))).unwrap();
        assert_eq!(
            doc["meta"]["Slurm"]["release"],
            format!("spur {}", env!("CARGO_PKG_VERSION")).as_str()
        );
        assert_eq!(doc["meta"]["Slurm"]["version"]["micro"], 42);
    }
}
