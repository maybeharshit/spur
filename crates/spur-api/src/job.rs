// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Job-shaped views: queue/accounting records, priority factors, live step
//! statistics, and job steps.

use serde::Serialize;
use spur_proto::proto::{JobInfo, JobStepInfo};

use crate::util::{duration_secs, minutes, seconds, ts_secs};

/// A job as published by `GET /jobs`, `squeue`, `sacct`, and
/// `scontrol show job`.
#[derive(Debug, Serialize)]
pub struct JobView {
    pub job_id: u32,
    pub name: String,
    pub user_name: String,
    pub user_id: u32,
    pub partition: String,
    pub account: String,
    pub qos: String,
    pub job_state: String,
    pub state_reason: String,
    pub submit_time: Option<i64>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    /// Wall-clock limit in minutes. `null` means unlimited.
    pub time_limit: Option<i64>,
    pub run_time: i64,
    pub node_count: u32,
    pub tasks: u32,
    pub cpus_per_task: u32,
    pub nodes: String,
    pub current_working_directory: String,
    pub command: String,
    pub standard_output: String,
    pub standard_error: String,
    pub standard_input: String,
    pub exit_code: i32,
    pub exit_signal: i32,
    pub derived_exit_code: i32,
    pub priority: u32,
    pub reservation: String,
    pub comment: String,
    pub array_job_id: u32,
    pub array_task_id: u32,
    pub requested_gpus: u32,
    pub requested_gpus_detail: String,
}

impl From<&JobInfo> for JobView {
    fn from(job: &JobInfo) -> Self {
        Self {
            job_id: job.job_id,
            name: job.name.clone(),
            user_name: job.user.clone(),
            user_id: job.uid,
            partition: job.partition.clone(),
            account: job.account.clone(),
            qos: job.qos.clone(),
            job_state: job_state_name(job.state),
            state_reason: job.state_reason.clone(),
            submit_time: ts_secs(job.submit_time.as_ref()),
            start_time: ts_secs(job.start_time.as_ref()),
            end_time: ts_secs(job.end_time.as_ref()),
            time_limit: minutes(job.time_limit.as_ref()),
            run_time: duration_secs(job.run_time.as_ref()),
            node_count: job.num_nodes,
            tasks: job.num_tasks,
            cpus_per_task: job.cpus_per_task,
            nodes: job.nodelist.clone(),
            current_working_directory: job.work_dir.clone(),
            command: job.command.clone(),
            standard_output: job.stdout_path.clone(),
            standard_error: job.stderr_path.clone(),
            standard_input: job.stdin_path.clone(),
            exit_code: job.exit_code,
            exit_signal: job.exit_signal,
            derived_exit_code: job.derived_exit_code,
            priority: job.priority,
            reservation: job.reservation.clone(),
            comment: job.comment.clone(),
            array_job_id: job.array_job_id,
            array_task_id: job.array_task_id,
            requested_gpus: job.req_gpus,
            requested_gpus_detail: job.req_gpus_detail.clone(),
        }
    }
}

/// The priority breakdown `sprio` renders. Factors are computed by the caller
/// so that the text and structured renderers cannot disagree.
#[derive(Debug, Serialize)]
pub struct PriorityFactorsView {
    pub job_id: u32,
    pub user_name: String,
    pub partition: String,
    pub qos: String,
    pub priority: u32,
    pub age_factor: f64,
    pub fair_share_factor: f64,
    pub partition_tier: u32,
    pub effective_priority: u32,
}

/// Live resource usage for a running job, as reported by `sstat`.
///
/// The `ave_*`/`max_*` counters require per-process sampling on the agents,
/// which Spur does not collect yet; they serialize as `null` rather than the
/// `"N/A"` placeholder the text renderer prints.
#[derive(Debug, Serialize)]
pub struct JobStatisticsView {
    pub job_id: u32,
    pub job_state: String,
    pub elapsed: i64,
    pub tasks: u32,
    pub cpus: u32,
    pub memory_allocated_mb: u64,
    pub gpus_allocated: usize,
    pub nodes: String,
    pub ave_cpu: Option<f64>,
    pub ave_rss: Option<u64>,
    pub ave_vm_size: Option<u64>,
    pub max_rss: Option<u64>,
    pub max_vm_size: Option<u64>,
}

impl From<&JobInfo> for JobStatisticsView {
    fn from(job: &JobInfo) -> Self {
        let resources = job.resources.as_ref();
        Self {
            job_id: job.job_id,
            job_state: job_state_name(job.state),
            elapsed: duration_secs(job.run_time.as_ref()),
            tasks: job.num_tasks,
            cpus: job.num_tasks * job.cpus_per_task.max(1),
            memory_allocated_mb: resources.map(|r| r.memory_mb).unwrap_or(0),
            gpus_allocated: resources
                .and_then(|r| r.devices.get("gpu"))
                .map(|d| d.devices.len())
                .unwrap_or(0),
            nodes: job.nodelist.clone(),
            ave_cpu: None,
            ave_rss: None,
            ave_vm_size: None,
            max_rss: None,
            max_vm_size: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StepView {
    pub job_id: u32,
    pub step_id: u32,
    /// `batch`, `extern`, or the numeric step ID, matching `scontrol show steps`.
    pub step_name: String,
    pub name: String,
    pub state: String,
    pub tasks: u32,
}

/// Slurm's sentinel step IDs for the implicit batch and extern steps.
const STEP_ID_BATCH: u32 = 0xFFFF_FFFE;
const STEP_ID_EXTERN: u32 = 0xFFFF_FFFD;

/// Render a step ID the way Slurm names it: the implicit batch and extern steps
/// get names rather than their sentinel numbers.
pub fn step_id_label(step_id: u32) -> String {
    match step_id {
        STEP_ID_BATCH => "batch".to_string(),
        STEP_ID_EXTERN => "extern".to_string(),
        id => id.to_string(),
    }
}

impl From<&JobStepInfo> for StepView {
    fn from(step: &JobStepInfo) -> Self {
        Self {
            job_id: step.job_id,
            step_id: step.step_id,
            step_name: step_id_label(step.step_id),
            name: step.name.clone(),
            state: step.state.clone(),
            tasks: step.num_tasks,
        }
    }
}

pub fn job_state_name(state: i32) -> String {
    spur_core::job::JobState::from_proto_i32(state)
        .map(|s| s.display().to_string())
        .unwrap_or_else(|| "UNKNOWN".into())
}

/// A job's wall-clock limit in seconds, for callers that need it alongside the
/// minute-granularity view field.
pub fn time_limit_seconds(job: &JobInfo) -> Option<i64> {
    seconds(job.time_limit.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spur_proto::proto::JobState;

    fn sample() -> JobInfo {
        JobInfo {
            job_id: 42,
            name: "train".into(),
            user: "alice".into(),
            uid: 1000,
            partition: "gpu".into(),
            account: "research".into(),
            state: JobState::JobRunning as i32,
            state_reason: "None".into(),
            submit_time: Some(prost_types::Timestamp {
                seconds: 1_700_000_000,
                nanos: 0,
            }),
            start_time: Some(prost_types::Timestamp {
                seconds: 1_700_000_060,
                nanos: 0,
            }),
            time_limit: Some(prost_types::Duration {
                seconds: 3600,
                nanos: 0,
            }),
            run_time: Some(prost_types::Duration {
                seconds: 120,
                nanos: 0,
            }),
            num_nodes: 2,
            num_tasks: 4,
            cpus_per_task: 8,
            nodelist: "node[01-02]".into(),
            qos: "normal".into(),
            priority: 500,
            ..Default::default()
        }
    }

    #[test]
    fn job_view_carries_identity_and_state() {
        let view = JobView::from(&sample());
        assert_eq!(view.job_id, 42);
        assert_eq!(view.user_name, "alice");
        assert_eq!(view.job_state, "RUNNING");
        assert_eq!(view.nodes, "node[01-02]");
    }

    #[test]
    fn time_limit_is_reported_in_minutes() {
        let view = JobView::from(&sample());
        assert_eq!(view.time_limit, Some(60));
        assert_eq!(view.run_time, 120);
    }

    #[test]
    fn absent_times_and_limits_serialize_as_null() {
        let view = JobView::from(&JobInfo::default());
        assert_eq!(view.time_limit, None);
        assert_eq!(view.start_time, None);
        assert_eq!(view.end_time, None);

        let doc = serde_json::to_value(&view).unwrap();
        assert!(doc["time_limit"].is_null());
        assert!(doc["start_time"].is_null());
    }

    #[test]
    fn unknown_state_discriminant_falls_back_to_a_name() {
        assert_eq!(job_state_name(9999), "UNKNOWN");
    }

    #[test]
    fn statistics_view_derives_total_cpus_from_tasks() {
        let view = JobStatisticsView::from(&sample());
        assert_eq!(view.cpus, 32);
        assert_eq!(view.elapsed, 120);
        assert_eq!(view.gpus_allocated, 0);
    }

    #[test]
    fn unsampled_statistics_counters_are_null_not_placeholders() {
        let doc = serde_json::to_value(JobStatisticsView::from(&sample())).unwrap();
        assert!(doc["ave_cpu"].is_null());
        assert!(doc["max_rss"].is_null());
    }

    #[test]
    fn step_labels_name_the_implicit_steps() {
        assert_eq!(step_id_label(STEP_ID_BATCH), "batch");
        assert_eq!(step_id_label(STEP_ID_EXTERN), "extern");
        assert_eq!(step_id_label(3), "3");
    }
}
