// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Structural validation of every command parser.
//!
//! clap checks argument definitions (duplicate short options, conflicting IDs,
//! invalid defaults) behind `debug_assertions`, and only when the offending
//! command is actually built. A release binary therefore ships a malformed
//! parser silently while every debug build panics on the first invocation.
//! Asserting each parser here turns that into a `cargo test` failure.

use clap::CommandFactory;

/// Every parser reachable from the symlink and subcommand dispatch in `main`.
/// Add new commands here when they are wired into that dispatch.
#[test]
fn command_parsers_are_well_formed() {
    crate::exec::ExecArgs::command().debug_assert();
    crate::image::ImageArgs::command().debug_assert();
    crate::k8s::K8sArgs::command().debug_assert();
    crate::net::NetArgs::command().debug_assert();
    crate::node::NodeArgs::command().debug_assert();
    crate::sacct::SacctArgs::command().debug_assert();
    crate::sacctmgr::SacctmgrArgs::command().debug_assert();
    crate::salloc::SallocArgs::command().debug_assert();
    crate::sattach::SattachArgs::command().debug_assert();
    crate::sbatch::SbatchArgs::command().debug_assert();
    crate::scancel::ScancelArgs::command().debug_assert();
    crate::scontrol::ScontrolArgs::command().debug_assert();
    crate::scrontab::ScrontabArgs::command().debug_assert();
    crate::sdiag::SdiagArgs::command().debug_assert();
    crate::sinfo::SinfoArgs::command().debug_assert();
    crate::smd::SmdArgs::command().debug_assert();
    crate::sprio::SprioArgs::command().debug_assert();
    crate::squeue::SqueueArgs::command().debug_assert();
    crate::sreport::SreportArgs::command().debug_assert();
    crate::srun::SrunArgs::command().debug_assert();
    crate::sshare::SshareArgs::command().debug_assert();
    crate::sstat::SstatArgs::command().debug_assert();
    crate::strigger::StriggerArgs::command().debug_assert();
    crate::token::TokenArgs::command().debug_assert();
}
