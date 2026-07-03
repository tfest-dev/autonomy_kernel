use std::error::Error;
use std::fmt;

use crate::{Position, Quantity, ResourceNodeId, StorageId, WorkerId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimError {
    UnknownWorker(WorkerId),
    UnknownResourceNode(ResourceNodeId),
    UnknownStorage(StorageId),
    InvalidAction(&'static str),
    InsufficientBattery {
        worker_id: WorkerId,
        required: Quantity,
        available: Quantity,
    },
    InsufficientResource {
        resource_node_id: ResourceNodeId,
        requested: Quantity,
        available: Quantity,
    },
    NotAdjacent {
        from: Position,
        to: Position,
    },
    CapacityExceeded {
        current: Quantity,
        added: Quantity,
    },
    TickOverflow,
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWorker(id) => write!(f, "unknown worker: {}", id.value()),
            Self::UnknownResourceNode(id) => write!(f, "unknown resource node: {}", id.value()),
            Self::UnknownStorage(id) => write!(f, "unknown storage: {}", id.value()),
            Self::InvalidAction(reason) => write!(f, "invalid action: {reason}"),
            Self::InsufficientBattery {
                worker_id,
                required,
                available,
            } => write!(
                f,
                "worker {} has insufficient battery: required {}, available {}",
                worker_id.value(),
                required.value(),
                available.value()
            ),
            Self::InsufficientResource {
                resource_node_id,
                requested,
                available,
            } => write!(
                f,
                "resource node {} has insufficient resource: requested {}, available {}",
                resource_node_id.value(),
                requested.value(),
                available.value(),
            ),
            Self::NotAdjacent { from, to } => write!(
                f,
                "positions are not adjacent: ({}, {}) -> ({}, {})",
                from.x, from.y, to.x, to.y
            ),
            Self::CapacityExceeded { current, added } => write!(
                f,
                "capacity exceeded: current {}, attempted addition {}",
                current.value(),
                added.value()
            ),
            Self::TickOverflow => write!(f, "tick overflow"),
        }
    }
}

impl Error for SimError {}
