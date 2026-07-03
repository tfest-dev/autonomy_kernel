use autonomy_core::{Quantity, SimError, WorkerId};

use crate::{
    action::WorkerAction,
    entity::{CarriedResource, Worker, WorkerRole, WorkerStatus},
    world::WorldState,
};

const ACTION_BATTERY_COST: Quantity = Quantity::ONE;

pub fn apply_action(state: &WorldState, action: &WorkerAction) -> Result<WorldState, SimError> {
    match action {
        WorkerAction::Move { worker_id, to } => apply_move(state, *worker_id, *to),
        WorkerAction::Mine {
            worker_id,
            node_id,
            quantity,
        } => apply_mine(state, *worker_id, *node_id, *quantity),
        WorkerAction::Deposit {
            worker_id,
            storage_id,
        } => apply_deposit(state, *worker_id, *storage_id),
        WorkerAction::Recharge { worker_id, amount } => apply_recharge(state, *worker_id, *amount),
        WorkerAction::Wait { worker_id } => apply_wait(state, *worker_id),
        WorkerAction::DisableWorker { worker_id } => apply_disable_worker(state, *worker_id),
        WorkerAction::RepairWorker { worker_id } => apply_repair_worker(state, *worker_id),
    }
}

fn apply_move(
    state: &WorldState,
    worker_id: WorkerId,
    to: autonomy_core::Position,
) -> Result<WorldState, SimError> {
    let worker = worker(state, worker_id)?;
    ensure_active(worker)?;
    let battery = battery_after_cost(worker, ACTION_BATTERY_COST)?;

    if !worker.position.is_adjacent(to) {
        return Err(SimError::NotAdjacent {
            from: worker.position,
            to,
        });
    }

    let mut next = state.clone();
    let worker = next
        .workers
        .get_mut(&worker_id)
        .expect("worker was validated before state clone");
    worker.position = to;
    worker.battery = battery;
    advance_tick(&mut next)?;
    Ok(next)
}

fn apply_mine(
    state: &WorldState,
    worker_id: WorkerId,
    node_id: autonomy_core::ResourceNodeId,
    quantity: Quantity,
) -> Result<WorldState, SimError> {
    if quantity.is_zero() {
        return Err(SimError::InvalidAction(
            "mine quantity must be greater than zero",
        ));
    }

    let worker = worker(state, worker_id)?;
    ensure_active(worker)?;
    let node = state
        .resource_nodes
        .get(&node_id)
        .ok_or(SimError::UnknownResourceNode(node_id))?;

    if worker.role != WorkerRole::Miner {
        return Err(SimError::InvalidAction("worker is not a miner"));
    }

    if worker.carried.is_some() {
        return Err(SimError::InvalidAction(
            "worker is already carrying a resource",
        ));
    }

    if !worker.position.is_same_or_adjacent(node.position) {
        return Err(SimError::NotAdjacent {
            from: worker.position,
            to: node.position,
        });
    }

    let battery = battery_after_cost(worker, ACTION_BATTERY_COST)?;
    let remaining = node
        .remaining
        .checked_sub(quantity)
        .ok_or(SimError::InsufficientResource {
            resource_node_id: node_id,
            requested: quantity,
            available: node.remaining,
        })?;

    let mut next = state.clone();
    let worker = next
        .workers
        .get_mut(&worker_id)
        .expect("worker was validated before state clone");
    worker.battery = battery;
    worker.carried = Some(CarriedResource {
        kind: node.kind,
        quantity,
    });

    let node = next
        .resource_nodes
        .get_mut(&node_id)
        .expect("resource node was validated before state clone");
    node.remaining = remaining;

    advance_tick(&mut next)?;
    Ok(next)
}

fn apply_deposit(
    state: &WorldState,
    worker_id: WorkerId,
    storage_id: autonomy_core::StorageId,
) -> Result<WorldState, SimError> {
    let worker = worker(state, worker_id)?;
    ensure_active(worker)?;
    let storage = state
        .storage
        .get(&storage_id)
        .ok_or(SimError::UnknownStorage(storage_id))?;

    if !worker.position.is_same_or_adjacent(storage.position) {
        return Err(SimError::NotAdjacent {
            from: worker.position,
            to: storage.position,
        });
    }

    let carried = worker
        .carried
        .as_ref()
        .ok_or(SimError::InvalidAction("worker is not carrying resource"))?;
    let battery = battery_after_cost(worker, ACTION_BATTERY_COST)?;
    let current = storage
        .inventory
        .get(&carried.kind)
        .copied()
        .unwrap_or(Quantity::ZERO);
    let inventory_total =
        current
            .checked_add(carried.quantity)
            .ok_or(SimError::CapacityExceeded {
                current,
                added: carried.quantity,
            })?;

    let mut next = state.clone();
    let worker = next
        .workers
        .get_mut(&worker_id)
        .expect("worker was validated before state clone");
    worker.battery = battery;
    worker.carried = None;

    let storage = next
        .storage
        .get_mut(&storage_id)
        .expect("storage was validated before state clone");
    storage.inventory.insert(carried.kind, inventory_total);

    advance_tick(&mut next)?;
    Ok(next)
}

fn apply_recharge(
    state: &WorldState,
    worker_id: WorkerId,
    amount: Quantity,
) -> Result<WorldState, SimError> {
    let worker = worker(state, worker_id)?;
    ensure_active(worker)?;
    let battery = worker
        .battery
        .checked_add(amount)
        .ok_or(SimError::CapacityExceeded {
            current: worker.battery,
            added: amount,
        })?;

    let mut next = state.clone();
    let worker = next
        .workers
        .get_mut(&worker_id)
        .expect("worker was validated before state clone");
    worker.battery = battery;
    advance_tick(&mut next)?;
    Ok(next)
}

fn apply_wait(state: &WorldState, worker_id: WorkerId) -> Result<WorldState, SimError> {
    let worker = worker(state, worker_id)?;
    ensure_active(worker)?;

    let mut next = state.clone();
    advance_tick(&mut next)?;
    Ok(next)
}

fn apply_disable_worker(state: &WorldState, worker_id: WorkerId) -> Result<WorldState, SimError> {
    let worker = worker(state, worker_id)?;
    if worker.status == WorkerStatus::Disabled {
        return Err(SimError::InvalidAction("worker is already disabled"));
    }

    let mut next = state.clone();
    let worker = next
        .workers
        .get_mut(&worker_id)
        .expect("worker was validated before state clone");
    worker.status = WorkerStatus::Disabled;
    advance_tick(&mut next)?;
    Ok(next)
}

fn apply_repair_worker(state: &WorldState, worker_id: WorkerId) -> Result<WorldState, SimError> {
    let worker = worker(state, worker_id)?;
    if worker.status == WorkerStatus::Active {
        return Err(SimError::InvalidAction("worker is already active"));
    }

    let mut next = state.clone();
    let worker = next
        .workers
        .get_mut(&worker_id)
        .expect("worker was validated before state clone");
    worker.status = WorkerStatus::Active;
    advance_tick(&mut next)?;
    Ok(next)
}

fn worker(state: &WorldState, worker_id: WorkerId) -> Result<&Worker, SimError> {
    state
        .workers
        .get(&worker_id)
        .ok_or(SimError::UnknownWorker(worker_id))
}

fn ensure_active(worker: &Worker) -> Result<(), SimError> {
    if worker.status == WorkerStatus::Disabled {
        return Err(SimError::WorkerDisabled(worker.id));
    }

    Ok(())
}

fn battery_after_cost(worker: &Worker, cost: Quantity) -> Result<Quantity, SimError> {
    worker
        .battery
        .checked_sub(cost)
        .ok_or(SimError::InsufficientBattery {
            worker_id: worker.id,
            required: cost,
            available: worker.battery,
        })
}

fn advance_tick(state: &mut WorldState) -> Result<(), SimError> {
    state.tick = state.tick.checked_next().ok_or(SimError::TickOverflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use autonomy_core::{Position, Quantity, ResourceNodeId, SimError, StorageId, Tick, WorkerId};

    use crate::{
        action::WorkerAction,
        entity::{
            CarriedResource, ResourceKind, ResourceNode, Storage, Worker, WorkerRole, WorkerStatus,
        },
        reducer::apply_action,
        world::WorldState,
    };

    fn worker_id(value: u64) -> WorkerId {
        WorkerId::new(value)
    }

    fn node_id(value: u64) -> ResourceNodeId {
        ResourceNodeId::new(value)
    }

    fn storage_id(value: u64) -> StorageId {
        StorageId::new(value)
    }

    fn base_world() -> WorldState {
        let mut state = WorldState::new();
        state.workers.insert(
            worker_id(1),
            Worker {
                id: worker_id(1),
                role: WorkerRole::Miner,
                position: Position::new(0, 0),
                battery: Quantity::new(3),
                carried: None,
                status: WorkerStatus::Active,
            },
        );
        state.workers.insert(
            worker_id(2),
            Worker {
                id: worker_id(2),
                role: WorkerRole::Hauler,
                position: Position::new(1, 0),
                battery: Quantity::new(3),
                carried: None,
                status: WorkerStatus::Active,
            },
        );
        state.resource_nodes.insert(
            node_id(1),
            ResourceNode {
                id: node_id(1),
                kind: ResourceKind::Iron,
                position: Position::new(0, 1),
                remaining: Quantity::new(10),
            },
        );
        state.storage.insert(
            storage_id(1),
            Storage {
                id: storage_id(1),
                position: Position::new(1, 1),
                inventory: BTreeMap::new(),
            },
        );
        state
    }

    #[test]
    fn initial_world_can_be_constructed_deterministically() {
        let state = base_world();

        assert_eq!(state.tick, Tick::ZERO);
        assert_eq!(
            state.workers.keys().copied().collect::<Vec<_>>(),
            vec![worker_id(1), worker_id(2)]
        );
        assert_eq!(
            state.resource_nodes.keys().copied().collect::<Vec<_>>(),
            vec![node_id(1)]
        );
        assert_eq!(
            state.storage.keys().copied().collect::<Vec<_>>(),
            vec![storage_id(1)]
        );
    }

    #[test]
    fn move_to_adjacent_tile_succeeds_and_advances_tick() {
        let state = base_world();
        let next = apply_action(
            &state,
            &WorkerAction::Move {
                worker_id: worker_id(1),
                to: Position::new(1, 0),
            },
        )
        .expect("adjacent move should succeed");

        let worker = next.workers.get(&worker_id(1)).expect("worker exists");
        assert_eq!(worker.position, Position::new(1, 0));
        assert_eq!(worker.battery, Quantity::new(2));
        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn move_to_non_adjacent_tile_fails_and_does_not_advance_tick() {
        let state = base_world();
        let result = apply_action(
            &state,
            &WorkerAction::Move {
                worker_id: worker_id(1),
                to: Position::new(2, 0),
            },
        );

        assert!(matches!(result, Err(SimError::NotAdjacent { .. })));
        assert_eq!(state.tick, Tick::ZERO);
    }

    #[test]
    fn move_with_insufficient_battery_fails() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().battery = Quantity::ZERO;
        let original = state.clone();

        let result = apply_action(
            &state,
            &WorkerAction::Move {
                worker_id: worker_id(1),
                to: Position::new(1, 0),
            },
        );

        assert!(matches!(result, Err(SimError::InsufficientBattery { .. })));
        assert_eq!(state, original);
    }

    #[test]
    fn miner_can_mine_iron_from_node() {
        let state = base_world();
        let next = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: worker_id(1),
                node_id: node_id(1),
                quantity: Quantity::new(4),
            },
        )
        .expect("miner should mine adjacent node");

        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn mining_reduces_node_remaining() {
        let state = base_world();
        let next = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: worker_id(1),
                node_id: node_id(1),
                quantity: Quantity::new(4),
            },
        )
        .expect("mining should succeed");

        let node = next.resource_nodes.get(&node_id(1)).expect("node exists");
        assert_eq!(node.remaining, Quantity::new(6));
    }

    #[test]
    fn mining_sets_worker_carried_resource() {
        let state = base_world();
        let next = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: worker_id(1),
                node_id: node_id(1),
                quantity: Quantity::new(4),
            },
        )
        .expect("mining should succeed");

        let worker = next.workers.get(&worker_id(1)).expect("worker exists");
        assert_eq!(
            worker.carried,
            Some(CarriedResource {
                kind: ResourceKind::Iron,
                quantity: Quantity::new(4),
            })
        );
    }

    #[test]
    fn non_miner_cannot_mine() {
        let state = base_world();
        let original = state.clone();
        let result = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: worker_id(2),
                node_id: node_id(1),
                quantity: Quantity::new(1),
            },
        );

        assert!(matches!(result, Err(SimError::InvalidAction(_))));
        assert_eq!(state, original);
    }

    #[test]
    fn worker_can_deposit_carried_iron_into_storage() {
        let mut state = base_world();
        let worker = state.workers.get_mut(&worker_id(1)).unwrap();
        worker.position = Position::new(1, 1);
        worker.carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(4),
        });

        let next = apply_action(
            &state,
            &WorkerAction::Deposit {
                worker_id: worker_id(1),
                storage_id: storage_id(1),
            },
        )
        .expect("deposit should succeed");

        let storage = next.storage.get(&storage_id(1)).expect("storage exists");
        assert_eq!(
            storage.inventory.get(&ResourceKind::Iron).copied(),
            Some(Quantity::new(4))
        );
        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn deposit_clears_worker_carried_resource() {
        let mut state = base_world();
        let worker = state.workers.get_mut(&worker_id(1)).unwrap();
        worker.position = Position::new(1, 1);
        worker.carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(4),
        });

        let next = apply_action(
            &state,
            &WorkerAction::Deposit {
                worker_id: worker_id(1),
                storage_id: storage_id(1),
            },
        )
        .expect("deposit should succeed");

        let worker = next.workers.get(&worker_id(1)).expect("worker exists");
        assert_eq!(worker.carried, None);
        assert_eq!(worker.battery, Quantity::new(2));
    }

    #[test]
    fn same_initial_state_and_action_sequence_produces_identical_final_state() {
        let initial = base_world();
        let actions = [
            WorkerAction::Move {
                worker_id: worker_id(1),
                to: Position::new(0, 1),
            },
            WorkerAction::Mine {
                worker_id: worker_id(1),
                node_id: node_id(1),
                quantity: Quantity::new(3),
            },
            WorkerAction::Move {
                worker_id: worker_id(1),
                to: Position::new(1, 1),
            },
            WorkerAction::Deposit {
                worker_id: worker_id(1),
                storage_id: storage_id(1),
            },
        ];

        let first = actions.iter().try_fold(initial.clone(), |state, action| {
            apply_action(&state, action)
        });
        let second = actions
            .iter()
            .try_fold(initial, |state, action| apply_action(&state, action));

        assert_eq!(first, second);
    }

    #[test]
    fn failed_action_leaves_state_unchanged() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().position = Position::new(1, 1);
        let original = state.clone();
        let result = apply_action(
            &state,
            &WorkerAction::Deposit {
                worker_id: worker_id(1),
                storage_id: storage_id(1),
            },
        );

        assert!(matches!(result, Err(SimError::InvalidAction(_))));
        assert_eq!(state, original);
    }

    #[test]
    fn wait_advances_tick_without_other_state_changes() {
        let state = base_world();
        let next = apply_action(
            &state,
            &WorkerAction::Wait {
                worker_id: worker_id(1),
            },
        )
        .expect("wait should succeed");

        let mut expected = state.clone();
        expected.tick = Tick::new(1);
        assert_eq!(next, expected);
    }

    #[test]
    fn recharge_is_unbounded_in_wp01_except_numeric_overflow() {
        let state = base_world();
        let next = apply_action(
            &state,
            &WorkerAction::Recharge {
                worker_id: worker_id(1),
                amount: Quantity::new(5),
            },
        )
        .expect("recharge should succeed");

        let worker = next.workers.get(&worker_id(1)).expect("worker exists");
        assert_eq!(worker.battery, Quantity::new(8));
        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn world_state_uses_deterministic_map_backed_storage() {
        let mut state = WorldState::new();
        state.workers.insert(
            worker_id(10),
            Worker {
                id: worker_id(10),
                role: WorkerRole::Hauler,
                position: Position::new(0, 0),
                battery: Quantity::ONE,
                carried: None,
                status: WorkerStatus::Active,
            },
        );
        state.workers.insert(
            worker_id(2),
            Worker {
                id: worker_id(2),
                role: WorkerRole::Miner,
                position: Position::new(0, 0),
                battery: Quantity::ONE,
                carried: None,
                status: WorkerStatus::Active,
            },
        );

        assert_eq!(
            state.workers.keys().copied().collect::<Vec<_>>(),
            vec![worker_id(2), worker_id(10)]
        );
    }

    #[test]
    fn new_workers_are_constructed_as_active() {
        let state = base_world();

        assert_eq!(
            state
                .workers
                .get(&worker_id(1))
                .expect("worker exists")
                .status,
            WorkerStatus::Active
        );
    }

    #[test]
    fn disabled_worker_cannot_move() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().status = WorkerStatus::Disabled;

        let result = apply_action(
            &state,
            &WorkerAction::Move {
                worker_id: worker_id(1),
                to: Position::new(1, 0),
            },
        );

        assert_eq!(result, Err(SimError::WorkerDisabled(worker_id(1))));
    }

    #[test]
    fn disabled_worker_cannot_mine() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().status = WorkerStatus::Disabled;

        let result = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: worker_id(1),
                node_id: node_id(1),
                quantity: Quantity::new(1),
            },
        );

        assert_eq!(result, Err(SimError::WorkerDisabled(worker_id(1))));
    }

    #[test]
    fn disabled_worker_cannot_deposit() {
        let mut state = base_world();
        let worker = state.workers.get_mut(&worker_id(1)).unwrap();
        worker.position = Position::new(1, 1);
        worker.status = WorkerStatus::Disabled;
        worker.carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(1),
        });

        let result = apply_action(
            &state,
            &WorkerAction::Deposit {
                worker_id: worker_id(1),
                storage_id: storage_id(1),
            },
        );

        assert_eq!(result, Err(SimError::WorkerDisabled(worker_id(1))));
    }

    #[test]
    fn failed_disabled_worker_action_leaves_state_unchanged() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().status = WorkerStatus::Disabled;
        let original = state.clone();

        let result = apply_action(
            &state,
            &WorkerAction::Wait {
                worker_id: worker_id(1),
            },
        );

        assert_eq!(result, Err(SimError::WorkerDisabled(worker_id(1))));
        assert_eq!(state, original);
    }

    #[test]
    fn failed_disabled_worker_action_does_not_advance_tick() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().status = WorkerStatus::Disabled;

        let result = apply_action(
            &state,
            &WorkerAction::Recharge {
                worker_id: worker_id(1),
                amount: Quantity::new(1),
            },
        );

        assert_eq!(result, Err(SimError::WorkerDisabled(worker_id(1))));
        assert_eq!(state.tick, Tick::ZERO);
    }

    #[test]
    fn disable_worker_disables_active_worker_and_advances_tick() {
        let state = base_world();

        let next = apply_action(
            &state,
            &WorkerAction::DisableWorker {
                worker_id: worker_id(1),
            },
        )
        .expect("disable should succeed");

        assert_eq!(
            next.workers
                .get(&worker_id(1))
                .expect("worker exists")
                .status,
            WorkerStatus::Disabled
        );
        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn disable_worker_rejects_already_disabled_worker() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().status = WorkerStatus::Disabled;
        let original = state.clone();

        let result = apply_action(
            &state,
            &WorkerAction::DisableWorker {
                worker_id: worker_id(1),
            },
        );

        assert!(matches!(result, Err(SimError::InvalidAction(_))));
        assert_eq!(state, original);
    }

    #[test]
    fn repair_worker_reactivates_disabled_worker_and_advances_tick() {
        let mut state = base_world();
        state.workers.get_mut(&worker_id(1)).unwrap().status = WorkerStatus::Disabled;

        let next = apply_action(
            &state,
            &WorkerAction::RepairWorker {
                worker_id: worker_id(1),
            },
        )
        .expect("repair should succeed");

        assert_eq!(
            next.workers
                .get(&worker_id(1))
                .expect("worker exists")
                .status,
            WorkerStatus::Active
        );
        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn repair_worker_rejects_already_active_worker() {
        let state = base_world();
        let original = state.clone();

        let result = apply_action(
            &state,
            &WorkerAction::RepairWorker {
                worker_id: worker_id(1),
            },
        );

        assert!(matches!(result, Err(SimError::InvalidAction(_))));
        assert_eq!(state, original);
    }

    #[test]
    fn repair_preserves_worker_position() {
        let mut state = base_world();
        let worker = state.workers.get_mut(&worker_id(1)).unwrap();
        worker.status = WorkerStatus::Disabled;
        worker.position = Position::new(1, 0);

        let next = apply_action(
            &state,
            &WorkerAction::RepairWorker {
                worker_id: worker_id(1),
            },
        )
        .expect("repair should succeed");

        assert_eq!(
            next.workers
                .get(&worker_id(1))
                .expect("worker exists")
                .position,
            Position::new(1, 0)
        );
    }

    #[test]
    fn repair_preserves_worker_carried_resource() {
        let mut state = base_world();
        let worker = state.workers.get_mut(&worker_id(1)).unwrap();
        worker.status = WorkerStatus::Disabled;
        worker.carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(2),
        });

        let next = apply_action(
            &state,
            &WorkerAction::RepairWorker {
                worker_id: worker_id(1),
            },
        )
        .expect("repair should succeed");

        assert_eq!(
            next.workers
                .get(&worker_id(1))
                .expect("worker exists")
                .carried,
            Some(CarriedResource {
                kind: ResourceKind::Iron,
                quantity: Quantity::new(2),
            })
        );
    }
}
