use std::error::Error;
use std::fmt;

use autonomy_core::{AssignmentId, EventId, SimError, Tick, WorkerId};
use autonomy_sim::{
    apply_action, validate_action_policy, ActionContext, ActionPolicy, Assignment, Decision,
    FailureReason, Objective, PolicyError, RecoveryKind, Task, WorkerAction, WorldState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope {
    pub id: EventId,
    pub tick: Tick,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    ObjectiveAccepted {
        objective: Objective,
    },
    DecisionEmitted {
        decision: Decision,
    },
    TaskCreated {
        task: Task,
    },
    TaskAssigned {
        assignment: Assignment,
    },
    FailureInjected {
        worker_id: WorkerId,
        reason: FailureReason,
    },
    RecoveryEmitted {
        worker_id: WorkerId,
        recovery: RecoveryKind,
    },
    PolicyAccepted {
        action: WorkerAction,
        context: ActionContext,
    },
    PolicyRejected {
        action: WorkerAction,
        context: ActionContext,
        error: PolicyError,
    },
    ActionRequested {
        action: WorkerAction,
        context: ActionContext,
    },
    ActionApplied {
        action: WorkerAction,
        context: ActionContext,
        resulting_tick: Tick,
    },
    ActionRejected {
        action: WorkerAction,
        context: ActionContext,
        error: SimError,
    },
}

impl EventKind {
    pub fn assignment_id(&self) -> Option<AssignmentId> {
        match self {
            Self::TaskAssigned { assignment } => Some(assignment.id),
            Self::ActionRequested { context, .. }
            | Self::ActionApplied { context, .. }
            | Self::ActionRejected { context, .. }
            | Self::PolicyAccepted { context, .. }
            | Self::PolicyRejected { context, .. } => context.assignment_id,
            Self::ObjectiveAccepted { .. }
            | Self::DecisionEmitted { .. }
            | Self::TaskCreated { .. }
            | Self::FailureInjected { .. }
            | Self::RecoveryEmitted { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionError {
    Policy(PolicyError),
    Sim(SimError),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(error) => write!(f, "policy rejected action: {error}"),
            Self::Sim(error) => write!(f, "action execution failed: {error}"),
        }
    }
}

impl Error for ExecutionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventLog {
    events: Vec<EventEnvelope>,
    next_id: EventId,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_id: EventId::new(1),
        }
    }

    pub fn append(&mut self, tick: Tick, kind: EventKind) -> EventEnvelope {
        let envelope = EventEnvelope {
            id: self.next_id,
            tick,
            kind,
        };
        self.next_id = self
            .next_id
            .checked_next()
            .expect("expect id overflow while appending to event log");
        self.events.push(envelope.clone());
        envelope
    }

    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn record_action(
    state: &WorldState,
    log: &mut EventLog,
    action: WorkerAction,
) -> Result<WorldState, SimError> {
    record_action_with_context(state, log, action, None)
}

pub fn record_action_with_context(
    state: &WorldState,
    log: &mut EventLog,
    action: WorkerAction,
    assignment_id: Option<AssignmentId>,
) -> Result<WorldState, SimError> {
    let context = ActionContext { assignment_id };
    let pre_action_tick = state.tick;
    log.append(
        pre_action_tick,
        EventKind::ActionRequested {
            action: action.clone(),
            context,
        },
    );

    match apply_action(state, &action) {
        Ok(next_state) => {
            let resulting_tick = next_state.tick;
            log.append(
                resulting_tick,
                EventKind::ActionApplied {
                    action,
                    context,
                    resulting_tick,
                },
            );
            Ok(next_state)
        }
        Err(error) => {
            log.append(
                pre_action_tick,
                EventKind::ActionRejected {
                    action,
                    context,
                    error: error.clone(),
                },
            );
            Err(error)
        }
    }
}

pub fn record_action_with_policy(
    state: &WorldState,
    log: &mut EventLog,
    action: WorkerAction,
    assignment_id: Option<AssignmentId>,
    policy: &ActionPolicy,
) -> Result<WorldState, ExecutionError> {
    let context = ActionContext { assignment_id };
    let pre_action_tick = state.tick;

    match validate_action_policy(state, &action, policy) {
        Ok(()) => {
            log.append(
                pre_action_tick,
                EventKind::PolicyAccepted {
                    action: action.clone(),
                    context,
                },
            );
            record_action_with_context(state, log, action, assignment_id)
                .map_err(ExecutionError::Sim)
        }
        Err(error) => {
            log.append(
                pre_action_tick,
                EventKind::PolicyRejected {
                    action,
                    context,
                    error: error.clone(),
                },
            );
            Err(ExecutionError::Policy(error))
        }
    }
}

pub fn record_objective_accepted(
    log: &mut EventLog,
    tick: Tick,
    objective: Objective,
) -> EventEnvelope {
    log.append(tick, EventKind::ObjectiveAccepted { objective })
}

pub fn record_decision_emitted(
    log: &mut EventLog,
    tick: Tick,
    decision: Decision,
) -> EventEnvelope {
    log.append(tick, EventKind::DecisionEmitted { decision })
}

pub fn record_task_created(log: &mut EventLog, tick: Tick, task: Task) -> EventEnvelope {
    log.append(tick, EventKind::TaskCreated { task })
}

pub fn record_task_assigned(
    log: &mut EventLog,
    tick: Tick,
    assignment: Assignment,
) -> EventEnvelope {
    log.append(tick, EventKind::TaskAssigned { assignment })
}

pub fn record_worker_failure(
    state: &WorldState,
    log: &mut EventLog,
    worker_id: WorkerId,
    assignment_id: Option<AssignmentId>,
) -> Result<WorldState, SimError> {
    log.append(
        state.tick,
        EventKind::FailureInjected {
            worker_id,
            reason: FailureReason::Injected,
        },
    );
    record_action_with_context(
        state,
        log,
        WorkerAction::DisableWorker { worker_id },
        assignment_id,
    )
}

pub fn record_worker_recovery(
    state: &WorldState,
    log: &mut EventLog,
    worker_id: WorkerId,
    assignment_id: Option<AssignmentId>,
) -> Result<WorldState, SimError> {
    log.append(
        state.tick,
        EventKind::RecoveryEmitted {
            worker_id,
            recovery: RecoveryKind::RepairWorker,
        },
    );
    record_action_with_context(
        state,
        log,
        WorkerAction::RepairWorker { worker_id },
        assignment_id,
    )
}

pub fn assignment_for_action_event(event: &EventEnvelope) -> Option<AssignmentId> {
    match &event.kind {
        EventKind::ActionRequested { context, .. }
        | EventKind::ActionApplied { context, .. }
        | EventKind::ActionRejected { context, .. } => context.assignment_id,
        _ => None,
    }
}

pub fn events_for_assignment(
    events: &[EventEnvelope],
    assignment_id: AssignmentId,
) -> Vec<&EventEnvelope> {
    events
        .iter()
        .filter(|event| event.kind.assignment_id() == Some(assignment_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use autonomy_core::{Position, Quantity, SimError, Tick};
    use autonomy_sim::{
        build_mining_bootstrap_world, ActionPolicy, PolicyError, WorkerAction,
        MINING_BOOTSTRAP_ASSIGNMENT_ID, MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
        MINING_BOOTSTRAP_NODE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    use crate::{
        event_log::{record_action_with_policy, EventKind, EventLog, ExecutionError},
        replay::replay_events,
    };

    #[test]
    fn policy_rejection_records_policy_rejected_without_action_requested() {
        let state = build_mining_bootstrap_world();
        let before = state.clone();
        let mut log = EventLog::new();
        let policy = ActionPolicy {
            max_mine_quantity: Some(Quantity::new(10)),
            ..ActionPolicy::default()
        };

        let result = record_action_with_policy(
            &state,
            &mut log,
            WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: Quantity::new(20),
            },
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
            &policy,
        );

        assert_eq!(
            result,
            Err(ExecutionError::Policy(
                PolicyError::MineQuantityLimitExceeded {
                    requested: Quantity::new(20),
                    maximum: Quantity::new(10),
                }
            ))
        );
        assert_eq!(state, before);
        assert_eq!(state.tick, Tick::ZERO);
        assert_eq!(log.len(), 1);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::PolicyRejected { .. }
        ));
        assert!(!log
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::ActionRequested { .. })));
    }

    #[test]
    fn policy_accepted_successful_action_records_policy_requested_and_applied_events() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        let policy = ActionPolicy::default();

        let next = record_action_with_policy(
            &state,
            &mut log,
            WorkerAction::Move {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                to: Position::new(1, 0),
            },
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
            &policy,
        )
        .expect("policy-accepted move should apply");

        assert_eq!(next.tick, Tick::new(1));
        assert_eq!(log.len(), 3);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::PolicyAccepted { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionRequested { .. }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::ActionApplied { .. }
        ));
        assert_eq!(log.events()[0].tick, Tick::ZERO);
        assert_eq!(log.events()[1].tick, Tick::ZERO);
        assert_eq!(log.events()[2].tick, Tick::new(1));
    }

    #[test]
    fn policy_accepted_reducer_failure_records_requested_and_rejected_events() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        let policy = ActionPolicy::default();

        let result = record_action_with_policy(
            &state,
            &mut log,
            WorkerAction::Move {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                to: Position::new(9, 9),
            },
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
            &policy,
        );

        assert!(matches!(
            result,
            Err(ExecutionError::Sim(SimError::NotAdjacent { .. }))
        ));
        assert_eq!(log.len(), 3);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::PolicyAccepted { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionRequested { .. }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::ActionRejected { .. }
        ));
        assert_eq!(log.events()[2].tick, state.tick);
    }

    #[test]
    fn policy_rejection_replays_as_non_mutating_audit_fact() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        let policy = ActionPolicy {
            max_mine_quantity: Some(Quantity::new(10)),
            ..ActionPolicy::default()
        };

        let result = record_action_with_policy(
            &state,
            &mut log,
            WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: Quantity::new(20),
            },
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
            &policy,
        );

        assert!(matches!(result, Err(ExecutionError::Policy(_))));
        let replayed =
            replay_events(&state, log.events()).expect("policy event replay should succeed");
        assert_eq!(replayed, state);
    }

    #[test]
    fn policy_accepted_action_replays_through_applied_action_event() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        let policy = ActionPolicy {
            max_mine_quantity: Some(MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON),
            ..ActionPolicy::default()
        };

        let next = record_action_with_policy(
            &state,
            &mut log,
            WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            },
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
            &policy,
        )
        .expect("policy-accepted mine should apply");

        let replayed =
            replay_events(&state, log.events()).expect("policy event replay should succeed");
        assert_eq!(replayed, next);
    }
}
