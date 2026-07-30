// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use clap::Parser;
use spur_api::{PriorityFactorsPayload, PriorityFactorsView};
use spur_proto::proto::{GetJobsRequest, GetPartitionsRequest, JobInfo, JobState, PartitionInfo};

use crate::output::{self, OutputArgs, OutputFormat};

/// View job priority breakdown for pending jobs.
#[derive(Parser, Debug)]
// -h is sprio's --noheader (Slurm convention), so disable clap's auto -h and
// re-add --help below as long-only.
#[command(
    name = "sprio",
    about = "View job priority factors",
    disable_help_flag = true
)]
pub struct SprioArgs {
    /// Show only these job IDs (comma-separated)
    #[arg(short = 'j', long)]
    pub jobs: Option<String>,

    /// Show only jobs for this user
    #[arg(short = 'u', long)]
    pub user: Option<String>,

    /// Long format (more detail)
    #[arg(short = 'l', long)]
    pub long: bool,

    /// Don't print header
    #[arg(short = 'h', long)]
    pub noheader: bool,

    /// Print help
    #[arg(long, action = clap::ArgAction::Help)]
    pub help: Option<bool>,

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
    let args = SprioArgs::try_parse_from(&argv)?;

    let format = args.output.format()?;

    // Parse job ID filter
    let job_ids = args
        .jobs
        .as_ref()
        .map(|s| {
            s.split(',')
                .filter_map(|j| j.trim().parse::<u32>().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let channel = spur_client::connect_channel(&args.controller)
        .await
        .context("failed to connect to spurctld")?;
    let mut client = spur_proto::controller_client(channel);

    // Get pending jobs only (priority is relevant for pending jobs)
    let response = client
        .get_jobs(GetJobsRequest {
            states: vec![JobState::JobPending as i32],
            user: args.user.clone().unwrap_or_default(),
            partition: String::new(),
            account: String::new(),
            job_ids,
            name: String::new(),
        })
        .await
        .context("failed to get jobs")?;

    let jobs = response.into_inner().jobs;

    // Get partitions for tier lookup
    let partitions_resp = client
        .get_partitions(GetPartitionsRequest {
            name: String::new(),
        })
        .await
        .context("failed to get partitions")?;

    let partitions = partitions_resp.into_inner().partitions;

    let factors = priority_factors(&jobs, &partitions, chrono::Utc::now().timestamp());

    if let OutputFormat::Structured(encoding) = format {
        return output::emit(encoding, &argv, PriorityFactorsPayload { jobs: factors });
    }

    for line in render_text(&args, &factors) {
        println!("{line}");
    }

    Ok(())
}

/// A week of queue time is the point at which the age factor saturates.
const AGE_FACTOR_SATURATION_MINUTES: f64 = 10080.0;

/// Fair share is approximated as 1.0 until the CLI has access to usage data.
const ASSUMED_FAIR_SHARE: f64 = 1.0;

/// Break each job's priority down into the factors that produced it. Computed
/// once so the text and structured renderers cannot disagree.
fn priority_factors(
    jobs: &[JobInfo],
    partitions: &[PartitionInfo],
    now_secs: i64,
) -> Vec<PriorityFactorsView> {
    jobs.iter()
        .map(|job| {
            let submit_secs = job
                .submit_time
                .as_ref()
                .map(|t| t.seconds)
                .unwrap_or(now_secs);
            let age_minutes = (now_secs - submit_secs) / 60;
            let age_factor = 1.0 + (age_minutes as f64 / AGE_FACTOR_SATURATION_MINUTES).min(1.0);

            let partition_tier = partitions
                .iter()
                .find(|p| p.name == job.partition)
                .map(|p| p.priority_tier)
                .unwrap_or(1);

            let effective = (job.priority as f64
                * ASSUMED_FAIR_SHARE.min(10.0)
                * age_factor
                * partition_tier.max(1) as f64) as u32;

            PriorityFactorsView {
                job_id: job.job_id,
                user_name: job.user.clone(),
                partition: job.partition.clone(),
                qos: if job.qos.is_empty() {
                    "normal".to_string()
                } else {
                    job.qos.clone()
                },
                priority: job.priority,
                age_factor,
                fair_share_factor: ASSUMED_FAIR_SHARE,
                partition_tier,
                effective_priority: effective,
            }
        })
        .collect()
}

fn render_text(args: &SprioArgs, factors: &[PriorityFactorsView]) -> Vec<String> {
    let mut lines = Vec::new();

    if args.long {
        if !args.noheader {
            lines.push(format!(
                "{:>10} {:>10} {:>10} {:>10} {:>10} {:>12} {:>10} {:>10}",
                "JOBID", "USER", "PRIORITY", "AGE", "FAIRSHARE", "PARTITION", "QOS", "EFFECTIVE"
            ));
        }
        lines.extend(factors.iter().map(|f| {
            format!(
                "{:>10} {:>10} {:>10} {:>10.4} {:>10.4} {:>12} {:>10} {:>10}",
                f.job_id,
                f.user_name,
                f.priority,
                f.age_factor,
                f.fair_share_factor,
                format!("{}(T{})", f.partition, f.partition_tier),
                f.qos,
                f.effective_priority,
            )
        }));
        return lines;
    }

    if !args.noheader {
        lines.push(format!(
            "{:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
            "JOBID", "USER", "PRIORITY", "AGE", "FAIRSHARE", "PARTITION"
        ));
    }
    lines.extend(factors.iter().map(|f| {
        format!(
            "{:>10} {:>10} {:>10} {:>10.4} {:>10.4} {:>10}",
            f.job_id, f.user_name, f.priority, f.age_factor, f.fair_share_factor, f.partition_tier,
        )
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(job_id: u32, partition: &str, priority: u32, submit_secs: i64) -> JobInfo {
        JobInfo {
            job_id,
            user: "alice".into(),
            partition: partition.into(),
            priority,
            submit_time: Some(prost_types::Timestamp {
                seconds: submit_secs,
                nanos: 0,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn a_freshly_submitted_job_has_the_minimum_age_factor() {
        let factors = priority_factors(&[pending(1, "gpu", 100, 1_000)], &[], 1_000);
        assert_eq!(factors[0].age_factor, 1.0);
        assert_eq!(factors[0].effective_priority, 100);
    }

    #[test]
    fn the_age_factor_saturates_after_a_week() {
        let week = (AGE_FACTOR_SATURATION_MINUTES as i64) * 60;
        let factors = priority_factors(&[pending(1, "gpu", 100, 0)], &[], week * 4);
        assert_eq!(factors[0].age_factor, 2.0);
        assert_eq!(factors[0].effective_priority, 200);
    }

    #[test]
    fn partition_tier_multiplies_effective_priority() {
        let partitions = vec![PartitionInfo {
            name: "gpu".into(),
            priority_tier: 3,
            ..Default::default()
        }];
        let factors = priority_factors(&[pending(1, "gpu", 100, 0)], &partitions, 0);
        assert_eq!(factors[0].partition_tier, 3);
        assert_eq!(factors[0].effective_priority, 300);
    }

    #[test]
    fn an_unknown_partition_falls_back_to_tier_one() {
        let factors = priority_factors(&[pending(1, "missing", 50, 0)], &[], 0);
        assert_eq!(factors[0].partition_tier, 1);
        assert_eq!(factors[0].effective_priority, 50);
    }

    #[test]
    fn an_empty_qos_is_reported_as_normal() {
        let factors = priority_factors(&[pending(1, "gpu", 1, 0)], &[], 0);
        assert_eq!(factors[0].qos, "normal");
    }

    #[test]
    fn long_output_adds_qos_and_effective_columns() {
        let args = SprioArgs::try_parse_from(["sprio", "-l"]).unwrap();
        let factors = priority_factors(&[pending(7, "gpu", 100, 0)], &[], 0);
        let lines = render_text(&args, &factors);
        assert!(lines[0].contains("EFFECTIVE"));
        assert!(lines[0].contains("QOS"));
        assert!(lines[1].contains("gpu(T1)"));
    }

    #[test]
    fn short_h_is_noheader_not_help() {
        let args = SprioArgs::try_parse_from(["sprio", "-h"]).unwrap();
        assert!(args.noheader);
    }

    #[test]
    fn long_help_flag_is_preserved() {
        let err = SprioArgs::try_parse_from(["sprio", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn structured_output_flags_are_accepted() {
        let args = SprioArgs::try_parse_from(["sprio", "--json"]).unwrap();
        assert_eq!(
            args.output.format().unwrap(),
            crate::output::OutputFormat::Structured(crate::output::Encoding::Json)
        );

        let args = SprioArgs::try_parse_from(["sprio", "--yaml"]).unwrap();
        assert_eq!(
            args.output.format().unwrap(),
            crate::output::OutputFormat::Structured(crate::output::Encoding::Yaml)
        );
    }

    #[test]
    fn json_document_carries_factors_under_a_jobs_key() {
        let factors = priority_factors(&[pending(7, "gpu", 100, 0)], &[], 0);
        let doc = crate::output::render(
            crate::output::Encoding::Json,
            &["sprio".to_string()],
            PriorityFactorsPayload { jobs: factors },
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(parsed["jobs"][0]["job_id"], 7);
        assert_eq!(parsed["jobs"][0]["priority"], 100);
        assert_eq!(parsed["jobs"][0]["effective_priority"], 100);
        assert_eq!(parsed["jobs"][0]["partition_tier"], 1);
    }

    #[test]
    fn noheader_suppresses_only_the_header() {
        let args = SprioArgs::try_parse_from(["sprio", "-h"]).unwrap();
        let factors = priority_factors(&[pending(7, "gpu", 100, 0)], &[], 0);
        let lines = render_text(&args, &factors);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains('7'));
    }
}
