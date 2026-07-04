use autonomy_core::{AssignmentId, Position};

use crate::{Assignment, Task, TaskKind, WorkerAction, WorkerStatus, WorldState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleOutcome {
    Action {
        assignment_id: AssignmentId,
        action: WorkerAction,
    },
    Complete {
        assignment_id: AssignmentId,
    },
    Blocked {
        assignment_id: AssignmentId,
        reason: ScheduleBlockReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleBlockReason {
    UnknownWorker,
    UnknownTask,
    UnknownResourceNode,
    UnknownStorage,
    WorkerDisabled,
    WorkerAlreadyCarrying,
    WorkerNotCarrying,
    UnsupportedTask,
}

pub fn schedule_next_action(
    state: &WorldState,
    task: &Task,
    assignment: &Assignment,
) -> ScheduleOutcome {
    if assignment.task_id != task.id {
        return blocked(assignment.id, ScheduleBlockReason::UnknownTask);
    }

    let Some(worker) = state.workers.get(&assignment.worker_id) else {
        return blocked(assignment.id, ScheduleBlockReason::UnknownWorker);
    };

    if worker.status == WorkerStatus::Disabled {
        return blocked(assignment.id, ScheduleBlockReason::WorkerDisabled);
    }

    match task.kind {
        TaskKind::MineResource {
            quantity, node_id, ..
        } => {
            let Some(node) = state.resource_nodes.get(&node_id) else {
                return blocked(assignment.id, ScheduleBlockReason::UnknownResourceNode);
            };

            if worker.carried.is_some() {
                return blocked(assignment.id, ScheduleBlockReason::WorkerAlreadyCarrying);
            }

            if !worker.position.is_same_or_adjacent(node.position) {
                return ScheduleOutcome::Action {
                    assignment_id: assignment.id,
                    action: WorkerAction::Move {
                        worker_id: worker.id,
                        to: step_toward(worker.position, node.position),
                    },
                };
            }

            ScheduleOutcome::Action {
                assignment_id: assignment.id,
                action: WorkerAction::Mine {
                    worker_id: worker.id,
                    node_id,
                    quantity,
                },
            }
        }
        TaskKind::DepositResource { storage_id } => {
            let Some(storage) = state.storage.get(&storage_id) else {
                return blocked(assignment.id, ScheduleBlockReason::UnknownStorage);
            };

            if worker.carried.is_none() {
                return blocked(assignment.id, ScheduleBlockReason::WorkerNotCarrying);
            }

            if !worker.position.is_same_or_adjacent(storage.position) {
                return ScheduleOutcome::Action {
                    assignment_id: assignment.id,
                    action: WorkerAction::Move {
                        worker_id: worker.id,
                        to: step_toward(worker.position, storage.position),
                    },
                };
            }

            ScheduleOutcome::Action {
                assignment_id: assignment.id,
                action: WorkerAction::Deposit {
                    worker_id: worker.id,
                    storage_id,
                },
            }
        }
    }
}

fn blocked(assignment_id: AssignmentId, reason: ScheduleBlockReason) -> ScheduleOutcome {
    ScheduleOutcome::Blocked {
        assignment_id,
        reason,
    }
}

fn step_toward(from: Position, to: Position) -> Position {
    if from.x != to.x {
        return Position::new(from.x + (to.x - from.x).signum(), from.y);
    }

    Position::new(from.x, from.y + (to.y - from.y).signum())
}

#[cfg(test)]
mod tests {
    use autonomy_core::{
        AssignmentId, Position, Quantity, ResourceNodeId, StorageId, TaskId, WorkerId,
    };

    use crate::{
        apply_action, build_mining_bootstrap_world, mining_bootstrap_assignment,
        mining_bootstrap_task, schedule_next_action, Assignment, CarriedResource, ResourceKind,
        ScheduleBlockReason, ScheduleOutcome, Task, TaskKind, WorkerAction, WorkerStatus,
        MINING_BOOTSTRAP_ASSIGNMENT_ID, MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
        MINING_BOOTSTRAP_NODE_ID, MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    fn deposit_task() -> Task {
        Task {
            id: TaskId::new(2),
            objective_id: crate::MINING_BOOTSTRAP_OBJECTIVE_ID,
            decision_id: Some(crate::MINING_BOOTSTRAP_DECISION_ID),
            kind: TaskKind::DepositResource {
                storage_id: MINING_BOOTSTRAP_STORAGE_ID,
            },
        }
    }

    fn deposit_assignment() -> Assignment {
        Assignment {
            id: AssignmentId::new(2),
            task_id: TaskId::new(2),
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
        }
    }

    #[test]
    fn scheduler_returns_blocked_for_unknown_worker() {
        let state = build_mining_bootstrap_world();
        let task = mining_bootstrap_task();
        let assignment = Assignment {
            worker_id: WorkerId::new(99),
            ..mining_bootstrap_assignment()
        };

        assert_eq!(
            schedule_next_action(&state, &task, &assignment),
            ScheduleOutcome::Blocked {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                reason: ScheduleBlockReason::UnknownWorker,
            }
        );
    }

    #[test]
    fn scheduler_returns_blocked_for_disabled_worker() {
        let mut state = build_mining_bootstrap_world();
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .status = WorkerStatus::Disabled;

        assert_eq!(
            schedule_next_action(
                &state,
                &mining_bootstrap_task(),
                &mining_bootstrap_assignment()
            ),
            ScheduleOutcome::Blocked {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                reason: ScheduleBlockReason::WorkerDisabled,
            }
        );
    }

    #[test]
    fn scheduler_returns_blocked_for_unknown_task() {
        let state = build_mining_bootstrap_world();
        let assignment = Assignment {
            task_id: TaskId::new(99),
            ..mining_bootstrap_assignment()
        };

        assert_eq!(
            schedule_next_action(&state, &mining_bootstrap_task(), &assignment),
            ScheduleOutcome::Blocked {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                reason: ScheduleBlockReason::UnknownTask,
            }
        );
    }

    #[test]
    fn scheduler_returns_blocked_for_unknown_resource_node() {
        let state = build_mining_bootstrap_world();
        let task = Task {
            kind: TaskKind::MineResource {
                resource: ResourceKind::Iron,
                quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
                node_id: ResourceNodeId::new(99),
            },
            ..mining_bootstrap_task()
        };

        assert_eq!(
            schedule_next_action(&state, &task, &mining_bootstrap_assignment()),
            ScheduleOutcome::Blocked {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                reason: ScheduleBlockReason::UnknownResourceNode,
            }
        );
    }

    #[test]
    fn scheduler_returns_blocked_for_unknown_storage() {
        let state = build_mining_bootstrap_world();
        let task = Task {
            kind: TaskKind::DepositResource {
                storage_id: StorageId::new(99),
            },
            ..deposit_task()
        };

        assert_eq!(
            schedule_next_action(&state, &task, &deposit_assignment()),
            ScheduleOutcome::Blocked {
                assignment_id: AssignmentId::new(2),
                reason: ScheduleBlockReason::UnknownStorage,
            }
        );
    }

    #[test]
    fn mining_task_emits_deterministic_move_when_worker_is_not_adjacent_to_node() {
        let mut state = build_mining_bootstrap_world();
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .position = Position::new(-1, 0);

        assert_eq!(
            schedule_next_action(
                &state,
                &mining_bootstrap_task(),
                &mining_bootstrap_assignment()
            ),
            ScheduleOutcome::Action {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                action: WorkerAction::Move {
                    worker_id: MINING_BOOTSTRAP_WORKER_ID,
                    to: Position::new(0, 0),
                },
            }
        );
    }

    #[test]
    fn mining_task_emits_mine_action_when_worker_is_adjacent_to_node() {
        let state = build_mining_bootstrap_world();

        assert_eq!(
            schedule_next_action(
                &state,
                &mining_bootstrap_task(),
                &mining_bootstrap_assignment()
            ),
            ScheduleOutcome::Action {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                action: WorkerAction::Mine {
                    worker_id: MINING_BOOTSTRAP_WORKER_ID,
                    node_id: MINING_BOOTSTRAP_NODE_ID,
                    quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
                },
            }
        );
    }

    #[test]
    fn deposit_task_emits_deterministic_move_when_worker_is_not_adjacent_to_storage() {
        let mut state = build_mining_bootstrap_world();
        let worker = state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists");
        worker.position = Position::new(3, 1);
        worker.carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(10),
        });

        assert_eq!(
            schedule_next_action(&state, &deposit_task(), &deposit_assignment()),
            ScheduleOutcome::Action {
                assignment_id: AssignmentId::new(2),
                action: WorkerAction::Move {
                    worker_id: MINING_BOOTSTRAP_WORKER_ID,
                    to: Position::new(2, 1),
                },
            }
        );
    }

    #[test]
    fn deposit_task_emits_deposit_action_when_worker_is_adjacent_to_storage() {
        let mut state = build_mining_bootstrap_world();
        state = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            },
        )
        .expect("mine should apply");

        assert_eq!(
            schedule_next_action(&state, &deposit_task(), &deposit_assignment()),
            ScheduleOutcome::Action {
                assignment_id: AssignmentId::new(2),
                action: WorkerAction::Deposit {
                    worker_id: MINING_BOOTSTRAP_WORKER_ID,
                    storage_id: MINING_BOOTSTRAP_STORAGE_ID,
                },
            }
        );
    }

    #[test]
    fn movement_choice_is_deterministic_x_axis_first_then_y_axis() {
        let mut state = build_mining_bootstrap_world();
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .position = Position::new(-2, -3);

        assert_eq!(
            schedule_next_action(
                &state,
                &mining_bootstrap_task(),
                &mining_bootstrap_assignment()
            ),
            ScheduleOutcome::Action {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                action: WorkerAction::Move {
                    worker_id: MINING_BOOTSTRAP_WORKER_ID,
                    to: Position::new(-1, -3),
                },
            }
        );
    }

    #[test]
    fn scheduler_function_does_not_mutate_state() {
        let state = build_mining_bootstrap_world();
        let before = state.clone();

        let _ = schedule_next_action(
            &state,
            &mining_bootstrap_task(),
            &mining_bootstrap_assignment(),
        );

        assert_eq!(state, before);
    }

    #[test]
    fn mine_task_blocks_when_worker_already_carrying() {
        let mut state = build_mining_bootstrap_world();
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(1),
        });

        assert_eq!(
            schedule_next_action(
                &state,
                &mining_bootstrap_task(),
                &mining_bootstrap_assignment()
            ),
            ScheduleOutcome::Blocked {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                reason: ScheduleBlockReason::WorkerAlreadyCarrying,
            }
        );
    }

    #[test]
    fn deposit_task_blocks_when_worker_not_carrying() {
        let state = build_mining_bootstrap_world();

        assert_eq!(
            schedule_next_action(&state, &deposit_task(), &deposit_assignment()),
            ScheduleOutcome::Blocked {
                assignment_id: AssignmentId::new(2),
                reason: ScheduleBlockReason::WorkerNotCarrying,
            }
        );
    }
}
