use std::collections::BTreeMap;

use autonomy_core::{ResourceNodeId, StorageId, Tick, WorkerId};

use crate::entity::{ResourceNode, Storage, Worker};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldState {
    pub tick: Tick,
    pub workers: BTreeMap<WorkerId, Worker>,
    pub resource_nodes: BTreeMap<ResourceNodeId, ResourceNode>,
    pub storage: BTreeMap<StorageId, Storage>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            tick: Tick::ZERO,
            workers: BTreeMap::new(),
            resource_nodes: BTreeMap::new(),
            storage: BTreeMap::new(),
        }
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}
