use autonomy_core::{AssignmentId, EventId, SimError, Tick, WorkerId};
use autonomy_sim::{
    apply_action, ActionContext, Assignment, Decision, FailureReason, Objective, RecoveryKind,
    Task, WorkerAction, WorldState,
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
            | Self::ActionRejected { context, .. } => context.assignment_id,
            Self::ObjectiveAccepted { .. }
            | Self::DecisionEmitted { .. }
            | Self::TaskCreated { .. }
            | Self::FailureInjected { .. }
            | Self::RecoveryEmitted { .. } => None,
        }
    }
}

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
