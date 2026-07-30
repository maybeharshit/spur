// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Node and partition views.

use serde::Serialize;
use spur_proto::proto::{GpuResource, NodeInfo, PartitionInfo, ReservationInfo};

use crate::util::{csv_list, minutes, ts_secs};

/// A node as published by `GET /nodes`, `sinfo`, and `scontrol show node`.
#[derive(Debug, Serialize)]
pub struct NodeView {
    pub name: String,
    pub state: String,
    pub reason: String,
    pub partitions: Vec<String>,
    pub cpus: u32,
    pub alloc_cpus: u32,
    pub real_memory: u64,
    pub alloc_memory: u64,
    pub free_mem: u64,
    /// Load average scaled by 100, matching Slurm's `CpuLoad`.
    pub cpu_load: u32,
    pub architecture: String,
    pub operating_system: String,
    pub features: Vec<String>,
    pub gpus: Vec<GpuView>,
    pub boot_time: Option<i64>,
    pub last_busy: Option<i64>,
    pub reservation: String,
}

impl From<&NodeInfo> for NodeView {
    fn from(node: &NodeInfo) -> Self {
        let total = node.total_resources.as_ref();
        let alloc = node.alloc_resources.as_ref();
        Self {
            name: node.name.clone(),
            state: node_state_name(node.state),
            reason: node.state_reason.clone(),
            partitions: node.partitions.clone(),
            cpus: total.map(|r| r.cpus).unwrap_or(0),
            alloc_cpus: alloc.map(|r| r.cpus).unwrap_or(0),
            real_memory: total.map(|r| r.memory_mb).unwrap_or(0),
            alloc_memory: alloc.map(|r| r.memory_mb).unwrap_or(0),
            free_mem: node.free_memory_mb,
            cpu_load: node.cpu_load,
            architecture: node.arch.clone(),
            operating_system: node.os.clone(),
            features: node.features.clone(),
            gpus: total
                .map(|r| r.gpus.iter().map(GpuView::from).collect())
                .unwrap_or_default(),
            boot_time: ts_secs(node.boot_time.as_ref()),
            last_busy: ts_secs(node.last_busy.as_ref()),
            reservation: node.active_reservation.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GpuView {
    pub device_id: u32,
    pub gpu_type: String,
    pub memory_mb: u64,
}

impl From<&GpuResource> for GpuView {
    fn from(gpu: &GpuResource) -> Self {
        Self {
            device_id: gpu.device_id,
            gpu_type: gpu.gpu_type.clone(),
            memory_mb: gpu.memory_mb,
        }
    }
}

/// A partition as published by `GET /partitions`, `sinfo`, and
/// `scontrol show partition`.
#[derive(Debug, Serialize)]
pub struct PartitionView {
    pub name: String,
    pub state: String,
    pub is_default: bool,
    pub total_nodes: u32,
    pub total_cpus: u32,
    pub nodes: String,
    /// Limits in minutes. `null` means unlimited.
    pub max_time: Option<i64>,
    pub default_time: Option<i64>,
    pub max_nodes: u32,
    pub min_nodes: u32,
    pub priority_tier: u32,
    pub preempt_mode: String,
    pub allow_root: bool,
    pub exclusive_user: bool,
    pub allow_accounts: Vec<String>,
    pub deny_accounts: Vec<String>,
    pub allow_groups: Vec<String>,
    pub allow_qos: Vec<String>,
}

impl From<&PartitionInfo> for PartitionView {
    fn from(part: &PartitionInfo) -> Self {
        Self {
            name: part.name.clone(),
            state: part.state.clone(),
            is_default: part.is_default,
            total_nodes: part.total_nodes,
            total_cpus: part.total_cpus,
            nodes: part.nodes.clone(),
            max_time: minutes(part.max_time.as_ref()),
            default_time: minutes(part.default_time.as_ref()),
            max_nodes: part.max_nodes,
            min_nodes: part.min_nodes,
            priority_tier: part.priority_tier,
            preempt_mode: part.preempt_mode.clone(),
            allow_root: part.allow_root,
            exclusive_user: part.exclusive_user,
            allow_accounts: csv_list(&part.allow_accounts),
            deny_accounts: csv_list(&part.deny_accounts),
            allow_groups: csv_list(&part.allow_groups),
            allow_qos: csv_list(&part.allow_qos),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReservationView {
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub nodes: String,
    pub state: String,
    pub flags: Vec<String>,
    pub accounts: Vec<String>,
    pub users: Vec<String>,
    pub owner: String,
}

impl From<&ReservationInfo> for ReservationView {
    fn from(res: &ReservationInfo) -> Self {
        Self {
            name: res.name.clone(),
            start_time: res.start_time.clone(),
            end_time: res.end_time.clone(),
            nodes: res.nodes.clone(),
            state: res.state.clone(),
            flags: csv_list(&res.flags),
            accounts: csv_list(&res.accounts),
            users: csv_list(&res.users),
            owner: res.owner.clone(),
        }
    }
}

pub fn node_state_name(state: i32) -> String {
    spur_core::node::NodeState::from_proto_i32(state)
        .map(|s| s.display().to_string())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spur_proto::proto::{NodeState, ResourceAllocations, ResourceSet};

    #[test]
    fn node_view_splits_total_and_allocated_resources() {
        let node = NodeInfo {
            name: "node01".into(),
            state: NodeState::NodeMixed as i32,
            partitions: vec!["gpu".into()],
            total_resources: Some(ResourceSet {
                cpus: 128,
                memory_mb: 512_000,
                gpus: vec![GpuResource {
                    device_id: 0,
                    gpu_type: "mi300x".into(),
                    memory_mb: 196_608,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            alloc_resources: Some(ResourceAllocations {
                cpus: 32,
                memory_mb: 64_000,
                ..Default::default()
            }),
            free_memory_mb: 448_000,
            ..Default::default()
        };

        let view = NodeView::from(&node);
        assert_eq!(view.state, "mixed");
        assert_eq!(view.cpus, 128);
        assert_eq!(view.alloc_cpus, 32);
        assert_eq!(view.real_memory, 512_000);
        assert_eq!(view.alloc_memory, 64_000);
        assert_eq!(view.gpus.len(), 1);
        assert_eq!(view.gpus[0].gpu_type, "mi300x");
    }

    #[test]
    fn node_without_resources_reports_zeroes_not_an_error() {
        let view = NodeView::from(&NodeInfo::default());
        assert_eq!(view.cpus, 0);
        assert_eq!(view.alloc_memory, 0);
        assert!(view.gpus.is_empty());
    }

    #[test]
    fn partition_access_lists_are_arrays() {
        let part = PartitionInfo {
            name: "gpu".into(),
            allow_accounts: "research,ml".into(),
            deny_accounts: "student".into(),
            ..Default::default()
        };
        let view = PartitionView::from(&part);
        assert_eq!(view.allow_accounts, vec!["research", "ml"]);
        assert_eq!(view.deny_accounts, vec!["student"]);
        assert!(view.allow_qos.is_empty());
    }

    #[test]
    fn partition_limits_are_reported_in_minutes() {
        let part = PartitionInfo {
            max_time: Some(prost_types::Duration {
                seconds: 7200,
                nanos: 0,
            }),
            ..Default::default()
        };
        let view = PartitionView::from(&part);
        assert_eq!(view.max_time, Some(120));
        assert_eq!(view.default_time, None);
    }

    #[test]
    fn reservation_lists_are_arrays() {
        let res = ReservationInfo {
            name: "maint".into(),
            users: "alice,bob".into(),
            flags: "MAINT".into(),
            ..Default::default()
        };
        let view = ReservationView::from(&res);
        assert_eq!(view.users, vec!["alice", "bob"]);
        assert_eq!(view.flags, vec!["MAINT"]);
        assert!(view.accounts.is_empty());
    }
}
