use std::error::Error;
use std::fmt;

use autonomy_core::{AssignmentId, EventId, SimError, Tick, WorkerId};
use autonomy_sim::{
    apply_action, parse_proposal_text, schedule_next_action, validate_action_policy,
    validate_proposal_against_world, ActionContext, ActionPolicy, Assignment, Decision,
    FailureReason, Objective, ParsedProposal, PolicyError, ProposalRejection, RecoveryKind,
    ScheduleOutcome, Task, WorkerAction, WorldState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope {
    pub id: EventId,
    pub tick: Tick,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    ProposalReceived {
        text: String,
    },
    ProposalParsed {
        proposal: ParsedProposal,
    },
    ProposalAccepted {
        proposal: ParsedProposal,
    },
    ProposalRejected {
        rejection: ProposalRejection,
    },
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
    SchedulerEmitted {
        assignment_id: AssignmentId,
        outcome: ScheduleOutcome,
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
            Self::SchedulerEmitted { assignment_id, .. } => Some(*assignment_id),
            Self::ActionRequested { context, .. }
            | Self::ActionApplied { context, .. }
            | Self::ActionRejected { context, .. }
            | Self::PolicyAccepted { context, .. }
            | Self::PolicyRejected { context, .. } => context.assignment_id,
            Self::ObjectiveAccepted { .. }
            | Self::ProposalReceived { .. }
            | Self::ProposalParsed { .. }
            | Self::ProposalAccepted { .. }
            | Self::ProposalRejected { .. }
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
pub enum ScheduledExecutionError {
    Execution(ExecutionError),
}

impl fmt::Display for ScheduledExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(error) => write!(f, "scheduled action failed: {error}"),
        }
    }
}

impl Error for ScheduledExecutionError {}

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

pub fn record_scheduled_step(
    state: &WorldState,
    log: &mut EventLog,
    task: &Task,
    assignment: &Assignment,
    policy: &ActionPolicy,
) -> Result<WorldState, ScheduledExecutionError> {
    let outcome = schedule_next_action(state, task, assignment);
    log.append(
        state.tick,
        EventKind::SchedulerEmitted {
            assignment_id: assignment.id,
            outcome: outcome.clone(),
        },
    );

    match outcome {
        ScheduleOutcome::Action {
            assignment_id,
            action,
        } => record_action_with_policy(state, log, action, Some(assignment_id), policy)
            .map_err(ScheduledExecutionError::Execution),
        ScheduleOutcome::Complete { .. } | ScheduleOutcome::Blocked { .. } => Ok(state.clone()),
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

pub fn record_proposal_text(log: &mut EventLog, tick: Tick, text: String) -> EventEnvelope {
    log.append(tick, EventKind::ProposalReceived { text })
}

pub fn record_proposal_parsed(
    log: &mut EventLog,
    tick: Tick,
    proposal: ParsedProposal,
) -> EventEnvelope {
    log.append(tick, EventKind::ProposalParsed { proposal })
}

pub fn record_proposal_accepted(
    log: &mut EventLog,
    tick: Tick,
    proposal: ParsedProposal,
) -> EventEnvelope {
    log.append(tick, EventKind::ProposalAccepted { proposal })
}

pub fn record_proposal_rejected(
    log: &mut EventLog,
    tick: Tick,
    rejection: ProposalRejection,
) -> EventEnvelope {
    log.append(tick, EventKind::ProposalRejected { rejection })
}

pub fn parse_validate_and_record_proposal(
    state: &WorldState,
    log: &mut EventLog,
    text: &str,
) -> Result<ParsedProposal, ProposalRejection> {
    let tick = state.tick;
    record_proposal_text(log, tick, text.to_string());

    let proposal = match parse_proposal_text(text) {
        Ok(proposal) => proposal,
        Err(error) => {
            let rejection = ProposalRejection::Parse(error);
            record_proposal_rejected(log, tick, rejection.clone());
            return Err(rejection);
        }
    };

    record_proposal_parsed(log, tick, proposal.clone());

    if let Err(error) = validate_proposal_against_world(&proposal, state) {
        let rejection = ProposalRejection::Validation(error);
        record_proposal_rejected(log, tick, rejection.clone());
        return Err(rejection);
    }

    record_proposal_accepted(log, tick, proposal.clone());
    Ok(proposal)
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
        build_mining_bootstrap_world, mining_bootstrap_assignment, mining_bootstrap_task,
        ActionPolicy, PolicyError, ProposalError, ProposalRejection, ProposalValidationError,
        ScheduleOutcome, WorkerAction, MINING_BOOTSTRAP_ASSIGNMENT_ID,
        MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON, MINING_BOOTSTRAP_NODE_ID,
        MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    use crate::{
        event_log::{
            parse_validate_and_record_proposal, record_action_with_policy, record_scheduled_step,
            EventKind, EventLog, ExecutionError, ScheduledExecutionError,
        },
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

    #[test]
    fn scheduled_step_records_scheduler_emitted() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let _ = record_scheduled_step(
            &state,
            &mut log,
            &mining_bootstrap_task(),
            &mining_bootstrap_assignment(),
            &ActionPolicy::default(),
        )
        .expect("scheduled step should succeed");

        assert!(matches!(
            log.events()[0].kind,
            EventKind::SchedulerEmitted { .. }
        ));
        assert_eq!(log.events()[0].tick, state.tick);
    }

    #[test]
    fn scheduled_action_preserves_assignment_context() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let _ = record_scheduled_step(
            &state,
            &mut log,
            &mining_bootstrap_task(),
            &mining_bootstrap_assignment(),
            &ActionPolicy::default(),
        )
        .expect("scheduled step should succeed");

        assert!(matches!(
            log.events()[0].kind,
            EventKind::SchedulerEmitted {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                ..
            }
        ));
        for event in &log.events()[1..] {
            assert_eq!(
                event.kind.assignment_id(),
                Some(MINING_BOOTSTRAP_ASSIGNMENT_ID)
            );
        }
    }

    #[test]
    fn scheduled_step_uses_policy_aware_recording() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let _ = record_scheduled_step(
            &state,
            &mut log,
            &mining_bootstrap_task(),
            &mining_bootstrap_assignment(),
            &ActionPolicy::default(),
        )
        .expect("scheduled step should succeed");

        assert!(matches!(
            log.events()[0].kind,
            EventKind::SchedulerEmitted { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::PolicyAccepted { .. }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::ActionRequested { .. }
        ));
        assert!(matches!(
            log.events()[3].kind,
            EventKind::ActionApplied { .. }
        ));
    }

    #[test]
    fn policy_rejection_after_scheduler_emission_does_not_record_action_requested() {
        let state = build_mining_bootstrap_world();
        let before = state.clone();
        let mut log = EventLog::new();
        let policy = ActionPolicy {
            max_mine_quantity: Some(Quantity::new(5)),
            ..ActionPolicy::default()
        };

        let result = record_scheduled_step(
            &state,
            &mut log,
            &mining_bootstrap_task(),
            &mining_bootstrap_assignment(),
            &policy,
        );

        assert!(matches!(
            result,
            Err(ScheduledExecutionError::Execution(ExecutionError::Policy(
                PolicyError::MineQuantityLimitExceeded { .. }
            )))
        ));
        assert_eq!(state, before);
        assert_eq!(state.tick, Tick::ZERO);
        assert_eq!(log.len(), 2);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::SchedulerEmitted {
                outcome: ScheduleOutcome::Action { .. },
                ..
            }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::PolicyRejected { .. }
        ));
        assert!(!log
            .events()
            .iter()
            .any(|event| matches!(event.kind, EventKind::ActionRequested { .. })));
    }

    #[test]
    fn scheduler_events_replay_as_non_mutating_audit_facts() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        log.append(
            state.tick,
            EventKind::SchedulerEmitted {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                outcome: ScheduleOutcome::Complete {
                    assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                },
            },
        );

        let replayed =
            replay_events(&state, log.events()).expect("scheduler event replay should succeed");

        assert_eq!(replayed, state);
    }

    #[test]
    fn parse_failure_records_proposal_received_and_rejected_only() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let result = parse_validate_and_record_proposal(
            &state,
            &mut log,
            "objective=maintain_stockpile\nresource=copper\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10",
        );

        assert_eq!(
            result,
            Err(ProposalRejection::Parse(
                ProposalError::UnsupportedResource("copper".to_string())
            ))
        );
        assert_eq!(log.len(), 2);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ProposalReceived { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ProposalRejected { .. }
        ));
        assert!(!log.events().iter().any(|event| matches!(
            event.kind,
            EventKind::ObjectiveAccepted { .. }
                | EventKind::TaskCreated { .. }
                | EventKind::TaskAssigned { .. }
                | EventKind::SchedulerEmitted { .. }
                | EventKind::ActionRequested { .. }
        )));
    }

    #[test]
    fn validation_failure_records_received_parsed_and_rejected_only() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let result = parse_validate_and_record_proposal(
            &state,
            &mut log,
            "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=99\nresource_node_id=1\nstorage_id=1\nmine_quantity=10",
        );

        assert_eq!(
            result,
            Err(ProposalRejection::Validation(
                ProposalValidationError::UnknownWorker {
                    worker_id: autonomy_core::WorkerId::new(99),
                }
            ))
        );
        assert_eq!(log.len(), 3);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ProposalReceived { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ProposalParsed { .. }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::ProposalRejected { .. }
        ));
        assert!(!log.events().iter().any(|event| matches!(
            event.kind,
            EventKind::ObjectiveAccepted { .. }
                | EventKind::TaskCreated { .. }
                | EventKind::TaskAssigned { .. }
                | EventKind::SchedulerEmitted { .. }
                | EventKind::ActionRequested { .. }
        )));
    }

    #[test]
    fn accepted_proposal_records_received_parsed_and_accepted() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let proposal = parse_validate_and_record_proposal(
            &state,
            &mut log,
            "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10",
        )
        .expect("proposal should be accepted");

        assert_eq!(proposal.worker_id, MINING_BOOTSTRAP_WORKER_ID);
        assert_eq!(proposal.resource_node_id, MINING_BOOTSTRAP_NODE_ID);
        assert_eq!(proposal.storage_id, MINING_BOOTSTRAP_STORAGE_ID);
        assert_eq!(log.len(), 3);
        assert!(matches!(
            log.events()[0].kind,
            EventKind::ProposalReceived { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ProposalParsed { .. }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::ProposalAccepted { .. }
        ));
        assert!(log.events().iter().all(|event| event.tick == Tick::ZERO));
    }

    #[test]
    fn proposal_rejection_does_not_advance_tick_and_replays_without_mutation() {
        let state = build_mining_bootstrap_world();
        let before = state.clone();
        let mut log = EventLog::new();

        let result = parse_validate_and_record_proposal(
            &state,
            &mut log,
            "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=999",
        );

        assert!(matches!(
            result,
            Err(ProposalRejection::Validation(
                ProposalValidationError::InsufficientResource { .. }
            ))
        ));
        assert_eq!(state, before);
        assert_eq!(state.tick, Tick::ZERO);

        let replayed =
            replay_events(&state, log.events()).expect("proposal event replay should succeed");
        assert_eq!(replayed, state);
    }
}
