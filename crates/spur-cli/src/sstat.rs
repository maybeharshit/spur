// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Context, Result};
use clap::Parser;
use spur_api::{JobStatisticsPayload, JobStatisticsView};
use spur_proto::proto::{GetJobRequest, JobInfo};

use crate::output::{self, OutputArgs, OutputFormat};

/// Display status information for running jobs.
#[derive(Parser, Debug)]
#[command(name = "sstat", about = "Display status of running jobs")]
pub struct SstatArgs {
    /// Job ID to query
    #[arg(short = 'j', long = "jobs", required = true)]
    pub job_id: String,

    /// Output format (comma-separated field names)
    #[arg(short = 'o', long)]
    pub format: Option<String>,

    /// Don't print header
    #[arg(long)]
    pub noheader: bool,

    /// Parsable output (delimiter-separated)
    #[arg(short = 'p', long)]
    pub parsable: bool,

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
    let args = SstatArgs::try_parse_from(&argv)?;

    let output_format = args.output.format()?;

    // Parse job IDs (comma-separated)
    let job_ids: Vec<u32> = args
        .job_id
        .split(',')
        .filter_map(|j| j.trim().parse::<u32>().ok())
        .collect();

    if job_ids.is_empty() {
        bail!("sstat: no valid job IDs specified");
    }

    let channel = spur_client::connect_channel(&args.controller)
        .await
        .context("failed to connect to spurctld")?;
    let mut client = spur_proto::controller_client(channel);

    let mut running = Vec::new();
    for job_id in &job_ids {
        let response = client
            .get_job(GetJobRequest { job_id: *job_id })
            .await
            .context(format!("failed to get job {}", job_id))?;

        let job = response.into_inner();

        // sstat reports live usage, which only exists for running jobs.
        if job.state != spur_proto::proto::JobState::JobRunning as i32 {
            eprintln!(
                "sstat: job {} is not running (state: {})",
                job_id,
                state_name(job.state)
            );
            continue;
        }

        running.push(job);
    }

    if let OutputFormat::Structured(encoding) = output_format {
        return output::emit(
            encoding,
            &argv,
            JobStatisticsPayload {
                statistics: running.iter().map(JobStatisticsView::from).collect(),
            },
        );
    }

    for line in render_text(&args, &running) {
        println!("{line}");
    }

    Ok(())
}

fn render_text(args: &SstatArgs, jobs: &[JobInfo]) -> Vec<String> {
    let fields = match args.format {
        Some(ref fmt) => parse_field_list(fmt),
        None => default_fields(),
    };
    let delimiter = if args.parsable { "|" } else { "  " };

    let mut lines = Vec::new();

    if !args.noheader {
        let headers: Vec<String> = fields.iter().map(format_header).collect();
        if args.parsable {
            lines.push(format!("{}|", headers.join(delimiter)));
        } else {
            lines.push(headers.join(delimiter));
            let sep: Vec<String> = fields.iter().map(|f| "-".repeat(field_width(f))).collect();
            lines.push(sep.join(delimiter));
        }
    }

    for job in jobs {
        let values: Vec<String> = fields.iter().map(|f| resolve_field(job, f)).collect();
        if args.parsable {
            lines.push(format!("{}|", values.join(delimiter)));
            continue;
        }
        let padded: Vec<String> = fields
            .iter()
            .zip(values.iter())
            .map(|(f, v)| format!("{:>width$}", v, width = field_width(f)))
            .collect();
        lines.push(padded.join(delimiter));
    }

    lines
}

#[derive(Debug, Clone, PartialEq)]
enum StatField {
    JobId,
    AveCpu,
    AveRss,
    AveVmSize,
    MaxRss,
    MaxVmSize,
    NTasks,
    NodeList,
    Cpus,
    MemAlloc,
    GpuAlloc,
    State,
    Elapsed,
}

fn default_fields() -> Vec<StatField> {
    vec![
        StatField::JobId,
        StatField::NTasks,
        StatField::Cpus,
        StatField::MemAlloc,
        StatField::GpuAlloc,
        StatField::Elapsed,
        StatField::NodeList,
    ]
}

fn parse_field_list(fmt: &str) -> Vec<StatField> {
    fmt.split(',')
        .filter_map(|name| match name.trim().to_lowercase().as_str() {
            "jobid" => Some(StatField::JobId),
            "avecpu" => Some(StatField::AveCpu),
            "averss" => Some(StatField::AveRss),
            "avevmsize" => Some(StatField::AveVmSize),
            "maxrss" => Some(StatField::MaxRss),
            "maxvmsize" => Some(StatField::MaxVmSize),
            "ntasks" => Some(StatField::NTasks),
            "nodelist" => Some(StatField::NodeList),
            "cpus" | "ncpus" => Some(StatField::Cpus),
            "memalloc" | "reqmem" => Some(StatField::MemAlloc),
            "gpualloc" | "gres" => Some(StatField::GpuAlloc),
            "state" => Some(StatField::State),
            "elapsed" => Some(StatField::Elapsed),
            _ => {
                eprintln!("sstat: unknown field '{}'", name.trim());
                None
            }
        })
        .collect()
}

fn format_header(field: &StatField) -> String {
    let (name, width) = header_info(field);
    format!("{:>width$}", name, width = width)
}

fn field_width(field: &StatField) -> usize {
    header_info(field).1
}

fn header_info(field: &StatField) -> (&'static str, usize) {
    match field {
        StatField::JobId => ("JobID", 10),
        StatField::AveCpu => ("AveCPU", 10),
        StatField::AveRss => ("AveRSS", 10),
        StatField::AveVmSize => ("AveVMSize", 10),
        StatField::MaxRss => ("MaxRSS", 10),
        StatField::MaxVmSize => ("MaxVMSize", 10),
        StatField::NTasks => ("NTasks", 8),
        StatField::NodeList => ("Nodelist", 20),
        StatField::Cpus => ("NCPUS", 8),
        StatField::MemAlloc => ("MemAlloc", 10),
        StatField::GpuAlloc => ("GPUAlloc", 10),
        StatField::State => ("State", 10),
        StatField::Elapsed => ("Elapsed", 12),
    }
}

fn resolve_field(job: &spur_proto::proto::JobInfo, field: &StatField) -> String {
    match field {
        StatField::JobId => job.job_id.to_string(),
        StatField::NTasks => job.num_tasks.to_string(),
        StatField::Cpus => {
            let cpus = job.num_tasks * job.cpus_per_task.max(1);
            cpus.to_string()
        }
        StatField::MemAlloc => {
            if let Some(ref res) = job.resources {
                format!("{}M", res.memory_mb)
            } else {
                "0M".into()
            }
        }
        StatField::GpuAlloc => {
            if let Some(ref res) = job.resources {
                res.devices
                    .get("gpu")
                    .map(|d| d.devices.len().to_string())
                    .unwrap_or_else(|| "0".into())
            } else {
                "0".into()
            }
        }
        StatField::NodeList => job.nodelist.clone(),
        StatField::State => state_name(job.state).to_string(),
        StatField::Elapsed => {
            if let Some(ref rt) = job.run_time {
                format_duration(rt.seconds)
            } else {
                "00:00:00".into()
            }
        }
        // These fields would require real-time process stats from the agent.
        // For now, show N/A since we don't poll agents for per-process metrics.
        StatField::AveCpu => "N/A".into(),
        StatField::AveRss => "N/A".into(),
        StatField::AveVmSize => "N/A".into(),
        StatField::MaxRss => "N/A".into(),
        StatField::MaxVmSize => "N/A".into(),
    }
}

fn state_name(state: i32) -> &'static str {
    spur_core::job::JobState::from_proto_i32(state)
        .map(|s| s.display())
        .unwrap_or("UNKNOWN")
}

fn format_duration(total_seconds: i64) -> String {
    let total_seconds = total_seconds.unsigned_abs();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if days > 0 {
        format!("{}-{:02}:{:02}:{:02}", days, hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spur_proto::proto::JobState;

    fn running(job_id: u32) -> JobInfo {
        JobInfo {
            job_id,
            state: JobState::JobRunning as i32,
            num_tasks: 4,
            cpus_per_task: 8,
            nodelist: "node[01-02]".into(),
            run_time: Some(prost_types::Duration {
                seconds: 3661,
                nanos: 0,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn structured_output_flags_are_accepted() {
        let args = SstatArgs::try_parse_from(["sstat", "-j", "1", "--json"]).unwrap();
        assert_eq!(
            args.output.format().unwrap(),
            crate::output::OutputFormat::Structured(crate::output::Encoding::Json)
        );

        let args = SstatArgs::try_parse_from(["sstat", "-j", "1", "--yaml"]).unwrap();
        assert_eq!(
            args.output.format().unwrap(),
            crate::output::OutputFormat::Structured(crate::output::Encoding::Yaml)
        );
    }

    #[test]
    fn json_document_carries_usage_under_a_statistics_key() {
        let doc = crate::output::render(
            crate::output::Encoding::Json,
            &["sstat".to_string()],
            JobStatisticsPayload {
                statistics: vec![JobStatisticsView::from(&running(5))],
            },
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(parsed["statistics"][0]["job_id"], 5);
        assert_eq!(parsed["statistics"][0]["cpus"], 32);
        assert_eq!(parsed["statistics"][0]["elapsed"], 3661);
    }

    #[test]
    fn text_rendering_emits_a_header_and_separator_above_the_rows() {
        let args = SstatArgs::try_parse_from(["sstat", "-j", "5"]).unwrap();
        let lines = render_text(&args, &[running(5)]);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("JobID"));
        assert!(lines[1].contains("---"));
        assert!(lines[2].contains('5'));
    }

    #[test]
    fn parsable_output_uses_pipes_and_omits_the_separator() {
        let args = SstatArgs::try_parse_from(["sstat", "-j", "5", "-p"]).unwrap();
        let lines = render_text(&args, &[running(5)]);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with('|'));
        assert!(lines[1].contains("node[01-02]|"));
    }

    #[test]
    fn noheader_drops_the_header_and_its_separator() {
        let args = SstatArgs::try_parse_from(["sstat", "-j", "5", "--noheader"]).unwrap();
        let lines = render_text(&args, &[running(5)]);
        assert_eq!(lines.len(), 1);
    }

    #[tokio::test]
    async fn non_numeric_job_ids_fail_before_connecting() {
        let err = main_with_args(vec!["sstat".into(), "-j".into(), "abc".into()])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no valid job IDs"));
    }
}
