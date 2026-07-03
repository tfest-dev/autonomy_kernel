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
            EventKind::ActionRequested { .. } => {
                validate_event_tick(event.id, state.tick, event.tick)?;
            }
            EventKind::ActionApplied {
                action,
                resulting_tick,
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
            EventKind::ActionRejected { action, error } => {
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
        EventId, Position, Quantity, ResourceNodeId, SimError, StorageId, Tick, WorkerId,
    };
    use autonomy_sim::{
        CarriedResource, ResourceKind, ResourceNode, Storage, Worker, WorkerAction, WorkerRole,
        WorldState,
    };

    use crate::{
        event_log::{record_action, EventEnvelope, EventKind, EventLog},
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
            },
        );
        let second = log.append(
            Tick::ZERO,
            EventKind::ActionRejected {
                action: invalid_move_action(),
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
    fn replay_rejects_non_monotonic_event_ids() {
        let events = vec![
            EventEnvelope {
                id: EventId::new(2),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
                },
            },
            EventEnvelope {
                id: EventId::new(1),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
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
                },
            },
            EventEnvelope {
                id: EventId::new(1),
                tick: Tick::ZERO,
                kind: EventKind::ActionRequested {
                    action: wait_action(),
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
    fn event_order_is_preserved() {
        let mut log = EventLog::new();
        let first = log.append(
            Tick::ZERO,
            EventKind::ActionRequested {
                action: wait_action(),
            },
        );
        let second = log.append(
            Tick::new(1),
            EventKind::ActionApplied {
                action: wait_action(),
                resulting_tick: Tick::new(1),
            },
        );
        let third = log.append(
            Tick::new(1),
            EventKind::ActionRequested {
                action: invalid_move_action(),
            },
        );

        assert_eq!(log.events()[0], first);
        assert_eq!(log.events()[1], second);
        assert_eq!(log.events()[2], third);
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

    #[test]
    fn replay_detects_rejected_action_error_mismatch() {
        let events = vec![EventEnvelope {
            id: EventId::new(1),
            tick: Tick::ZERO,
            kind: EventKind::ActionRejected {
                action: invalid_move_action(),
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
