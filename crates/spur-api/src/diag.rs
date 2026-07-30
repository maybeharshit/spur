// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Controller diagnostics, as published by `sdiag`.

use std::collections::BTreeMap;

use serde::Serialize;
use spur_core::job::JobState as CoreJobState;
use spur_core::node::NodeState as CoreNodeState;
use spur_proto::proto::{JobMetrics, NodeMetrics, PingResponse, RpcStats, SchedStats};

use crate::util::ts_secs;

#[derive(Debug, Serialize)]
pub struct DiagnosticsView {
    pub server: ServerView,
    pub jobs: JobStatisticsSummary,
    pub nodes: NodeStatisticsSummary,
    pub scheduler: SchedulerStatisticsView,
    pub rpcs: Vec<RpcStatisticsView>,
}

#[derive(Debug, Serialize)]
pub struct ServerView {
    pub hostname: String,
    pub version: String,
    pub server_time: Option<i64>,
    pub federation_peers: Vec<String>,
}

impl From<&PingResponse> for ServerView {
    fn from(ping: &PingResponse) -> Self {
        Self {
            hostname: ping.hostname.clone(),
            version: ping.version.clone(),
            server_time: ts_secs(ping.server_time.as_ref()),
            federation_peers: ping.federation_peers.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JobStatisticsSummary {
    pub total: u64,
    /// Counts keyed by state name, e.g. `{"RUNNING": 3}`.
    pub by_state: BTreeMap<String, u64>,
    pub held_pending: u64,
    pub cpus_allocated: u64,
    pub memory_allocated_bytes: u64,
    pub gpus_allocated: u64,
    pub finished: u64,
    pub active: u64,
    pub success_rate: f64,
}

impl From<&JobMetrics> for JobStatisticsSummary {
    fn from(metrics: &JobMetrics) -> Self {
        Self {
            total: metrics.total,
            by_state: CoreJobState::ALL
                .iter()
                .map(|s| (s.display().to_string(), job_count(metrics, *s)))
                .collect(),
            held_pending: metrics.held_pending,
            cpus_allocated: metrics.running_cpus,
            memory_allocated_bytes: metrics.running_memory_bytes,
            gpus_allocated: metrics.running_gpus,
            finished: finished_jobs(metrics),
            active: active_jobs(metrics),
            success_rate: success_rate(metrics),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NodeStatisticsSummary {
    pub total: u64,
    pub by_state: BTreeMap<String, u64>,
    pub total_cpus: u64,
    pub alloc_cpus: u64,
    pub total_memory_bytes: u64,
    pub alloc_memory_bytes: u64,
    pub total_gpus: u64,
    pub alloc_gpus: u64,
}

impl From<&NodeMetrics> for NodeStatisticsSummary {
    fn from(metrics: &NodeMetrics) -> Self {
        Self {
            total: metrics.total,
            by_state: CoreNodeState::ALL
                .iter()
                .map(|s| (s.display_upper().to_string(), node_count(metrics, *s)))
                .collect(),
            total_cpus: metrics.total_cpus,
            alloc_cpus: metrics.alloc_cpus,
            total_memory_bytes: metrics.total_memory_bytes,
            alloc_memory_bytes: metrics.alloc_memory_bytes,
            total_gpus: metrics.total_gpus,
            alloc_gpus: metrics.alloc_gpus,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SchedulerStatisticsView {
    pub plugin: String,
    pub cycles: u64,
    pub cycle_last_time_us: u64,
    pub cycle_total_time_us: u64,
    pub cycle_avg_time_us: u64,
    pub schedule_last_time_us: u64,
    pub schedule_total_time_us: u64,
    pub schedule_avg_time_us: u64,
    pub jobs_submitted: u64,
    pub jobs_started: u64,
    pub jobs_finalized: u64,
    pub jobs_started_last_cycle: u64,
    pub exit_end: u64,
    pub exit_max_depth: u64,
}

impl From<&SchedStats> for SchedulerStatisticsView {
    fn from(stats: &SchedStats) -> Self {
        Self {
            plugin: stats.plugin.clone(),
            cycles: stats.cycles,
            cycle_last_time_us: stats.cycle_last_time_us,
            cycle_total_time_us: stats.cycle_total_time_us,
            cycle_avg_time_us: stats.cycle_avg_time_us,
            schedule_last_time_us: stats.schedule_last_time_us,
            schedule_total_time_us: stats.schedule_total_time_us,
            schedule_avg_time_us: stats.schedule_avg_time_us,
            jobs_submitted: stats.jobs_submitted,
            jobs_started: stats.jobs_started,
            jobs_finalized: stats.jobs_finalized,
            jobs_started_last_cycle: stats.jobs_started_last_cycle,
            exit_end: stats.exit_end,
            exit_max_depth: stats.exit_max_depth,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RpcStatisticsView {
    pub operation: String,
    pub count: u64,
    pub average_time_us: u64,
    pub total_time_us: u64,
}

/// RPC statistics ordered by total time descending, matching the text renderer.
pub fn rpc_statistics(stats: &RpcStats) -> Vec<RpcStatisticsView> {
    let mut ops: Vec<&spur_proto::proto::RpcOperationStats> = stats.by_operation.iter().collect();
    ops.sort_by_key(|op| std::cmp::Reverse(op.total_time_us));
    ops.into_iter()
        .map(|op| RpcStatisticsView {
            operation: op.operation.clone(),
            count: op.count,
            average_time_us: op.avg_time_us,
            total_time_us: op.total_time_us,
        })
        .collect()
}

pub fn job_count(metrics: &JobMetrics, state: CoreJobState) -> u64 {
    let wire = state.to_proto_i32();
    metrics
        .by_state
        .iter()
        .find(|e| e.state == wire)
        .map(|e| e.count)
        .unwrap_or(0)
}

pub fn node_count(metrics: &NodeMetrics, state: CoreNodeState) -> u64 {
    let wire = state.to_proto_i32();
    metrics
        .by_state
        .iter()
        .find(|e| e.state == wire)
        .map(|e| e.count)
        .unwrap_or(0)
}

pub fn finished_jobs(metrics: &JobMetrics) -> u64 {
    CoreJobState::ALL
        .iter()
        .filter(|s| s.is_terminal())
        .map(|s| job_count(metrics, *s))
        .sum()
}

pub fn active_jobs(metrics: &JobMetrics) -> u64 {
    CoreJobState::ALL
        .iter()
        .filter(|s| s.is_active())
        .map(|s| job_count(metrics, *s))
        .sum()
}

/// Share of finished jobs that completed successfully, as a percentage.
pub fn success_rate(metrics: &JobMetrics) -> f64 {
    let finished = finished_jobs(metrics);
    if finished == 0 {
        return 0.0;
    }
    (job_count(metrics, CoreJobState::Completed) as f64 / finished as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use spur_proto::proto::{JobState, JobStateCount, RpcOperationStats};

    fn metrics() -> JobMetrics {
        JobMetrics {
            total: 6,
            by_state: vec![
                JobStateCount {
                    state: JobState::JobPending as i32,
                    count: 1,
                },
                JobStateCount {
                    state: JobState::JobRunning as i32,
                    count: 1,
                },
                JobStateCount {
                    state: JobState::JobSuspended as i32,
                    count: 1,
                },
                JobStateCount {
                    state: JobState::JobNodeFail as i32,
                    count: 1,
                },
                JobStateCount {
                    state: JobState::JobCompleted as i32,
                    count: 2,
                },
            ],
            held_pending: 0,
            running_cpus: 4,
            running_memory_bytes: 0,
            running_gpus: 0,
        }
    }

    #[test]
    fn derived_totals_use_terminal_and_active_flags() {
        assert_eq!(finished_jobs(&metrics()), 3);
        assert_eq!(active_jobs(&metrics()), 2);
    }

    #[test]
    fn success_rate_is_completed_over_finished() {
        // 2 completed of 3 finished.
        assert!((success_rate(&metrics()) - 66.666).abs() < 0.01);
    }

    #[test]
    fn success_rate_of_an_idle_cluster_is_zero_not_nan() {
        assert_eq!(success_rate(&JobMetrics::default()), 0.0);
    }

    #[test]
    fn job_summary_keys_states_by_name() {
        let summary = JobStatisticsSummary::from(&metrics());
        assert_eq!(summary.by_state.get("RUNNING"), Some(&1));
        assert_eq!(summary.by_state.get("COMPLETED"), Some(&2));
        // Every known state is present, so consumers can index without checking.
        assert_eq!(summary.by_state.len(), CoreJobState::ALL.len());
    }

    #[test]
    fn node_summary_keys_states_by_name() {
        let summary = NodeStatisticsSummary::from(&NodeMetrics::default());
        assert_eq!(summary.by_state.get("IDLE"), Some(&0));
        assert_eq!(summary.by_state.len(), CoreNodeState::ALL.len());
    }

    #[test]
    fn rpc_statistics_sort_by_total_time_descending() {
        let stats = RpcStats {
            by_operation: vec![
                RpcOperationStats {
                    operation: "GetJobs".into(),
                    count: 5,
                    total_time_us: 250,
                    avg_time_us: 50,
                },
                RpcOperationStats {
                    operation: "SubmitJob".into(),
                    count: 2,
                    total_time_us: 3000,
                    avg_time_us: 1500,
                },
            ],
        };
        let views = rpc_statistics(&stats);
        assert_eq!(views[0].operation, "SubmitJob");
        assert_eq!(views[1].operation, "GetJobs");
    }
}
