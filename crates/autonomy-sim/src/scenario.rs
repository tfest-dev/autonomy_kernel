use std::collections::BTreeMap;

use autonomy_core::{
    AssignmentId, DecisionId, ObjectiveId, Position, Quantity, ResourceNodeId, StorageId, TaskId,
    Tick, WorkerId,
};

use crate::{
    Assignment, Decision, DecisionKind, Objective, ObjectiveKind, ResourceKind, ResourceNode,
    Storage, Task, TaskKind, Worker, WorkerAction, WorkerRole, WorkerStatus, WorldState,
};

pub const MINING_BOOTSTRAP_OBJECTIVE_ID: ObjectiveId = ObjectiveId(1);
pub const MINING_BOOTSTRAP_DECISION_ID: DecisionId = DecisionId(1);
pub const MINING_BOOTSTRAP_TASK_ID: TaskId = TaskId(1);
pub const MINING_BOOTSTRAP_ASSIGNMENT_ID: AssignmentId = AssignmentId(1);
pub const MINING_BOOTSTRAP_WORKER_ID: WorkerId = WorkerId(1);
pub const MINING_BOOTSTRAP_NODE_ID: ResourceNodeId = ResourceNodeId(1);
pub const MINING_BOOTSTRAP_STORAGE_ID: StorageId = StorageId(1);
pub const MINING_BOOTSTRAP_STOCKPILE_MINIMUM: Quantity = Quantity(10);
pub const MINING_BOOTSTRAP_INITIAL_NODE_IRON: Quantity = Quantity(100);
pub const MINING_BOOTSTRAP_EXPECTED_NODE_IRON: Quantity = Quantity(90);
pub const MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON: Quantity = Quantity(10);
pub const MINING_BOOTSTRAP_WORKER_BATTERY: Quantity = Quantity(10);
pub const MINING_BOOTSTRAP_EXPECTED_FINAL_TICK: Tick = Tick(4);

pub fn build_mining_bootstrap_world() -> WorldState {
    let mut state = WorldState::new();
    state.workers.insert(
        MINING_BOOTSTRAP_WORKER_ID,
        Worker {
            id: MINING_BOOTSTRAP_WORKER_ID,
            role: WorkerRole::Miner,
            position: Position::new(0, 0),
            battery: MINING_BOOTSTRAP_WORKER_BATTERY,
            carried: None,
            status: WorkerStatus::Active,
        },
    );
    state.resource_nodes.insert(
        MINING_BOOTSTRAP_NODE_ID,
        ResourceNode {
            id: MINING_BOOTSTRAP_NODE_ID,
            kind: ResourceKind::Iron,
            position: Position::new(1, 0),
            remaining: MINING_BOOTSTRAP_INITIAL_NODE_IRON,
        },
    );

    let mut inventory = BTreeMap::new();
    inventory.insert(ResourceKind::Iron, Quantity::ZERO);
    state.storage.insert(
        MINING_BOOTSTRAP_STORAGE_ID,
        Storage {
            id: MINING_BOOTSTRAP_STORAGE_ID,
            position: Position::new(0, 1),
            inventory,
        },
    );
    state
}

pub fn mining_bootstrap_objective() -> Objective {
    Objective {
        id: MINING_BOOTSTRAP_OBJECTIVE_ID,
        kind: ObjectiveKind::MaintainStockpile {
            resource: ResourceKind::Iron,
            minimum: MINING_BOOTSTRAP_STOCKPILE_MINIMUM,
        },
    }
}

pub fn mining_bootstrap_decision() -> Decision {
    Decision {
        id: MINING_BOOTSTRAP_DECISION_ID,
        objective_id: MINING_BOOTSTRAP_OBJECTIVE_ID,
        kind: DecisionKind::CreateTask {
            task_id: MINING_BOOTSTRAP_TASK_ID,
        },
    }
}

pub fn mining_bootstrap_task() -> Task {
    Task {
        id: MINING_BOOTSTRAP_TASK_ID,
        objective_id: MINING_BOOTSTRAP_OBJECTIVE_ID,
        decision_id: Some(MINING_BOOTSTRAP_DECISION_ID),
        kind: TaskKind::MineResource {
            resource: ResourceKind::Iron,
            quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            node_id: MINING_BOOTSTRAP_NODE_ID,
        },
    }
}

pub fn mining_bootstrap_assignment() -> Assignment {
    Assignment {
        id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
        task_id: MINING_BOOTSTRAP_TASK_ID,
        worker_id: MINING_BOOTSTRAP_WORKER_ID,
    }
}

pub fn mining_bootstrap_actions() -> Vec<WorkerAction> {
    vec![
        WorkerAction::Move {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            to: Position::new(1, 0),
        },
        WorkerAction::Mine {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            node_id: MINING_BOOTSTRAP_NODE_ID,
            quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
        },
        WorkerAction::Move {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            to: Position::new(0, 0),
        },
        WorkerAction::Deposit {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            storage_id: MINING_BOOTSTRAP_STORAGE_ID,
        },
    ]
}

pub fn stockpile_quantity(
    state: &WorldState,
    storage_id: StorageId,
    resource: ResourceKind,
) -> Quantity {
    state
        .storage
        .get(&storage_id)
        .and_then(|storage| storage.inventory.get(&resource))
        .copied()
        .unwrap_or(Quantity::ZERO)
}

pub fn objective_satisfied(
    state: &WorldState,
    objective: &Objective,
    storage_id: StorageId,
) -> bool {
    match objective.kind {
        ObjectiveKind::MaintainStockpile { resource, minimum } => {
            stockpile_quantity(state, storage_id, resource) >= minimum
        }
    }
}
