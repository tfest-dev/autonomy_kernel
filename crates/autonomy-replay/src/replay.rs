use std::error::Error;
use std::fmt;

use autonomy_core::{EventId, SimError, Tick};
use autonomy_sim::{apply_action, WorldState};

use crate::event_log::{EventEnvelope, EventKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    InvalidEventId {
        event_id: EventId,
    },
    DuplicateEventId {
        event_id: EventId,
    },
    NonMonotonicEventId {
        previous: EventId,
        current: EventId,
    },
    EventTickMismatch {
        event_id: EventId,
        expected: Tick,
        actual: Tick,
    },
    AppliedActionFailed {
        event_id: EventId,
        error: SimError,
    },
    RejectedActionUnexpectedlySucceeded {
        event_id: EventId,
    },
    RejectedActionErrorMismatch {
        event_id: EventId,
        expected: SimError,
        actual: SimError,
    },
    ResultingTickMismatch {
        event_id: EventId,
        expected: Tick,
        actual: Tick,
    },
    FinalStateMismatch,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEventId { event_id } => {
                write!(f, "invalid event id: {}", event_id.value())
            }
            Self::DuplicateEventId { event_id } => {
                write!(f, "duplicate event id: {}", event_id.value())
            }
            Self::NonMonotonicEventId { previous, current } => write!(
                f,
                "non-monotonic event id: previous {}, current {}",
                previous.value(),
                current.value()
            ),
            Self::EventTickMismatch {
                event_id,
                expected,
                actual,
            } => write!(
                f,
                "event {} has tick mismatch: expected {}, actual {}",
                event_id.value(),
                expected.value(),
                actual.value()
            ),
            Self::AppliedActionFailed { event_id, error } => {
                write!(
                    f,
                    "applied action in event {} failed during replay: {error}",
                    event_id.value()
                )
            }
            Self::RejectedActionUnexpectedlySucceeded { event_id } => write!(
                f,
                "rejected action in event {} unexpectedly succeeded during replay",
                event_id.value()
            ),
            Self::RejectedActionErrorMismatch {
                event_id,
                expected,
                actual,
            } => write!(
                f,
                "rejected action in event {} failed with a different error: expected {expected}, actual {actual}",
                event_id.value()
            ),
            Self::ResultingTickMismatch {
                event_id,
                expected,
                actual,
            } => write!(
                f,
                "event {} resulting tick mismatch: expected {}, actual {}",
                event_id.value(),
                expected.value(),
                actual.value()
            ),
            Self::FinalStateMismatch => write!(f, "replayed final state does not match expected"),
        }
    }
}

impl Error for ReplayError {}

pub fn replay_events(
    initial_state: &WorldState,
    events: &[EventEnvelope],
) -> Result<WorldState, ReplayError> {
    let mut state = initial_state.clone();
    let mut previous_id = None;

    for event in events {
        validate_event_id(event.id, previous_id)?;

        match &event.kind {
            EventKind::ObjectiveAccepted { .. }
            | EventKind::DecisionEmitted { .. }
            | EventKind::TaskCreated { .. }
            | EventKind::TaskAssigned { .. }
            | EventKind::FailureInjected { .. }
            | EventKind::RecoveryEmitted { .. }
            | EventKind::SchedulerEmitted { .. }
            | EventKind::PolicyAccepted { .. }
            | EventKind::PolicyRejected { .. } => {
                validate_event_tick(event.id, state.tick, event.tick)?;
            }
            EventKind::ActionRequested { .. } => {
                validate_event_tick(event.id, state.tick, event.tick)?;
            }
            EventKind::ActionApplied {
                action,
                resulting_tick,
                ..
            } => {
                let next_state = apply_action(&state, action).map_err(|error| {
                    ReplayError::AppliedActionFailed {
                        event_id: event.id,
                        error,
                    }
                })?;

                if next_state.tick != *resulting_tick {
                    return Err(ReplayError::ResultingTickMismatch {
                        event_id: event.id,
                        expected: next_state.tick,
                        actual: *resulting_tick,
                    });
                }

                validate_event_tick(event.id, next_state.tick, event.tick)?;
                state = next_state;
            }
            EventKind::ActionRejected { action, error, .. } => {
                validate_event_tick(event.id, state.tick, event.tick)?;

                match apply_action(&state, action) {
                    Ok(_) => {
                        return Err(ReplayError::RejectedActionUnexpectedlySucceeded {
                            event_id: event.id,
                        });
                    }
                    Err(actual) if actual != *error => {
                        return Err(ReplayError::RejectedActionErrorMismatch {
                            event_id: event.id,
                            expected: error.clone(),
                            actual,
                        });
                    }
                    Err(_) => {}
                }
            }
        }

        previous_id = Some(event.id);
    }

    Ok(state)
}

fn validate_event_id(event_id: EventId, previous_id: Option<EventId>) -> Result<(), ReplayError> {
    if event_id.value() == 0 {
        return Err(ReplayError::InvalidEventId { event_id });
    }

    if let Some(previous) = previous_id {
        if event_id == previous {
            return Err(ReplayError::DuplicateEventId { event_id });
        }

        if event_id < previous {
            return Err(ReplayError::NonMonotonicEventId {
                previous,
                current: event_id,
            });
        }
    }

    Ok(())
}

fn validate_event_tick(event_id: EventId, expected: Tick, actual: Tick) -> Result<(), ReplayError> {
    if expected != actual {
        return Err(ReplayError::EventTickMismatch {
            event_id,
            expected,
            actual,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use autonomy_core::{
        AssignmentId, DecisionId, EventId, ObjectiveId, Position, Quantity, ResourceNodeId,
        SimError, StorageId, TaskId, Tick, WorkerId,
    };
    use autonomy_sim::{
        ActionContext, Assignment, CarriedResource, Decision, DecisionKind, Objective,
        ObjectiveKind, ResourceKind, ResourceNode, Storage, Task, TaskKind, Worker, WorkerAction,
        WorkerRole, WorkerStatus, WorldState,
    };

    use crate::{
        event_log::{
            assignment_for_action_event, events_for_assignment, record_action,
            record_action_with_context, record_decision_emitted, record_objective_accepted,
            record_task_assigned, record_task_created, EventEnvelope, EventKind, EventLog,
        },
        replay::{replay_events, ReplayError},
        verification::verify_replay,
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

    fn objective_id(value: u64) -> ObjectiveId {
        ObjectiveId::new(value)
    }

    fn decision_id(value: u64) -> DecisionId {
        DecisionId::new(value)
    }

    fn task_id(value: u64) -> TaskId {
        TaskId::new(value)
    }

    fn assignment_id(value: u64) -> AssignmentId {
        AssignmentId::new(value)
    }

    fn base_world() -> WorldState {
        let mut state = WorldState::new();
        state.workers.insert(
            worker_id(1),
            Worker {
                id: worker_id(1),
                role: WorkerRole::Miner,
                position: Position::new(0, 0),
                battery: Quantity::new(4),
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
                battery: Quantity::new(4),
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

    fn stockpile_objective() -> Objective {
        Objective {
            id: objective_id(1),
            kind: ObjectiveKind::MaintainStockpile {
                resource: ResourceKind::Iron,
                minimum: Quantity::new(5),
            },
        }
    }

    fn create_task_decision() -> Decision {
        Decision {
            id: decision_id(1),
            objective_id: objective_id(1),
            kind: DecisionKind::CreateTask {
                task_id: task_id(1),
            },
        }
    }

    fn mine_task() -> Task {
        Task {
            id: task_id(1),
            objective_id: objective_id(1),
            decision_id: Some(decision_id(1)),
            kind: TaskKind::MineResource {
                resource: ResourceKind::Iron,
                quantity: Quantity::new(2),
                node_id: node_id(1),
            },
        }
    }

    fn miner_assignment() -> Assignment {
        Assignment {
            id: assignment_id(1),
            task_id: task_id(1),
            worker_id: worker_id(1),
        }
    }

    fn mine_action() -> WorkerAction {
        WorkerAction::Mine {
            worker_id: worker_id(1),
            node_id: node_id(1),
            quantity: Quantity::new(2),
        }
    }

    fn wait_action() -> WorkerAction {
        WorkerAction::Wait {
            worker_id: worker_id(1),
        }
    }

    fn invalid_move_action() -> WorkerAction {
        WorkerAction::Move {
            worker_id: worker_id(1),
            to: Position::new(2, 0),
        }
    }

    #[test]
    fn objective_decision_task_and_assignment_ids_are_typed_and_orderable() {
        let mut objectives = BTreeMap::new();
        objectives.insert(objective_id(2), "second");
        objectives.insert(objective_id(1), "first");
        assert_eq!(
            objectives.keys().copied().collect::<Vec<_>>(),
            vec![objective_id(1), objective_id(2)]
        );

        let mut decisions = BTreeMap::new();
        decisions.insert(decision_id(2), "second");
        decisions.insert(decision_id(1), "first");
        assert_eq!(
            decisions.keys().copied().collect::<Vec<_>>(),
            vec![decision_id(1), decision_id(2)]
        );

        let mut tasks = BTreeMap::new();
        tasks.insert(task_id(2), "second");
        tasks.insert(task_id(1), "first");
        assert_eq!(
            tasks.keys().copied().collect::<Vec<_>>(),
            vec![task_id(1), task_id(2)]
        );

        let mut assignments = BTreeMap::new();
        assignments.insert(assignment_id(2), "second");
        assignments.insert(assignment_id(1), "first");
        assert_eq!(
            assignments.keys().copied().collect::<Vec<_>>(),
            vec![assignment_id(1), assignment_id(2)]
        );
    }

    #[test]
    fn empty_event_log_starts_with_length_zero() {
        let log = EventLog::new();

        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.events(), &[]);
    }

    #[test]
    fn first_appended_event_has_event_id_one() {
        let mut log = EventLog::new();
        let event = log.append(
            Tick::ZERO,
            EventKind::ActionRequested {
                action: wait_action(),
                context: ActionContext::DIRECT,
            },
        );

        assert_eq!(event.id, EventId::new(1));
        assert_eq!(log.events()[0].id, EventId::new(1));
    }

    #[test]
    fn event_ids_increment_deterministically() {
        let mut log = EventLog::new();

        let first = log.append(
            Tick::ZERO,
            EventKind::ActionRequested {
                action: wait_action(),
                context: ActionContext::DIRECT,
            },
        );
        let second = log.append(
            Tick::ZERO,
            EventKind::ActionRejected {
                action: invalid_move_action(),
                context: ActionContext::DIRECT,
                error: SimError::NotAdjacent {
                    from: Position::new(0, 0),
                    to: Position::new(2, 0),
                },
            },
        );

        assert_eq!(first.id, EventId::new(1));
        assert_eq!(second.id, EventId::new(2));
    }

    #[test]
    fn objective_accepted_appends_deterministically_with_next_event_id() {
        let mut log = EventLog::new();
        let event = record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());

        assert_eq!(event.id, EventId::new(1));
        assert_eq!(event.tick, Tick::ZERO);
        assert!(matches!(
            event.kind,
            EventKind::ObjectiveAccepted {
                objective: Objective {
                    id: ObjectiveId(1),
                    ..
                }
            }
        ));
    }

    #[test]
    fn decision_emitted_appends_deterministically() {
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        let event = record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());

        assert_eq!(event.id, EventId::new(2));
        assert!(matches!(
            event.kind,
            EventKind::DecisionEmitted {
                decision: Decision {
                    id: DecisionId(1),
                    ..
                }
            }
        ));
    }

    #[test]
    fn task_created_appends_deterministically() {
        let mut log = EventLog::new();
        let event = record_task_created(&mut log, Tick::ZERO, mine_task());

        assert_eq!(event.id, EventId::new(1));
        assert!(matches!(
            event.kind,
            EventKind::TaskCreated {
                task: Task { id: TaskId(1), .. }
            }
        ));
    }

    #[test]
    fn task_assigned_appends_deterministically() {
        let mut log = EventLog::new();
        let event = record_task_assigned(&mut log, Tick::ZERO, miner_assignment());

        assert_eq!(event.id, EventId::new(1));
        assert!(matches!(
            event.kind,
            EventKind::TaskAssigned {
                assignment: Assignment {
                    id: AssignmentId(1),
                    ..
                }
            }
        ));
    }

    #[test]
    fn lifecycle_events_do_not_mutate_world_state_during_replay() {
        let initial = base_world();
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());
        record_task_created(&mut log, Tick::ZERO, mine_task());
        record_task_assigned(&mut log, Tick::ZERO, miner_assignment());

        let replayed = replay_events(&initial, log.events()).expect("replay should succeed");

        assert_eq!(replayed, initial);
    }

    #[test]
    fn direct_action_recording_still_works_without_assignment_context() {
        let initial = base_world();
        let mut log = EventLog::new();
        let final_state =
            record_action(&initial, &mut log, wait_action()).expect("wait should succeed");

        assert_eq!(final_state.tick, Tick::new(1));
        assert_eq!(log.events()[0].kind.assignment_id(), None);
        assert_eq!(log.events()[1].kind.assignment_id(), None);
    }

    #[test]
    fn assigned_successful_action_records_assignment_id_on_requested_and_applied() {
        let initial = base_world();
        let mut log = EventLog::new();
        let final_state =
            record_action_with_context(&initial, &mut log, mine_action(), Some(assignment_id(1)))
                .expect("assigned mining should succeed");

        assert_eq!(final_state.tick, Tick::new(1));
        assert_eq!(log.events()[0].kind.assignment_id(), Some(assignment_id(1)));
        assert_eq!(log.events()[1].kind.assignment_id(), Some(assignment_id(1)));
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ActionRequested {
                context: ActionContext {
                    assignment_id: Some(AssignmentId(1))
                },
                ..
            }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionApplied {
                context: ActionContext {
                    assignment_id: Some(AssignmentId(1))
                },
                ..
            }
        ));
    }

    #[test]
    fn assigned_failed_action_records_assignment_id_on_requested_and_rejected() {
        let initial = base_world();
        let mut log = EventLog::new();
        let result = record_action_with_context(
            &initial,
            &mut log,
            invalid_move_action(),
            Some(assignment_id(1)),
        );

        assert!(matches!(result, Err(SimError::NotAdjacent { .. })));
        assert_eq!(log.events()[0].kind.assignment_id(), Some(assignment_id(1)));
        assert_eq!(log.events()[1].kind.assignment_id(), Some(assignment_id(1)));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionRejected {
                context: ActionContext {
                    assignment_id: Some(AssignmentId(1))
                },
                ..
            }
        ));
    }

    #[test]
    fn successful_recorded_action_appends_requested_and_applied_events() {
        let state = base_world();
        let mut log = EventLog::new();
        let next = record_action(&state, &mut log, wait_action()).expect("wait should succeed");

        assert_eq!(next.tick, Tick::new(1));
        assert_eq!(log.len(), 2);
        assert_eq!(log.events()[0].id, EventId::new(1));
        assert_eq!(log.events()[0].tick, Tick::ZERO);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ActionRequested { .. }
        ));
        assert_eq!(log.events()[1].id, EventId::new(2));
        assert_eq!(log.events()[1].tick, Tick::new(1));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionApplied {
                resulting_tick: Tick(1),
                ..
            }
        ));
    }

    #[test]
    fn failed_recorded_action_appends_requested_and_rejected_events() {
        let state = base_world();
        let mut log = EventLog::new();
        let result = record_action(&state, &mut log, invalid_move_action());

        assert!(matches!(result, Err(SimError::NotAdjacent { .. })));
        assert_eq!(log.len(), 2);
        assert_eq!(log.events()[0].tick, Tick::ZERO);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ActionRequested { .. }
        ));
        assert_eq!(log.events()[1].tick, Tick::ZERO);
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionRejected {
                error: SimError::NotAdjacent { .. },
                ..
            }
        ));
    }

    #[test]
    fn successful_recorded_action_advances_state_tick() {
        let state = base_world();
        let mut log = EventLog::new();
        let next = record_action(&state, &mut log, wait_action()).expect("wait should succeed");

        assert_eq!(state.tick, Tick::ZERO);
        assert_eq!(next.tick, Tick::new(1));
    }

    #[test]
    fn failed_recorded_action_does_not_advance_state_tick() {
        let state = base_world();
        let mut log = EventLog::new();
        let result = record_action(&state, &mut log, invalid_move_action());

        assert!(result.is_err());
        assert_eq!(state.tick, Tick::ZERO);
    }

    #[test]
    fn failed_recorded_action_leaves_state_unchanged() {
        let state = base_world();
        let original = state.clone();
        let mut log = EventLog::new();
        let result = record_action(&state, &mut log, invalid_move_action());

        assert!(result.is_err());
        assert_eq!(state, original);
    }

    #[test]
    fn replay_of_successful_action_events_reproduces_final_state() {
        let initial = base_world();
        let mut log = EventLog::new();
        let final_state =
            record_action(&initial, &mut log, wait_action()).expect("wait should succeed");

        let replayed = replay_events(&initial, log.events()).expect("replay should succeed");

        assert_eq!(replayed, final_state);
        verify_replay(&initial, log.events(), &final_state).expect("verification should pass");
    }

    #[test]
    fn replay_including_rejected_action_reproduces_final_state() {
        let initial = base_world();
        let mut log = EventLog::new();
        let final_state =
            record_action(&initial, &mut log, wait_action()).expect("wait should succeed");
        let rejected = record_action(&final_state, &mut log, invalid_move_action());

        assert!(rejected.is_err());
        let replayed = replay_events(&initial, log.events()).expect("replay should succeed");

        assert_eq!(replayed, final_state);
    }

    #[test]
    fn replay_of_lifecycle_events_plus_assigned_successful_action_reproduces_final_state() {
        let initial = base_world();
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());
        record_task_created(&mut log, Tick::ZERO, mine_task());
        record_task_assigned(&mut log, Tick::ZERO, miner_assignment());
        let final_state =
            record_action_with_context(&initial, &mut log, mine_action(), Some(assignment_id(1)))
                .expect("assigned mining should succeed");

        let replayed = replay_events(&initial, log.events()).expect("replay should succeed");

        assert_eq!(replayed, final_state);
    }

    #[test]
    fn replay_of_lifecycle_events_plus_assigned_rejected_action_reproduces_final_state() {
        let initial = base_world();
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());
        record_task_created(&mut log, Tick::ZERO, mine_task());
        record_task_assigned(&mut log, Tick::ZERO, miner_assignment());
        let rejected = record_action_with_context(
            &initial,
            &mut log,
            invalid_move_action(),
            Some(assignment_id(1)),
        );

        assert!(rejected.is_err());
        let replayed = replay_events(&initial, log.events()).expect("replay should succeed");

        assert_eq!(replayed, initial);
    }

    #[test]
    fn replay_rejects_non_monotonic_event_ids() {
        let events = vec![
            EventEnvelope {
                id: EventId::new(2),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
                    context: ActionContext::DIRECT,
                },
            },
            EventEnvelope {
                id: EventId::new(1),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
                    context: ActionContext::DIRECT,
                },
            },
        ];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(
            result,
            Err(ReplayError::NonMonotonicEventId { .. })
        ));
    }

    #[test]
    fn replay_rejects_duplicate_event_ids() {
        let events = vec![
            EventEnvelope {
                id: EventId::new(1),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
                    context: ActionContext::DIRECT,
                },
            },
            EventEnvelope {
                id: EventId::new(1),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
                    context: ActionContext::DIRECT,
                },
            },
        ];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(result, Err(ReplayError::DuplicateEventId { .. })));
    }

    #[test]
    fn replay_rejects_applied_action_that_cannot_be_applied() {
        let events = vec![EventEnvelope {
            id: EventId::new(1),
            tick: Tick::new(1),
            kind: EventKind::ActionApplied {
                action: invalid_move_action(),
                context: ActionContext::DIRECT,
                resulting_tick: Tick::new(1),
            },
        }];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(
            result,
            Err(ReplayError::AppliedActionFailed { .. })
        ));
    }

    #[test]
    fn replay_rejects_rejected_action_that_would_succeed() {
        let events = vec![EventEnvelope {
            id: EventId::new(1),
            tick: Tick::ZERO,
            kind: EventKind::ActionRejected {
                action: wait_action(),
                context: ActionContext::DIRECT,
                error: SimError::UnknownWorker(worker_id(99)),
            },
        }];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(
            result,
            Err(ReplayError::RejectedActionUnexpectedlySucceeded { .. })
        ));
    }

    #[test]
    fn replay_rejects_resulting_tick_mismatch() {
        let events = vec![EventEnvelope {
            id: EventId::new(1),
            tick: Tick::new(1),
            kind: EventKind::ActionApplied {
                action: wait_action(),
                context: ActionContext::DIRECT,
                resulting_tick: Tick::new(2),
            },
        }];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(
            result,
            Err(ReplayError::ResultingTickMismatch { .. })
        ));
    }

    #[test]
    fn replay_rejects_event_tick_mismatch() {
        let events = vec![EventEnvelope {
            id: EventId::new(1),
            tick: Tick::new(1),
            kind: EventKind::ActionRequested {
                action: wait_action(),
                context: ActionContext::DIRECT,
            },
        }];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(result, Err(ReplayError::EventTickMismatch { .. })));
    }

    #[test]
    fn same_initial_state_and_recorded_actions_produce_identical_events_and_final_state() {
        let actions = [
            wait_action(),
            invalid_move_action(),
            WorkerAction::Recharge {
                worker_id: worker_id(1),
                amount: Quantity::new(2),
            },
        ];

        let (first_state, first_events) = record_sequence(base_world(), &actions);
        let (second_state, second_events) = record_sequence(base_world(), &actions);

        assert_eq!(first_state, second_state);
        assert_eq!(first_events, second_events);
    }

    #[test]
    fn same_manual_causal_sequence_produces_identical_events_and_final_state() {
        let (first_state, first_events) = record_causal_sequence(base_world());
        let (second_state, second_events) = record_causal_sequence(base_world());

        assert_eq!(first_state, second_state);
        assert_eq!(first_events, second_events);
    }

    #[test]
    fn event_order_is_preserved() {
        let mut log = EventLog::new();
        let first = log.append(
            Tick::ZERO,
            EventKind::ActionRequested {
                action: wait_action(),
                context: ActionContext::DIRECT,
            },
        );
        let second = log.append(
            Tick::new(1),
            EventKind::ActionApplied {
                action: wait_action(),
                context: ActionContext::DIRECT,
                resulting_tick: Tick::new(1),
            },
        );
        let third = log.append(
            Tick::new(1),
            EventKind::ActionRequested {
                action: invalid_move_action(),
                context: ActionContext::DIRECT,
            },
        );

        assert_eq!(log.events()[0], first);
        assert_eq!(log.events()[1], second);
        assert_eq!(log.events()[2], third);
    }

    #[test]
    fn causal_event_order_is_preserved_from_objective_to_worker_action() {
        let initial = base_world();
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());
        record_task_created(&mut log, Tick::ZERO, mine_task());
        record_task_assigned(&mut log, Tick::ZERO, miner_assignment());
        let _final_state =
            record_action_with_context(&initial, &mut log, mine_action(), Some(assignment_id(1)))
                .expect("assigned mining should succeed");

        assert_eq!(
            log.events()
                .iter()
                .map(|event| event.id)
                .collect::<Vec<_>>(),
            vec![
                EventId::new(1),
                EventId::new(2),
                EventId::new(3),
                EventId::new(4),
                EventId::new(5),
                EventId::new(6),
            ]
        );
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::DecisionEmitted { .. }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::TaskCreated { .. }
        ));
        assert!(matches!(
            log.events()[3].kind,
            EventKind::TaskAssigned { .. }
        ));
        assert!(matches!(
            log.events()[4].kind,
            EventKind::ActionRequested { .. }
        ));
        assert!(matches!(
            log.events()[5].kind,
            EventKind::ActionApplied { .. }
        ));
    }

    #[test]
    fn helper_retrieves_assignment_id_from_action_events() {
        let initial = base_world();
        let mut log = EventLog::new();
        record_action_with_context(&initial, &mut log, mine_action(), Some(assignment_id(1)))
            .expect("assigned mining should succeed");

        assert_eq!(
            assignment_for_action_event(&log.events()[0]),
            Some(assignment_id(1))
        );
        assert_eq!(
            assignment_for_action_event(&log.events()[1]),
            Some(assignment_id(1))
        );
    }

    #[test]
    fn helper_filters_events_for_assignment() {
        let initial = base_world();
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());
        record_task_created(&mut log, Tick::ZERO, mine_task());
        record_task_assigned(&mut log, Tick::ZERO, miner_assignment());
        record_action_with_context(&initial, &mut log, mine_action(), Some(assignment_id(1)))
            .expect("assigned mining should succeed");

        let assignment_events = events_for_assignment(log.events(), assignment_id(1));

        assert_eq!(assignment_events.len(), 3);
        assert!(matches!(
            assignment_events[0].kind,
            EventKind::TaskAssigned { .. }
        ));
        assert!(matches!(
            assignment_events[1].kind,
            EventKind::ActionRequested { .. }
        ));
        assert!(matches!(
            assignment_events[2].kind,
            EventKind::ActionApplied { .. }
        ));
    }

    #[test]
    fn replay_from_empty_event_list_returns_initial_state_unchanged() {
        let initial = base_world();
        let replayed = replay_events(&initial, &[]).expect("empty replay should succeed");

        assert_eq!(replayed, initial);
    }

    fn record_sequence(
        initial: WorldState,
        actions: &[WorkerAction],
    ) -> (WorldState, Vec<EventEnvelope>) {
        let mut state = initial;
        let mut log = EventLog::new();

        for action in actions {
            if let Ok(next_state) = record_action(&state, &mut log, action.clone()) {
                state = next_state;
            }
        }

        (state, log.events().to_vec())
    }

    fn record_causal_sequence(initial: WorldState) -> (WorldState, Vec<EventEnvelope>) {
        let mut log = EventLog::new();
        record_objective_accepted(&mut log, Tick::ZERO, stockpile_objective());
        record_decision_emitted(&mut log, Tick::ZERO, create_task_decision());
        record_task_created(&mut log, Tick::ZERO, mine_task());
        record_task_assigned(&mut log, Tick::ZERO, miner_assignment());
        let state =
            record_action_with_context(&initial, &mut log, mine_action(), Some(assignment_id(1)))
                .expect("assigned mining should succeed");

        (state, log.events().to_vec())
    }

    #[test]
    fn replay_detects_rejected_action_error_mismatch() {
        let events = vec![EventEnvelope {
            id: EventId::new(1),
            tick: Tick::ZERO,
            kind: EventKind::ActionRejected {
                action: invalid_move_action(),
                context: ActionContext::DIRECT,
                error: SimError::UnknownWorker(worker_id(99)),
            },
        }];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(
            result,
            Err(ReplayError::RejectedActionErrorMismatch { .. })
        ));
    }

    #[test]
    fn verify_replay_rejects_final_state_mismatch() {
        let initial = base_world();
        let mut log = EventLog::new();
        let _final_state =
            record_action(&initial, &mut log, wait_action()).expect("wait should succeed");

        let result = verify_replay(&initial, log.events(), &initial);

        assert!(matches!(result, Err(ReplayError::FinalStateMismatch)));
    }

    #[test]
    fn replay_rejects_zero_event_id() {
        let events = vec![EventEnvelope {
            id: EventId::new(0),
            tick: Tick::ZERO,
            kind: EventKind::ActionRequested {
                action: wait_action(),
                context: ActionContext::DIRECT,
            },
        }];

        let result = replay_events(&base_world(), &events);

        assert!(matches!(result, Err(ReplayError::InvalidEventId { .. })));
    }

    #[test]
    fn replayed_deposit_events_preserve_inventory_state() {
        let mut initial = base_world();
        let worker = initial.workers.get_mut(&worker_id(1)).unwrap();
        worker.position = Position::new(1, 1);
        worker.carried = Some(CarriedResource {
            kind: ResourceKind::Iron,
            quantity: Quantity::new(3),
        });

        let mut log = EventLog::new();
        let final_state = record_action(
            &initial,
            &mut log,
            WorkerAction::Deposit {
                worker_id: worker_id(1),
                storage_id: storage_id(1),
            },
        )
        .expect("deposit should succeed");

        let replayed = replay_events(&initial, log.events()).expect("replay should succeed");

        assert_eq!(replayed, final_state);
        assert_eq!(
            replayed
                .storage
                .get(&storage_id(1))
                .and_then(|storage| storage.inventory.get(&ResourceKind::Iron))
                .copied(),
            Some(Quantity::new(3))
        );
    }
}
