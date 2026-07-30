// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use clap::Parser;
use spur_api::diag::{
    active_jobs, finished_jobs, job_count, node_count, rpc_statistics, success_rate,
    DiagnosticsView, ServerView,
};
use spur_api::DiagnosticsPayload;
use spur_core::job::JobState as CoreJobState;
use spur_core::node::NodeState as CoreNodeState;
use spur_proto::proto::{JobMetrics, NodeMetrics, PingResponse, RpcStats, SchedStats};

use crate::output::{self, OutputArgs, OutputFormat};

/// Display scheduler diagnostics and statistics.
#[derive(Parser, Debug)]
#[command(name = "sdiag", about = "Display scheduling diagnostics")]
pub struct SdiagArgs {
    /// Don't print header
    #[arg(long)]
    pub noheader: bool,

    /// Reset accumulated statistics counters on the controller
    #[arg(long)]
    pub reset: bool,

    #[command(flatten)]
    pub output: OutputArgs,

    /// Controller address
    #[arg(
        long,
        env = "SPUR_CONTROLLER_ADDR",
        default_value = "http://localhost:6817"
    )]
    pub controller: String,
}

pub async fn main() -> Result<()> {
    main_with_args(std::env::args().collect()).await
}

pub async fn main_with_args(argv: Vec<String>) -> Result<()> {
    let args = SdiagArgs::try_parse_from(&argv)?;

    let format = args.output.format()?;

    let channel = spur_client::connect_channel(&args.controller)
        .await
        .context("failed to connect to spurctld")?;
    let mut client = spur_proto::controller_client(channel);

    if args.reset {
        client
            .reset_diag_stats(())
            .await
            .context("failed to reset diagnostic statistics")?;
    }

    let ping_resp = client.ping(()).await.context("failed to ping controller")?;
    let ping = ping_resp.into_inner();

    let job_metrics = client
        .get_job_metrics(())
        .await
        .context("failed to get job metrics")?
        .into_inner();

    let node_metrics = client
        .get_node_metrics(())
        .await
        .context("failed to get node metrics")?
        .into_inner();

    let rpc_stats = client
        .get_rpc_stats(())
        .await
        .context("failed to get RPC statistics")?
        .into_inner();

    let sched_stats = client
        .get_sched_stats(())
        .await
        .context("failed to get scheduler statistics")?
        .into_inner();

    if let OutputFormat::Structured(encoding) = format {
        return output::emit(
            encoding,
            &argv,
            DiagnosticsPayload {
                statistics: DiagnosticsView {
                    server: ServerView::from(&ping),
                    jobs: (&job_metrics).into(),
                    nodes: (&node_metrics).into(),
                    scheduler: (&sched_stats).into(),
                    rpcs: rpc_statistics(&rpc_stats),
                },
            },
        );
    }

    for line in render_text(
        &args,
        &ping,
        &job_metrics,
        &node_metrics,
        &sched_stats,
        &rpc_stats,
    ) {
        println!("{line}");
    }

    Ok(())
}

fn render_text(
    args: &SdiagArgs,
    ping: &PingResponse,
    job_metrics: &JobMetrics,
    node_metrics: &NodeMetrics,
    sched_stats: &SchedStats,
    rpc_stats: &RpcStats,
) -> Vec<String> {
    let server_time = ping
        .server_time
        .as_ref()
        .map(|t| {
            chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32)
                .unwrap_or_default()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "N/A".into());

    let mut lines = Vec::new();

    if !args.noheader {
        lines.push("***********************************************".to_string());
        lines.push(format!("sdiag output at {}", server_time));
        lines.push("***********************************************".to_string());
        lines.push(String::new());
    }

    lines.push("Server Information:".to_string());
    lines.push(format!("  Hostname          : {}", ping.hostname));
    lines.push(format!("  Version           : {}", ping.version));
    lines.push(format!("  Server Time       : {}", server_time));

    if !ping.federation_peers.is_empty() {
        lines.push(format!(
            "  Federation Peers  : {}",
            ping.federation_peers.join(", ")
        ));
    }

    lines.extend(job_statistics_lines(job_metrics));
    lines.extend(node_statistics_lines(node_metrics));
    lines.extend(scheduler_statistics_lines(sched_stats));
    lines.extend(rpc_statistics_lines(rpc_stats));
    lines
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    if bytes >= GIB as u64 {
        format!("{:.1} GiB", bytes as f64 / GIB)
    } else if bytes >= MIB as u64 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= KIB as u64 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

fn job_statistics_lines(metrics: &JobMetrics) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        "Job Statistics:".to_string(),
        format!("  Total Jobs        : {}", metrics.total),
    ];

    for &state in &CoreJobState::ALL {
        lines.push(format!(
            "  {:18}: {}",
            state.display(),
            job_count(metrics, state)
        ));
    }

    if metrics.held_pending > 0 {
        lines.push(format!("  Held (pending)    : {}", metrics.held_pending));
    }

    lines.push(format!("  CPUs Allocated    : {}", metrics.running_cpus));
    lines.push(format!(
        "  Memory Allocated  : {}",
        format_bytes(metrics.running_memory_bytes)
    ));
    lines.push(format!("  GPUs Allocated    : {}", metrics.running_gpus));

    lines.push(String::new());
    lines.push("Derived Statistics:".to_string());
    lines.push(format!("  Finished Jobs     : {}", finished_jobs(metrics)));
    lines.push(format!(
        "  Success Rate      : {:.1}%",
        success_rate(metrics)
    ));
    lines.push(format!(
        "  Active Jobs       : {} (running + completing + suspended)",
        active_jobs(metrics)
    ));
    lines
}

fn node_statistics_lines(metrics: &NodeMetrics) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        "Node Statistics:".to_string(),
        format!("  Total Nodes       : {}", metrics.total),
    ];

    for &state in &CoreNodeState::ALL {
        lines.push(format!(
            "  {:18}: {}",
            state.display_upper(),
            node_count(metrics, state)
        ));
    }

    lines.push(format!("  Total CPUs        : {}", metrics.total_cpus));
    lines.push(format!("  Allocated CPUs    : {}", metrics.alloc_cpus));
    lines.push(format!(
        "  Total Memory      : {}",
        format_bytes(metrics.total_memory_bytes)
    ));
    lines.push(format!(
        "  Allocated Memory  : {}",
        format_bytes(metrics.alloc_memory_bytes)
    ));
    lines.push(format!("  Total GPUs        : {}", metrics.total_gpus));
    lines.push(format!("  Allocated GPUs    : {}", metrics.alloc_gpus));
    lines
}

fn scheduler_statistics_lines(stats: &SchedStats) -> Vec<String> {
    vec![
        String::new(),
        "Scheduler Statistics:".to_string(),
        format!("  Plugin              : {}", stats.plugin),
        format!("  Cycles              : {}", stats.cycles),
        format!("  Cycle last (us)     : {}", stats.cycle_last_time_us),
        format!("  Cycle total (us)    : {}", stats.cycle_total_time_us),
        format!("  Cycle avg (us)      : {}", stats.cycle_avg_time_us),
        format!("  Schedule last (us)  : {}", stats.schedule_last_time_us),
        format!("  Schedule total (us) : {}", stats.schedule_total_time_us),
        format!("  Schedule avg (us)   : {}", stats.schedule_avg_time_us),
        format!("  Jobs submitted      : {}", stats.jobs_submitted),
        format!("  Jobs started        : {}", stats.jobs_started),
        format!("  Jobs finalized      : {}", stats.jobs_finalized),
        format!("  Jobs started (last) : {}", stats.jobs_started_last_cycle),
        format!("  Exit end of queue   : {}", stats.exit_end),
        format!("  Exit max depth      : {}", stats.exit_max_depth),
    ]
}

fn rpc_statistics_lines(stats: &RpcStats) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        "Remote Procedure Call statistics by operation:".to_string(),
    ];

    let ops = rpc_statistics(stats);
    if ops.is_empty() {
        lines.push("  (no RPC calls recorded)".to_string());
        return lines;
    }

    lines.extend(ops.iter().map(|op| {
        format!(
            "  {:24} count:{:8}  ave_time_us:{:8}  total_time_us:{}",
            op.operation, op.count, op.average_time_us, op.total_time_us
        )
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use spur_proto::proto::{JobState, JobStateCount, RpcOperationStats};

    #[test]
    fn structured_output_flags_are_accepted() {
        let args = SdiagArgs::try_parse_from(["sdiag", "--json"]).unwrap();
        assert_eq!(
            args.output.format().unwrap(),
            crate::output::OutputFormat::Structured(crate::output::Encoding::Json)
        );

        let args = SdiagArgs::try_parse_from(["sdiag", "--yaml"]).unwrap();
        assert_eq!(
            args.output.format().unwrap(),
            crate::output::OutputFormat::Structured(crate::output::Encoding::Yaml)
        );
    }

    #[test]
    fn json_document_nests_every_section_under_statistics() {
        let view = DiagnosticsView {
            server: ServerView::from(&PingResponse {
                hostname: "ctld01".into(),
                version: "0.6.0".into(),
                ..Default::default()
            }),
            jobs: (&JobMetrics::default()).into(),
            nodes: (&NodeMetrics::default()).into(),
            scheduler: (&SchedStats {
                plugin: "backfill".into(),
                cycles: 5,
                ..Default::default()
            })
                .into(),
            rpcs: rpc_statistics(&RpcStats::default()),
        };
        let doc = crate::output::render(
            crate::output::Encoding::Json,
            &["sdiag".to_string()],
            DiagnosticsPayload { statistics: view },
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(parsed["statistics"]["server"]["hostname"], "ctld01");
        assert_eq!(parsed["statistics"]["scheduler"]["plugin"], "backfill");
        assert_eq!(parsed["statistics"]["scheduler"]["cycles"], 5);
        assert_eq!(parsed["statistics"]["rpcs"], serde_json::json!([]));
        assert!(parsed["statistics"]["jobs"]["by_state"].is_object());
    }

    #[test]
    fn text_rendering_includes_the_banner_unless_suppressed() {
        let ping = PingResponse {
            hostname: "ctld01".into(),
            ..Default::default()
        };
        let render = |argv: &[&str]| {
            render_text(
                &SdiagArgs::try_parse_from(argv).unwrap(),
                &ping,
                &JobMetrics::default(),
                &NodeMetrics::default(),
                &SchedStats::default(),
                &RpcStats::default(),
            )
        };
        assert!(render(&["sdiag"])[0].starts_with("****"));
        assert_eq!(render(&["sdiag", "--noheader"])[0], "Server Information:");
    }

    #[test]
    fn format_bytes_uses_binary_units() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(512), "512 bytes");
        assert_eq!(format_bytes(8_388_608), "8.0 MiB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
    }

    #[test]
    fn job_statistics_lines_report_every_state_and_the_derived_totals() {
        let metrics = JobMetrics {
            total: 3,
            by_state: vec![
                JobStateCount {
                    state: JobState::JobRunning as i32,
                    count: 1,
                },
                JobStateCount {
                    state: JobState::JobCompleted as i32,
                    count: 2,
                },
            ],
            ..Default::default()
        };
        let lines = job_statistics_lines(&metrics);
        assert!(lines
            .iter()
            .any(|l| l.contains("RUNNING") && l.ends_with('1')));
        assert!(lines.iter().any(|l| l.contains("Finished Jobs     : 2")));
        assert!(lines
            .iter()
            .any(|l| l.contains("Success Rate      : 100.0%")));
        assert!(lines.iter().any(|l| l.contains("Active Jobs       : 1")));
    }

    #[test]
    fn node_statistics_lines_report_capacity_and_allocation() {
        let metrics = NodeMetrics {
            total: 2,
            total_cpus: 256,
            alloc_cpus: 64,
            ..Default::default()
        };
        let lines = node_statistics_lines(&metrics);
        assert!(lines.iter().any(|l| l.contains("Total Nodes       : 2")));
        assert!(lines.iter().any(|l| l.contains("Total CPUs        : 256")));
        assert!(lines.iter().any(|l| l.contains("Allocated CPUs    : 64")));
        assert!(lines.iter().any(|l| l.contains("IDLE")));
    }

    #[test]
    fn scheduler_statistics_lines_include_cycle_and_lifecycle_fields() {
        let stats = SchedStats {
            plugin: "backfill".into(),
            cycles: 5,
            cycle_total_time_us: 2500,
            cycle_last_time_us: 600,
            cycle_avg_time_us: 500,
            schedule_total_time_us: 800,
            schedule_last_time_us: 200,
            schedule_avg_time_us: 160,
            jobs_submitted: 10,
            jobs_started: 8,
            jobs_finalized: 7,
            jobs_started_last_cycle: 2,
            exit_end: 4,
            exit_max_depth: 1,
        };
        let lines = scheduler_statistics_lines(&stats);
        assert!(lines.iter().any(|l| l.contains("Plugin")));
        assert!(lines.iter().any(|l| l.contains("Cycles")));
        assert!(lines.iter().any(|l| l.contains("Cycle total (us)")));
        assert!(lines.iter().any(|l| l.contains("Schedule total (us)")));
        assert!(lines.iter().any(|l| l.contains("Jobs started (last) : 2")));
        assert!(lines.iter().any(|l| l.contains("Exit end of queue   : 4")));
        assert!(lines.iter().any(|l| l.contains("Exit max depth      : 1")));
    }

    #[test]
    fn rpc_statistics_lines_empty_state() {
        let lines = rpc_statistics_lines(&RpcStats::default());
        assert!(lines.iter().any(|l| l == "  (no RPC calls recorded)"));
        assert!(lines
            .iter()
            .any(|l| l == "Remote Procedure Call statistics by operation:"));
    }

    #[test]
    fn rpc_statistics_lines_sorted_by_total_time_us() {
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
        let lines = rpc_statistics_lines(&stats);
        let data_lines: Vec<&String> = lines.iter().filter(|l| l.contains("count:")).collect();
        assert_eq!(data_lines.len(), 2);
        assert!(data_lines[0].contains("SubmitJob"));
        assert!(data_lines[1].contains("GetJobs"));
        assert!(data_lines[0].contains("ave_time_us:"));
        assert!(data_lines[0].contains("total_time_us:3000"));
    }
}
