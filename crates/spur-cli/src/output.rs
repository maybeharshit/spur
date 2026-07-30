// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Structured output (`--json` / `--yaml`) shared by every query command.
//!
//! Commands flatten [`OutputArgs`] into their own argument struct, then branch
//! once on [`OutputArgs::format`] before rendering. The documents themselves are
//! built from `spur_api` views, so CLI output and the REST API stay in step.

use anyhow::{bail, Result};
use clap::Args;
use serde::Serialize;
use spur_api::Envelope;

/// The only data parser Spur publishes. Slurm names its schema versions this
/// way and accepts them as an argument to `--json`/`--yaml`; scripts written
/// against Slurm pass the version explicitly, so the form is accepted rather
/// than rejected as an unknown flag.
pub const DATA_PARSER: &str = "v0.0.42";

/// How a command should render its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Slurm-style fixed-width text.
    Text,
    Structured(Encoding),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Json,
    Yaml,
}

/// The `--json` / `--yaml` flag pair.
///
/// Text-formatting flags (`-o`, `-l`, `-h`) are deliberately not marked as
/// conflicting: Slurm ignores them in structured mode, and wrapper scripts
/// commonly set them unconditionally.
#[derive(Args, Debug, Default, Clone)]
pub struct OutputArgs {
    /// Emit JSON instead of formatted text
    #[arg(
        long,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = DATA_PARSER,
        value_name = "DATA_PARSER",
        conflicts_with = "yaml",
    )]
    pub json: Option<String>,

    /// Emit YAML instead of formatted text
    #[arg(
        long,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = DATA_PARSER,
        value_name = "DATA_PARSER",
    )]
    pub yaml: Option<String>,
}

impl OutputArgs {
    pub fn format(&self) -> Result<OutputFormat> {
        if let Some(parser) = self.json.as_deref() {
            check_data_parser(parser)?;
            return Ok(OutputFormat::Structured(Encoding::Json));
        }
        if let Some(parser) = self.yaml.as_deref() {
            check_data_parser(parser)?;
            return Ok(OutputFormat::Structured(Encoding::Yaml));
        }
        Ok(OutputFormat::Text)
    }
}

fn check_data_parser(requested: &str) -> Result<()> {
    if requested == DATA_PARSER {
        return Ok(());
    }
    bail!("unsupported data parser '{requested}'; spur emits '{DATA_PARSER}'")
}

/// Serialize `payload` inside the shared envelope and write it to stdout.
///
/// `argv` is recorded as `meta.command`, mirroring Slurm's CLI output.
pub fn emit<T: Serialize>(encoding: Encoding, argv: &[String], payload: T) -> Result<()> {
    print!("{}", render(encoding, argv, payload)?);
    Ok(())
}

/// The document `emit` would print. Separated so tests can assert on the output
/// without capturing stdout.
pub fn render<T: Serialize>(encoding: Encoding, argv: &[String], payload: T) -> Result<String> {
    let envelope = Envelope::new(payload).with_command(argv.to_vec());
    let text = match encoding {
        Encoding::Json => format!("{}\n", serde_json::to_string_pretty(&envelope)?),
        Encoding::Yaml => serde_yaml::to_string(&envelope)?,
    };
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use spur_api::JobsPayload;

    #[derive(Parser, Debug)]
    struct Harness {
        #[command(flatten)]
        output: OutputArgs,
    }

    fn parse(args: &[&str]) -> Harness {
        Harness::try_parse_from(args).unwrap()
    }

    #[test]
    fn text_is_the_default() {
        let h = parse(&["test"]);
        assert_eq!(h.output.format().unwrap(), OutputFormat::Text);
    }

    #[test]
    fn bare_flags_select_an_encoding() {
        assert_eq!(
            parse(&["test", "--json"]).output.format().unwrap(),
            OutputFormat::Structured(Encoding::Json)
        );
        assert_eq!(
            parse(&["test", "--yaml"]).output.format().unwrap(),
            OutputFormat::Structured(Encoding::Yaml)
        );
    }

    #[test]
    fn slurms_explicit_parser_argument_is_accepted() {
        let h = parse(&["test", "--json=v0.0.42"]);
        assert_eq!(
            h.output.format().unwrap(),
            OutputFormat::Structured(Encoding::Json)
        );
    }

    #[test]
    fn an_unsupported_parser_is_reported_rather_than_ignored() {
        let h = parse(&["test", "--json=v0.0.40"]);
        let err = h.output.format().unwrap_err();
        assert!(err.to_string().contains("v0.0.40"));
        assert!(err.to_string().contains(DATA_PARSER));
    }

    #[test]
    fn json_and_yaml_are_mutually_exclusive() {
        let err = Harness::try_parse_from(["test", "--json", "--yaml"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn json_output_is_pretty_printed_and_newline_terminated() {
        let doc = render(
            Encoding::Json,
            &["squeue".to_string()],
            JobsPayload { jobs: Vec::new() },
        )
        .unwrap();
        assert!(doc.ends_with('\n'));
        assert!(doc.contains("\n  \"meta\""), "expected indented output");
        let parsed: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(parsed["jobs"], serde_json::json!([]));
    }

    #[test]
    fn yaml_output_parses_as_yaml() {
        let doc = render(
            Encoding::Yaml,
            &["squeue".to_string()],
            JobsPayload { jobs: Vec::new() },
        )
        .unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&doc).unwrap();
        assert!(parsed.get("meta").is_some());
        assert!(parsed.get("jobs").is_some());
    }

    #[test]
    fn argv_is_recorded_as_meta_command() {
        let doc = render(
            Encoding::Json,
            &["squeue".to_string(), "--json".to_string()],
            JobsPayload { jobs: Vec::new() },
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(
            parsed["meta"]["command"],
            serde_json::json!(["squeue", "--json"])
        );
    }
}
