pub mod errors;
pub mod requests;
pub use crate::errors::*;

use candid::{CandidType, Decode};
use core::hash::Hash;
use ic_nns_governance::pb::v1::NnsFunction;
use ic_nns_governance::pb::v1::ProposalInfo;
use ic_nns_governance::pb::v1::ProposalStatus;
use ic_registry_subnet_type::SubnetType;
use ic_types::PrincipalId;
use registry_canister::mutations::do_add_nodes_to_subnet::AddNodesToSubnetPayload;
use registry_canister::mutations::do_change_subnet_membership::ChangeSubnetMembershipPayload;
use registry_canister::mutations::do_create_subnet::CreateSubnetPayload;
use registry_canister::mutations::do_remove_nodes_from_subnet::RemoveNodesFromSubnetPayload;
use registry_canister::mutations::do_update_subnet_replica::UpdateSubnetReplicaVersionPayload;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::cmp::{Eq, Ord, PartialEq, PartialOrd};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::net::Ipv6Addr;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use strum::VariantNames;
use strum_macros::{Display, EnumString, EnumVariantNames};
use url::Url;

pub trait NnsFunctionProposal: CandidType + serde::de::DeserializeOwned {
    const TYPE: NnsFunction;
    fn decode(function_type: NnsFunction, function_payload: &[u8]) -> anyhow::Result<Self> {
        if function_type == Self::TYPE {
            Decode!(function_payload, Self).map_err(|e| anyhow::format_err!("failed decoding candid: {}", e))
        } else {
            Err(anyhow::format_err!("unsupported NNS function"))
        }
    }
}

impl NnsFunctionProposal for AddNodesToSubnetPayload {
    const TYPE: NnsFunction = NnsFunction::AddNodeToSubnet;
}

impl NnsFunctionProposal for RemoveNodesFromSubnetPayload {
    const TYPE: NnsFunction = NnsFunction::RemoveNodesFromSubnet;
}

impl NnsFunctionProposal for CreateSubnetPayload {
    const TYPE: NnsFunction = NnsFunction::CreateSubnet;
}

impl NnsFunctionProposal for UpdateSubnetReplicaVersionPayload {
    const TYPE: NnsFunction = NnsFunction::UpdateSubnetReplicaVersion;
}

impl NnsFunctionProposal for ChangeSubnetMembershipPayload {
    const TYPE: NnsFunction = NnsFunction::ChangeSubnetMembership;
}

pub trait SubnetMembershipChangePayload: NnsFunctionProposal {
    fn get_nodes(&self) -> Vec<PrincipalId>;
    fn get_subnet(&self) -> Option<PrincipalId>;
}

impl SubnetMembershipChangePayload for CreateSubnetPayload {
    fn get_nodes(&self) -> Vec<PrincipalId> {
        self.node_ids.iter().map(|node_id| node_id.get()).collect()
    }

    fn get_subnet(&self) -> Option<PrincipalId> {
        None
    }
}

impl SubnetMembershipChangePayload for AddNodesToSubnetPayload {
    fn get_nodes(&self) -> Vec<PrincipalId> {
        self.node_ids.iter().map(|node_id| node_id.get()).collect()
    }

    fn get_subnet(&self) -> Option<PrincipalId> {
        Some(self.subnet_id)
    }
}

impl SubnetMembershipChangePayload for RemoveNodesFromSubnetPayload {
    fn get_nodes(&self) -> Vec<PrincipalId> {
        self.node_ids.iter().map(|node_id| node_id.get()).collect()
    }

    fn get_subnet(&self) -> Option<PrincipalId> {
        None
    }
}

impl SubnetMembershipChangePayload for ChangeSubnetMembershipPayload {
    fn get_nodes(&self) -> Vec<PrincipalId> {
        self.node_ids_add
            .iter()
            .map(|node_id| node_id.get())
            .chain(self.node_ids_remove.iter().map(|node_id| node_id.get()))
            .collect()
    }

    fn get_subnet(&self) -> Option<PrincipalId> {
        Some(self.subnet_id)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubnetMembershipChangeProposal {
    pub nodes: Vec<PrincipalId>,
    pub subnet_id: Option<PrincipalId>,
    pub id: u64,
}

impl<T: SubnetMembershipChangePayload> From<(ProposalInfo, T)> for SubnetMembershipChangeProposal {
    fn from((info, payload): (ProposalInfo, T)) -> Self {
        Self {
            subnet_id: payload.get_subnet(),
            nodes: payload.get_nodes(),
            id: info.id.unwrap().id,
        }
    }
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct Subnet {
    #[serde_as(as = "DisplayFromStr")]
    pub principal: PrincipalId,
    pub nodes: Vec<Node>,
    pub subnet_type: SubnetType,
    pub metadata: SubnetMetadata,
    pub replica_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<SubnetMembershipChangeProposal>,
    pub replica_release: Option<ReplicaRelease>,
}

type Application = String;
type Label = String;

#[derive(Clone, Serialize, Default, Deserialize)]
pub struct SubnetMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Label>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applications: Option<Vec<Application>>,
}

#[serde_as]
#[derive(Clone, Serialize, Debug, Deserialize)]
pub struct Node {
    #[serde_as(as = "DisplayFromStr")]
    pub principal: PrincipalId,
    pub ip_addr: Ipv6Addr,
    pub operator: Operator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet: Option<PrincipalId>,
    pub dfinity_owned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<SubnetMembershipChangeProposal>,
    pub label: Option<String>,
    #[serde(default)]
    pub decentralized: bool,
}

#[derive(
    Display, EnumString, EnumVariantNames, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Serialize, Deserialize, Debug,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum NodeFeature {
    NodeProvider,
    DataCenter,
    DataCenterOwner,
    City,
    Country,
    Continent,
}

impl NodeFeature {
    pub fn variants() -> Vec<Self> {
        NodeFeature::VARIANTS
            .iter()
            .map(|f| NodeFeature::from_str(f).unwrap())
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MinNakamotoCoefficients {
    pub coefficients: BTreeMap<NodeFeature, f64>,
    pub average: f64,
}

#[derive(Clone, Serialize, Debug, Deserialize)]
pub struct TopologyProposal {
    pub id: u64,
    pub kind: TopologyProposalKind,
    pub status: TopologyProposalStatus,
}

#[derive(EnumString, Clone, Deserialize, Debug, Serialize)]
pub enum TopologyProposalStatus {
    Open,
    Executed,
}

impl TryFrom<ProposalStatus> for TopologyProposalStatus {
    type Error = String;

    fn try_from(value: ProposalStatus) -> Result<Self, Self::Error> {
        match value {
            ProposalStatus::Open => Ok(Self::Open),
            ProposalStatus::Executed => Ok(Self::Executed),
            _ => Err("cannot convert to topology proposal".to_string()),
        }
    }
}

#[derive(Clone, Serialize, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyProposalKind {
    ReplaceNode(ReplaceNodeProposalInfo),
    CreateSubnet(CreateSubnetProposalInfo),
}

#[derive(Clone, Serialize, Debug, Deserialize)]
pub struct ReplaceNodeProposalInfo {
    pub old_nodes: Vec<PrincipalId>,
    pub new_nodes: Vec<PrincipalId>,
    pub first: bool,
}

#[derive(Clone, Serialize, Debug, Deserialize)]
pub struct CreateSubnetProposalInfo {
    pub nodes: Vec<PrincipalId>,
}

#[serde_as]
#[derive(Clone, Serialize, Default, Debug, Deserialize)]
pub struct Operator {
    #[serde_as(as = "DisplayFromStr")]
    pub principal: PrincipalId,
    pub provider: Provider,
    pub allowance: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacenter: Option<Datacenter>,
}

#[serde_as]
#[derive(Clone, Serialize, Default, Debug, Deserialize)]
pub struct Provider {
    #[serde_as(as = "DisplayFromStr")]
    pub principal: PrincipalId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub website: Option<String>,
}

#[derive(Clone, Serialize, Default, Debug, Deserialize)]
pub struct Datacenter {
    pub name: String,
    pub owner: DatacenterOwner,
    pub city: String,
    pub country: String,
    pub continent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
}

#[derive(Clone, Serialize, Default, Debug, Deserialize)]
pub struct DatacenterOwner {
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub struct Guest {
    pub datacenter: String,
    pub ipv6: Ipv6Addr,
    pub name: String,
    pub dfinity_owned: bool,
    pub decentralized: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub struct FactsDBGuest {
    pub name: String,
    pub node_type: String,
    pub ipv6: Ipv6Addr,
    pub principal: String,
    pub subnet: String,
    pub physical_system: String,
}

impl From<FactsDBGuest> for Guest {
    fn from(g: FactsDBGuest) -> Self {
        Guest {
            datacenter: g
                .physical_system
                .split('.')
                .nth(1)
                .expect("invalid physical system name")
                .to_string(),
            ipv6: g.ipv6,
            name: g
                .physical_system
                .split('.')
                .next()
                .expect("invalid physical system name")
                .to_string(),
            dfinity_owned: g.node_type.contains("dfinity"),
            decentralized: g.ipv6.segments()[4] == 0x6801,
        }
    }
}

// https://ic-api.internetcomputer.org/api/v2/locations
#[derive(Clone, Serialize, Deserialize)]
pub struct Location {
    pub key: String,
    pub latitude: f64,
    pub longitude: f64,
    pub name: String,
    pub node_operator: String,
}

// https://ic-api.internetcomputer.org/api/v3/node-providers
#[derive(Clone, Serialize, Deserialize)]
pub struct NodeProvidersResponse {
    pub node_providers: Vec<NodeProviderDetails>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeProviderDetails {
    pub display_name: String,
    pub principal_id: PrincipalId,
    pub website: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeReplacements {
    pub removed: Vec<PrincipalId>,
    pub added: Vec<PrincipalId>,
}

#[derive(PartialOrd, Ord, Eq, PartialEq, Clone, Serialize, Deserialize, Debug, EnumString)]
pub enum Health {
    Offline,
    Degraded,
    Healthy,
    Unknown,
}

impl From<i64> for Health {
    fn from(value: i64) -> Self {
        match value {
            1 => Self::Healthy,
            0 => Self::Offline,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatusSource {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    pub health: Health,
    pub ip_addr: Ipv6Addr,
    pub sources: Vec<NodeStatusSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVersion {
    pub principal: PrincipalId,
    pub replica_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ReplicaRelease {
    pub commit_hash: String,
    pub branch: String,
    pub name: String,
    pub time: chrono::NaiveDateTime,
    pub previous_patch_release: Option<Arc<ReplicaRelease>>,
}

impl ReplicaRelease {
    pub fn patch_count(&self) -> u32 {
        match &self.previous_patch_release {
            Some(rv) => rv.patch_count(),
            None => 0,
        }
    }

    pub fn patches(&self, replica_release: &ReplicaRelease) -> bool {
        match &self.previous_patch_release {
            Some(rv) => rv.deref().eq(replica_release) || rv.patches(replica_release),
            None => false,
        }
    }

    pub fn contains_patch(&self, commit_hash: &str) -> bool {
        self.commit_hash == commit_hash
            || self
                .previous_patch_release
                .as_ref()
                .map(|r| r.contains_patch(commit_hash))
                .unwrap_or_default()
    }

    pub fn patches_for(&self, commit_hash: &str) -> Result<Vec<ReplicaRelease>, String> {
        if self.commit_hash == *commit_hash {
            Ok(vec![])
        } else if let Some(previous) = &self.previous_patch_release {
            previous.patches_for(commit_hash).map(|mut patches| {
                patches.push(self.clone());
                patches
            })
        } else {
            Err("doesn't patch this release".to_string())
        }
    }

    pub fn get(&self, commit_hash: &str) -> Result<ReplicaRelease, String> {
        if self.commit_hash == *commit_hash {
            Ok(self.clone())
        } else if let Some(previous) = &self.previous_patch_release {
            previous.get(commit_hash)
        } else {
            Err("doesn't patch this release".to_string())
        }
    }
}

#[derive(Clone)]
pub enum Network {
    Staging,
    Mainnet,
    Url(url::Url),
}

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "mainnet" => Self::Mainnet,
            "staging" => Self::Staging,
            _ => Self::Url(url::Url::from_str(s).map_err(|e| format!("{}", e))?),
        })
    }
}

impl Network {
    pub fn get_url(&self) -> Url {
        match self {
            Network::Mainnet => Url::from_str("https://ic0.app").unwrap(),
            Network::Staging => Url::from_str("https://staging.release.dfinity.network").unwrap(),
            Self::Url(url) => url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlessedVersions {
    pub all: Vec<String>,
    pub obsolete: Vec<String>,
}
