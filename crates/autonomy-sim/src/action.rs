use autonomy_core::{Position, Quantity, ResourceNodeId, StorageId, WorkerId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerAction {
    Move {
        worker_id: WorkerId,
        to: Position,
    },
    Mine {
        worker_id: WorkerId,
        node_id: ResourceNodeId,
        quantity: Quantity,
    },
    Deposit {
        worker_id: WorkerId,
        storage_id: StorageId,
    },
    Recharge {
        worker_id: WorkerId,
        amount: Quantity,
    },
    Wait {
        worker_id: WorkerId,
    },
    DisableWorker {
        worker_id: WorkerId,
    },
    RepairWorker {
        worker_id: WorkerId,
    },
}
