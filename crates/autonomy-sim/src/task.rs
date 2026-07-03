use autonomy_core::{DecisionId, ObjectiveId, Quantity, ResourceNodeId, StorageId, TaskId};

use crate::entity::ResourceKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub id: DecisionId,
    pub objective_id: ObjectiveId,
    pub kind: DecisionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionKind {
    CreateTask { task_id: TaskId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub objective_id: ObjectiveId,
    pub decision_id: Option<DecisionId>,
    pub kind: TaskKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskKind {
    MineResource {
        resource: ResourceKind,
        quantity: Quantity,
        node_id: ResourceNodeId,
    },
    DepositResource {
        storage_id: StorageId,
    },
}
