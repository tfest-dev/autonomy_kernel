use std::collections::BTreeMap;

use autonomy_core::{Position, Quantity, ResourceNodeId, StorageId, WorkerId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceKind {
    Iron,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerRole {
    Miner,
    Hauler,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerStatus {
    Active,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarriedResource {
    pub kind: ResourceKind,
    pub quantity: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worker {
    pub id: WorkerId,
    pub role: WorkerRole,
    pub position: Position,
    pub battery: Quantity,
    pub carried: Option<CarriedResource>,
    pub status: WorkerStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceNode {
    pub id: ResourceNodeId,
    pub kind: ResourceKind,
    pub position: Position,
    pub remaining: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Storage {
    pub id: StorageId,
    pub position: Position,
    pub inventory: BTreeMap<ResourceKind, Quantity>,
}
