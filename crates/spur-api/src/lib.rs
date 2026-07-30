// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Spur's machine-readable API schema.
//!
//! The REST API and the CLI's `--json`/`--yaml` modes both render through the
//! views defined here, so the two surfaces cannot describe the same entity
//! differently. Views are defined over `spur_proto` messages rather than the
//! internal `spur_core` domain types for two reasons: proto is the published
//! API surface that REST and FFI already depend on, and it is what the CLI
//! receives over the wire. The controller converts its domain types to proto on
//! the way out either way.
//!
//! This crate deliberately depends on nothing but the proto definitions, the
//! domain enums it needs for state names, and serde. Keeping the dependency
//! list closed is what stops the published schema from drifting into
//! transport- or storage-specific concerns.

mod envelope;
mod util;

pub mod diag;
pub mod job;
pub mod node;
pub mod payload;

pub use envelope::{ApiError, Envelope, Meta, SlurmMeta, VersionInfo};
pub use job::{JobStatisticsView, JobView, PriorityFactorsView, StepView};
pub use node::{GpuView, NodeView, PartitionView, ReservationView};
pub use payload::{
    ClusterConfigView, ClusterPayload, ConfigPayload, DiagnosticsPayload, FederationPayload,
    FederationPeerView, JobStatisticsPayload, JobsPayload, NodesPayload, PartitionsPayload,
    PriorityFactorsPayload, ReservationsPayload, StepsPayload,
};
