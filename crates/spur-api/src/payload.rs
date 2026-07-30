// Copyright (c) 2026 Advanced Micro Devices, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Top-level payloads. Each one names the key its entities appear under once
//! flattened into an [`Envelope`](crate::Envelope).

use serde::Serialize;

use crate::diag::DiagnosticsView;
use crate::job::{JobStatisticsView, JobView, PriorityFactorsView, StepView};
use crate::node::{NodeView, PartitionView, ReservationView};

#[derive(Debug, Serialize)]
pub struct JobsPayload {
    pub jobs: Vec<JobView>,
}

impl JobsPayload {
    pub fn from_proto(jobs: &[spur_proto::proto::JobInfo]) -> Self {
        Self {
            jobs: jobs.iter().map(JobView::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NodesPayload {
    pub nodes: Vec<NodeView>,
}

impl NodesPayload {
    pub fn from_proto(nodes: &[spur_proto::proto::NodeInfo]) -> Self {
        Self {
            nodes: nodes.iter().map(NodeView::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PartitionsPayload {
    pub partitions: Vec<PartitionView>,
}

impl PartitionsPayload {
    pub fn from_proto(partitions: &[spur_proto::proto::PartitionInfo]) -> Self {
        Self {
            partitions: partitions.iter().map(PartitionView::from).collect(),
        }
    }
}

/// `sinfo` reports on both nodes and the partitions they belong to, so its
/// document carries the two arrays side by side.
#[derive(Debug, Serialize)]
pub struct ClusterPayload {
    pub nodes: Vec<NodeView>,
    pub partitions: Vec<PartitionView>,
}

impl ClusterPayload {
    pub fn from_proto(
        nodes: &[spur_proto::proto::NodeInfo],
        partitions: &[spur_proto::proto::PartitionInfo],
    ) -> Self {
        Self {
            nodes: nodes.iter().map(NodeView::from).collect(),
            partitions: partitions.iter().map(PartitionView::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReservationsPayload {
    pub reservations: Vec<ReservationView>,
}

impl ReservationsPayload {
    pub fn from_proto(reservations: &[spur_proto::proto::ReservationInfo]) -> Self {
        Self {
            reservations: reservations.iter().map(ReservationView::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StepsPayload {
    pub steps: Vec<StepView>,
}

impl StepsPayload {
    pub fn from_proto(steps: &[spur_proto::proto::JobStepInfo]) -> Self {
        Self {
            steps: steps.iter().map(StepView::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PriorityFactorsPayload {
    pub jobs: Vec<PriorityFactorsView>,
}

#[derive(Debug, Serialize)]
pub struct JobStatisticsPayload {
    pub statistics: Vec<JobStatisticsView>,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticsPayload {
    pub statistics: DiagnosticsView,
}

#[derive(Debug, Serialize)]
pub struct ConfigPayload {
    pub config: ClusterConfigView,
}

#[derive(Debug, Serialize)]
pub struct ClusterConfigView {
    pub cluster_name: String,
    pub controller_address: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct FederationPayload {
    pub federation: Vec<FederationPeerView>,
}

#[derive(Debug, Serialize)]
pub struct FederationPeerView {
    pub cluster: String,
    pub address: String,
}

impl FederationPeerView {
    /// Peers arrive as `name@address`; entries without a separator are reported
    /// verbatim with an unknown address rather than dropped.
    pub fn parse(peer: &str) -> Self {
        match peer.split_once('@') {
            Some((cluster, address)) => Self {
                cluster: cluster.to_string(),
                address: address.to_string(),
            },
            None => Self {
                cluster: peer.to_string(),
                address: String::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn federation_peers_split_on_the_address_separator() {
        let peer = FederationPeerView::parse("west@10.0.0.1:6817");
        assert_eq!(peer.cluster, "west");
        assert_eq!(peer.address, "10.0.0.1:6817");
    }

    #[test]
    fn a_peer_without_an_address_keeps_its_name() {
        let peer = FederationPeerView::parse("west");
        assert_eq!(peer.cluster, "west");
        assert!(peer.address.is_empty());
    }
}
