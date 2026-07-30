// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! HTTP-specific glue around the shared [`spur_api`] schema.
//!
//! The envelope and entity views live in `spur-api` so the CLI's `--json` and
//! `--yaml` modes emit the same documents these handlers do. Only the Axum
//! wrapping and the request bodies are local.

use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use spur_api::{ApiError, Envelope};
use std::collections::HashMap;

pub type ApiResponse<T> = Envelope<T>;

pub fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(Envelope::new(data))
}

pub type RestError = (StatusCode, Json<ApiResponse<serde_json::Value>>);

pub fn error_response(msg: &str) -> RestError {
    api_error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
}

pub fn bad_request_response(msg: &str) -> RestError {
    api_error_response(StatusCode::BAD_REQUEST, msg)
}

pub fn not_found_response(msg: &str) -> RestError {
    api_error_response(StatusCode::NOT_FOUND, msg)
}

pub fn unavailable_response(msg: &str) -> RestError {
    api_error_response(StatusCode::SERVICE_UNAVAILABLE, msg)
}

pub fn api_error_response(status: StatusCode, msg: &str) -> RestError {
    (
        status,
        Json(Envelope::new(serde_json::json!({})).with_errors(vec![ApiError::new(msg)])),
    )
}

// -- Request/response data types --

#[derive(Serialize)]
pub struct PingData {
    pub ping: Vec<PingInfo>,
}

#[derive(Serialize)]
pub struct PingInfo {
    pub hostname: String,
    pub pinged: String,
    pub latency: u64,
    pub mode: String,
}

#[derive(Deserialize)]
pub struct JobsQuery {
    pub user: Option<String>,
    pub partition: Option<String>,
    pub state: Option<String>,
    pub account: Option<String>,
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct SubmitRequest {
    pub job: SubmitJobFields,
}

#[derive(Deserialize)]
pub struct SubmitJobFields {
    pub name: Option<String>,
    pub user: Option<String>,
    pub partition: Option<String>,
    pub account: Option<String>,
    pub nodes: Option<u32>,
    pub ntasks: Option<u32>,
    pub cpus_per_task: Option<u32>,
    pub time_limit: Option<String>,
    pub script: Option<String>,
    #[serde(default)]
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub gres: Vec<String>,
    /// GPU requests ("4" or "mi300x:4"); at most one may be set.
    pub gpus: Option<String>,
    pub gpus_per_node: Option<String>,
    pub gpus_per_task: Option<String>,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    pub job_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_job_fields_deserialize_user() {
        let body: SubmitRequest =
            serde_json::from_str(r#"{"job":{"user":"alice","account":"research"}}"#).unwrap();
        assert_eq!(body.job.user.as_deref(), Some("alice"));
        assert_eq!(body.job.account.as_deref(), Some("research"));
    }

    #[test]
    fn error_responses_carry_the_message_in_the_envelope() {
        let (status, Json(envelope)) = bad_request_response("bad state");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let doc = serde_json::to_value(&envelope).unwrap();
        assert_eq!(doc["errors"][0]["error"], "bad state");
        assert!(doc.get("meta").is_some());
    }
}
