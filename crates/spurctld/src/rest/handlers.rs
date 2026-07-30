// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::Json;
use spur_api::{JobsPayload, NodesPayload, PartitionsPayload};

use super::convert::parse_states_query;
use super::types::*;
use super::RestState;
use crate::server::{job_to_proto, node_to_proto, partition_to_proto};

pub async fn ping(
    State(state): State<Arc<RestState>>,
) -> Result<Json<ApiResponse<PingData>>, RestError> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".into());

    Ok(ok(PingData {
        ping: vec![PingInfo {
            hostname,
            pinged: "UP".into(),
            latency: 0,
            mode: if state.raft.is_leader() {
                "primary"
            } else {
                "replica"
            }
            .into(),
        }],
    }))
}

pub async fn get_jobs(
    State(state): State<Arc<RestState>>,
    Query(query): Query<JobsQuery>,
) -> Result<Json<ApiResponse<JobsPayload>>, RestError> {
    let states = match query.state.as_deref() {
        Some(s) => parse_states_query(s).map_err(|e| bad_request_response(&e))?,
        None => Vec::new(),
    };

    let user = query.user.as_deref();
    let partition = query.partition.as_deref();
    let account = query.account.as_deref();
    let name = query.name.as_deref();

    let jobs = state
        .cluster
        .get_jobs(&states, user, partition, account, name, &[]);
    let protos: Vec<_> = jobs.iter().map(job_to_proto).collect();

    Ok(ok(JobsPayload::from_proto(&protos)))
}

pub async fn get_job(
    State(state): State<Arc<RestState>>,
    Path(job_id): Path<u32>,
) -> Result<Json<ApiResponse<JobsPayload>>, RestError> {
    let job = state
        .cluster
        .get_job_for_display(job_id)
        .ok_or_else(|| not_found_response(&format!("job {job_id} not found")))?;

    Ok(ok(JobsPayload::from_proto(&[job_to_proto(&job)])))
}

pub async fn submit_job(
    State(state): State<Arc<RestState>>,
    Json(body): Json<SubmitRequest>,
) -> Result<Json<ApiResponse<SubmitResponse>>, RestError> {
    if !state.raft.is_leader() {
        return Err(unavailable_response("not the Raft leader"));
    }

    let time_limit = body
        .job
        .time_limit
        .as_ref()
        .and_then(|t| spur_core::config::parse_time_minutes(t))
        .map(|mins| chrono::Duration::minutes(mins as i64));

    let spec = spur_core::job::JobSpec {
        name: body.job.name.unwrap_or_default(),
        user: body.job.user.unwrap_or_default(),
        partition: body.job.partition,
        account: body.job.account,
        num_nodes: body.job.nodes.unwrap_or(1),
        num_tasks: body.job.ntasks.unwrap_or(1),
        cpus_per_task: body.job.cpus_per_task.unwrap_or(1),
        time_limit,
        script: body.job.script,
        environment: body.job.environment,
        gres: body.job.gres,
        gpus: parse_rest_gpu(body.job.gpus.as_deref())?,
        gpus_per_node: parse_rest_gpu(body.job.gpus_per_node.as_deref())?,
        gpus_per_task: parse_rest_gpu(body.job.gpus_per_task.as_deref())?,
        ..Default::default()
    };

    // Validate the GPU request (mutually exclusive forms, --gpus >= num_nodes).
    spur_core::gpu_request::resolve_gpu_demand(&spec)
        .map_err(|e| bad_request_response(&e.to_string()))?;

    let job_id = state.cluster.submit_job(spec).map_err(submit_rest_error)?;

    Ok(ok(SubmitResponse { job_id }))
}

/// Parse a REST GPU field ("4" or "mi300x:4") into a core GPU request.
#[allow(clippy::result_large_err)]
fn parse_rest_gpu(
    value: Option<&str>,
) -> Result<Option<spur_core::gpu_request::GpuRequest>, RestError> {
    match value {
        Some(v) if !v.is_empty() => spur_core::gpu_request::GpuRequest::parse_flag(v)
            .map_err(|e| bad_request_response(&e.to_string())),
        _ => Ok(None),
    }
}

fn submit_rest_error(err: crate::cluster::SubmitError) -> RestError {
    match err {
        crate::cluster::SubmitError::InvalidArgument(m) => bad_request_response(&m),
        crate::cluster::SubmitError::Internal(m) => error_response(&format!("submit failed: {m}")),
    }
}

pub async fn cancel_job(
    State(state): State<Arc<RestState>>,
    Path(job_id): Path<u32>,
) -> Result<Json<ApiResponse<serde_json::Value>>, RestError> {
    if !state.raft.is_leader() {
        return Err(unavailable_response("not the Raft leader"));
    }

    let job = state.cluster.get_job(job_id);

    state
        .cluster
        .cancel_job(job_id, "")
        .map_err(|e| error_response(&format!("cancel failed: {e}")))?;

    if let Some(job) = job {
        let cluster = state.cluster.clone();
        tokio::spawn(async move {
            crate::scheduler_loop::send_cancel_to_agents(&cluster, &job, 0).await;
        });
    }

    Ok(ok(serde_json::json!({})))
}

pub async fn get_nodes(
    State(state): State<Arc<RestState>>,
) -> Result<Json<ApiResponse<NodesPayload>>, RestError> {
    let nodes = state.cluster.get_nodes();
    let protos: Vec<_> = nodes.iter().map(node_to_proto).collect();

    Ok(ok(NodesPayload::from_proto(&protos)))
}

pub async fn get_node(
    State(state): State<Arc<RestState>>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<NodesPayload>>, RestError> {
    let node = state
        .cluster
        .get_node(&name)
        .ok_or_else(|| not_found_response(&format!("node {name} not found")))?;

    Ok(ok(NodesPayload::from_proto(&[node_to_proto(&node)])))
}

pub async fn get_partitions(
    State(state): State<Arc<RestState>>,
) -> Result<Json<ApiResponse<PartitionsPayload>>, RestError> {
    let partitions = state.cluster.get_partitions();
    let protos: Vec<_> = partitions.iter().map(partition_to_proto).collect();

    Ok(ok(PartitionsPayload::from_proto(&protos)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rest_gpu_valid() {
        let req = parse_rest_gpu(Some("mi300x:4"));
        assert!(req.is_ok());
        let req = req.ok().flatten().unwrap();
        assert_eq!(req.count, 4);
        assert_eq!(req.gpu_type, Some("mi300x".into()));
    }

    #[test]
    fn parse_rest_gpu_zero_is_none() {
        let res = parse_rest_gpu(Some("0"));
        assert!(res.is_ok());
        assert!(res.ok().flatten().is_none());
    }

    #[test]
    fn parse_rest_gpu_invalid_returns_error() {
        let err = parse_rest_gpu(Some("::bad"));
        assert!(err.is_err());
    }

    #[test]
    fn conflict_error_text_is_neutral() {
        let msg = spur_core::gpu_request::GpuRequestError::Conflict.to_string();
        assert!(
            !msg.contains("--"),
            "error message should not contain CLI flags"
        );
        assert!(msg.contains("gres"));
    }
}
