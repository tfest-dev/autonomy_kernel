use std::error::Error;
use std::fmt;

use autonomy_core::{
    AssignmentId, Position, Quantity, ResourceNodeId, SimError, StorageId, TaskId, Tick, WorkerId,
};
use autonomy_sim::{
    accepted_proposal_to_plan, build_mining_bootstrap_world, mining_bootstrap_actions,
    mining_bootstrap_assignment, mining_bootstrap_decision, mining_bootstrap_objective,
    mining_bootstrap_task, objective_satisfied, stockpile_quantity, ActionPolicy, Assignment,
    PolicyError, ProposalPlanIds, ProposalRejection, ResourceKind, Task, TaskKind, WorkerAction,
    WorkerStatus, WorldState, MINING_BOOTSTRAP_ASSIGNMENT_ID, MINING_BOOTSTRAP_DECISION_ID,
    MINING_BOOTSTRAP_EXPECTED_FINAL_TICK, MINING_BOOTSTRAP_EXPECTED_NODE_IRON,
    MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON, MINING_BOOTSTRAP_NODE_ID,
    MINING_BOOTSTRAP_OBJECTIVE_ID, MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_TASK_ID,
    MINING_BOOTSTRAP_WORKER_ID,
};

use crate::{
    event_log::{
        parse_validate_and_record_proposal, record_action_with_context, record_action_with_policy,
        record_decision_emitted, record_objective_accepted, record_scheduled_step,
        record_task_assigned, record_task_created, record_worker_failure, record_worker_recovery,
        EventEnvelope, EventLog, ExecutionError, ScheduledExecutionError,
    },
    replay::ReplayError,
    verification::verify_replay,
};

const POLICY_GATE_REJECTED_MINE_QUANTITY: Quantity = Quantity(20);
const POLICY_GATE_EXPECTED_FINAL_TICK: Tick = Tick(2);
const SCHEDULED_MINING_DEPOSIT_TASK_ID: TaskId = TaskId(2);
const SCHEDULED_MINING_DEPOSIT_ASSIGNMENT_ID: AssignmentId = AssignmentId(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioRun {
    pub initial_state: WorldState,
    pub final_state: WorldState,
    pub events: Vec<EventEnvelope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioError {
    ActionFailed(SimError),
    PolicyFailed(PolicyError),
    ProposalRejected(ProposalRejection),
    ReplayFailed(ReplayError),
    ObjectiveNotSatisfied,
    ExpectedPolicyRejectionMissing,
    ExpectedWorkerMissing(WorkerId),
    ExpectedResourceNodeMissing(ResourceNodeId),
    ExpectedStorageMissing(StorageId),
    WorkerStillCarrying(WorkerId),
    StorageQuantityMismatch {
        expected: Quantity,
        actual: Quantity,
    },
    ResourceQuantityMismatch {
        expected: Quantity,
        actual: Quantity,
    },
    TickMismatch {
        expected: Tick,
        actual: Tick,
    },
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActionFailed(error) => write!(f, "scenario action failed: {error}"),
            Self::PolicyFailed(error) => write!(f, "scenario policy check failed: {error}"),
            Self::ProposalRejected(error) => write!(f, "scenario proposal rejected: {error}"),
            Self::ReplayFailed(error) => write!(f, "scenario replay failed: {error}"),
            Self::ObjectiveNotSatisfied => write!(f, "scenario objective is not satisfied"),
            Self::ExpectedPolicyRejectionMissing => {
                write!(f, "scenario expected a policy rejection that did not occur")
            }
            Self::ExpectedWorkerMissing(worker_id) => {
                write!(f, "expected worker missing: {}", worker_id.value())
            }
            Self::ExpectedResourceNodeMissing(node_id) => {
                write!(f, "expected resource node missing: {}", node_id.value())
            }
            Self::ExpectedStorageMissing(storage_id) => {
                write!(f, "expected storage missing: {}", storage_id.value())
            }
            Self::WorkerStillCarrying(worker_id) => {
                write!(f, "worker {} is still carrying resource", worker_id.value())
            }
            Self::StorageQuantityMismatch { expected, actual } => write!(
                f,
                "storage quantity mismatch: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::ResourceQuantityMismatch { expected, actual } => write!(
                f,
                "resource quantity mismatch: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::TickMismatch { expected, actual } => write!(
                f,
                "tick mismatch: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
        }
    }
}

impl Error for ScenarioError {}

pub fn run_mining_bootstrap() -> Result<ScenarioRun, ScenarioError> {
    let initial_state = build_mining_bootstrap_world();
    let objective = mining_bootstrap_objective();
    let mut final_state = initial_state.clone();
    let mut log = EventLog::new();

    record_objective_accepted(&mut log, initial_state.tick, objective.clone());
    record_decision_emitted(&mut log, initial_state.tick, mining_bootstrap_decision());
    record_task_created(&mut log, initial_state.tick, mining_bootstrap_task());
    record_task_assigned(&mut log, initial_state.tick, mining_bootstrap_assignment());

    for action in mining_bootstrap_actions() {
        final_state = record_action_with_context(
            &final_state,
            &mut log,
            action,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        )
        .map_err(ScenarioError::ActionFailed)?;
    }

    validate_mining_bootstrap_final_state(&final_state)?;
    if !objective_satisfied(&final_state, &objective, MINING_BOOTSTRAP_STORAGE_ID) {
        return Err(ScenarioError::ObjectiveNotSatisfied);
    }

    verify_replay(&initial_state, log.events(), &final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(ScenarioRun {
        initial_state,
        final_state,
        events: log.events().to_vec(),
    })
}

pub fn run_worker_failure_recovery() -> Result<ScenarioRun, ScenarioError> {
    let initial_state = build_mining_bootstrap_world();
    let objective = mining_bootstrap_objective();
    let mut final_state = initial_state.clone();
    let mut log = EventLog::new();

    record_objective_accepted(&mut log, initial_state.tick, objective.clone());
    record_decision_emitted(&mut log, initial_state.tick, mining_bootstrap_decision());
    record_task_created(&mut log, initial_state.tick, mining_bootstrap_task());
    record_task_assigned(&mut log, initial_state.tick, mining_bootstrap_assignment());

    final_state = record_action_with_context(
        &final_state,
        &mut log,
        WorkerAction::Move {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            to: Position::new(1, 0),
        },
        Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
    )
    .map_err(ScenarioError::ActionFailed)?;

    final_state = record_worker_failure(
        &final_state,
        &mut log,
        MINING_BOOTSTRAP_WORKER_ID,
        Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
    )
    .map_err(ScenarioError::ActionFailed)?;

    match record_action_with_context(
        &final_state,
        &mut log,
        WorkerAction::Mine {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            node_id: MINING_BOOTSTRAP_NODE_ID,
            quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
        },
        Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
    ) {
        Err(SimError::WorkerDisabled(MINING_BOOTSTRAP_WORKER_ID)) => {}
        Err(error) => return Err(ScenarioError::ActionFailed(error)),
        Ok(_) => {
            return Err(ScenarioError::ActionFailed(SimError::InvalidAction(
                "disabled worker action unexpectedly succeeded",
            )));
        }
    }

    final_state = record_worker_recovery(
        &final_state,
        &mut log,
        MINING_BOOTSTRAP_WORKER_ID,
        Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
    )
    .map_err(ScenarioError::ActionFailed)?;

    for action in [
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
    ] {
        final_state = record_action_with_context(
            &final_state,
            &mut log,
            action,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        )
        .map_err(ScenarioError::ActionFailed)?;
    }

    validate_worker_failure_recovery_final_state(&final_state)?;
    if !objective_satisfied(&final_state, &objective, MINING_BOOTSTRAP_STORAGE_ID) {
        return Err(ScenarioError::ObjectiveNotSatisfied);
    }

    verify_replay(&initial_state, log.events(), &final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(ScenarioRun {
        initial_state,
        final_state,
        events: log.events().to_vec(),
    })
}

pub fn run_policy_gate() -> Result<ScenarioRun, ScenarioError> {
    let initial_state = build_mining_bootstrap_world();
    let objective = mining_bootstrap_objective();
    let mut final_state = initial_state.clone();
    let mut log = EventLog::new();
    let policy = policy_gate_policy();

    record_objective_accepted(&mut log, initial_state.tick, objective.clone());
    record_decision_emitted(&mut log, initial_state.tick, mining_bootstrap_decision());
    record_task_created(&mut log, initial_state.tick, mining_bootstrap_task());
    record_task_assigned(&mut log, initial_state.tick, mining_bootstrap_assignment());

    match record_action_with_policy(
        &final_state,
        &mut log,
        WorkerAction::Mine {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            node_id: MINING_BOOTSTRAP_NODE_ID,
            quantity: POLICY_GATE_REJECTED_MINE_QUANTITY,
        },
        Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        &policy,
    ) {
        Err(ExecutionError::Policy(PolicyError::MineQuantityLimitExceeded {
            requested,
            maximum,
        })) if requested == POLICY_GATE_REJECTED_MINE_QUANTITY
            && maximum == MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON => {}
        Err(ExecutionError::Policy(error)) => return Err(ScenarioError::PolicyFailed(error)),
        Err(ExecutionError::Sim(error)) => return Err(ScenarioError::ActionFailed(error)),
        Ok(_) => return Err(ScenarioError::ExpectedPolicyRejectionMissing),
    }

    for action in [
        WorkerAction::Mine {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            node_id: MINING_BOOTSTRAP_NODE_ID,
            quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
        },
        WorkerAction::Deposit {
            worker_id: MINING_BOOTSTRAP_WORKER_ID,
            storage_id: MINING_BOOTSTRAP_STORAGE_ID,
        },
    ] {
        final_state = record_action_with_policy(
            &final_state,
            &mut log,
            action,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
            &policy,
        )
        .map_err(|error| match error {
            ExecutionError::Policy(error) => ScenarioError::PolicyFailed(error),
            ExecutionError::Sim(error) => ScenarioError::ActionFailed(error),
        })?;
    }

    validate_policy_gate_final_state(&final_state)?;
    if !objective_satisfied(&final_state, &objective, MINING_BOOTSTRAP_STORAGE_ID) {
        return Err(ScenarioError::ObjectiveNotSatisfied);
    }

    verify_replay(&initial_state, log.events(), &final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(ScenarioRun {
        initial_state,
        final_state,
        events: log.events().to_vec(),
    })
}

pub fn run_scheduled_mining() -> Result<ScenarioRun, ScenarioError> {
    let initial_state = build_mining_bootstrap_world();
    let objective = mining_bootstrap_objective();
    let mine_task = mining_bootstrap_task();
    let mine_assignment = mining_bootstrap_assignment();
    let deposit_task = scheduled_mining_deposit_task();
    let deposit_assignment = scheduled_mining_deposit_assignment();
    let policy = scheduled_mining_policy();
    let mut final_state = initial_state.clone();
    let mut log = EventLog::new();

    record_objective_accepted(&mut log, initial_state.tick, objective.clone());
    record_decision_emitted(&mut log, initial_state.tick, mining_bootstrap_decision());
    record_task_created(&mut log, initial_state.tick, mine_task.clone());
    record_task_assigned(&mut log, initial_state.tick, mine_assignment.clone());

    final_state = scheduled_step(
        &final_state,
        &mut log,
        &mine_task,
        &mine_assignment,
        &policy,
    )?;

    record_task_created(&mut log, final_state.tick, deposit_task.clone());
    record_task_assigned(&mut log, final_state.tick, deposit_assignment.clone());

    final_state = scheduled_step(
        &final_state,
        &mut log,
        &deposit_task,
        &deposit_assignment,
        &policy,
    )?;

    validate_scheduled_mining_final_state(&final_state)?;
    if !objective_satisfied(&final_state, &objective, MINING_BOOTSTRAP_STORAGE_ID) {
        return Err(ScenarioError::ObjectiveNotSatisfied);
    }

    verify_replay(&initial_state, log.events(), &final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(ScenarioRun {
        initial_state,
        final_state,
        events: log.events().to_vec(),
    })
}

pub fn run_proposal_adaptor() -> Result<ScenarioRun, ScenarioError> {
    let initial_state = build_mining_bootstrap_world();
    let mut final_state = initial_state.clone();
    let mut log = EventLog::new();
    let policy = scheduled_mining_policy();

    match parse_validate_and_record_proposal(
        &final_state,
        &mut log,
        invalid_proposal_adaptor_text(),
    ) {
        Err(ProposalRejection::Parse(_)) => {}
        Err(error) => return Err(ScenarioError::ProposalRejected(error)),
        Ok(_) => {
            return Err(ScenarioError::ProposalRejected(ProposalRejection::Parse(
                autonomy_sim::ProposalError::UnsupportedResource("copper".to_string()),
            )));
        }
    }

    let proposal =
        parse_validate_and_record_proposal(&final_state, &mut log, valid_proposal_adaptor_text())
            .map_err(ScenarioError::ProposalRejected)?;
    let plan = accepted_proposal_to_plan(&proposal, proposal_adaptor_plan_ids());

    record_objective_accepted(&mut log, final_state.tick, plan.objective.clone());
    record_decision_emitted(&mut log, final_state.tick, plan.decision);
    record_task_created(&mut log, final_state.tick, plan.mine_task.clone());
    record_task_assigned(&mut log, final_state.tick, plan.mine_assignment.clone());

    final_state = scheduled_step(
        &final_state,
        &mut log,
        &plan.mine_task,
        &plan.mine_assignment,
        &policy,
    )?;

    record_task_created(&mut log, final_state.tick, plan.deposit_task.clone());
    record_task_assigned(&mut log, final_state.tick, plan.deposit_assignment.clone());

    final_state = scheduled_step(
        &final_state,
        &mut log,
        &plan.deposit_task,
        &plan.deposit_assignment,
        &policy,
    )?;

    validate_scheduled_mining_final_state(&final_state)?;
    if !objective_satisfied(&final_state, &plan.objective, MINING_BOOTSTRAP_STORAGE_ID) {
        return Err(ScenarioError::ObjectiveNotSatisfied);
    }

    verify_replay(&initial_state, log.events(), &final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(ScenarioRun {
        initial_state,
        final_state,
        events: log.events().to_vec(),
    })
}

pub fn validate_mining_bootstrap_final_state(state: &WorldState) -> Result<(), ScenarioError> {
    let storage = state.storage.get(&MINING_BOOTSTRAP_STORAGE_ID).ok_or(
        ScenarioError::ExpectedStorageMissing(MINING_BOOTSTRAP_STORAGE_ID),
    )?;
    let storage_iron = storage
        .inventory
        .get(&ResourceKind::Iron)
        .copied()
        .unwrap_or(Quantity::ZERO);
    if storage_iron != MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON {
        return Err(ScenarioError::StorageQuantityMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            actual: storage_iron,
        });
    }

    let node = state.resource_nodes.get(&MINING_BOOTSTRAP_NODE_ID).ok_or(
        ScenarioError::ExpectedResourceNodeMissing(MINING_BOOTSTRAP_NODE_ID),
    )?;
    if node.remaining != MINING_BOOTSTRAP_EXPECTED_NODE_IRON {
        return Err(ScenarioError::ResourceQuantityMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_NODE_IRON,
            actual: node.remaining,
        });
    }

    let worker = state.workers.get(&MINING_BOOTSTRAP_WORKER_ID).ok_or(
        ScenarioError::ExpectedWorkerMissing(MINING_BOOTSTRAP_WORKER_ID),
    )?;
    if worker.carried.is_some() {
        return Err(ScenarioError::WorkerStillCarrying(
            MINING_BOOTSTRAP_WORKER_ID,
        ));
    }

    if state.tick != MINING_BOOTSTRAP_EXPECTED_FINAL_TICK {
        return Err(ScenarioError::TickMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_FINAL_TICK,
            actual: state.tick,
        });
    }

    Ok(())
}

pub fn validate_policy_gate_final_state(state: &WorldState) -> Result<(), ScenarioError> {
    let storage = state.storage.get(&MINING_BOOTSTRAP_STORAGE_ID).ok_or(
        ScenarioError::ExpectedStorageMissing(MINING_BOOTSTRAP_STORAGE_ID),
    )?;
    let storage_iron = storage
        .inventory
        .get(&ResourceKind::Iron)
        .copied()
        .unwrap_or(Quantity::ZERO);
    if storage_iron != MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON {
        return Err(ScenarioError::StorageQuantityMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            actual: storage_iron,
        });
    }

    let node = state.resource_nodes.get(&MINING_BOOTSTRAP_NODE_ID).ok_or(
        ScenarioError::ExpectedResourceNodeMissing(MINING_BOOTSTRAP_NODE_ID),
    )?;
    if node.remaining != MINING_BOOTSTRAP_EXPECTED_NODE_IRON {
        return Err(ScenarioError::ResourceQuantityMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_NODE_IRON,
            actual: node.remaining,
        });
    }

    let worker = state.workers.get(&MINING_BOOTSTRAP_WORKER_ID).ok_or(
        ScenarioError::ExpectedWorkerMissing(MINING_BOOTSTRAP_WORKER_ID),
    )?;
    if worker.carried.is_some() {
        return Err(ScenarioError::WorkerStillCarrying(
            MINING_BOOTSTRAP_WORKER_ID,
        ));
    }

    if state.tick != POLICY_GATE_EXPECTED_FINAL_TICK {
        return Err(ScenarioError::TickMismatch {
            expected: POLICY_GATE_EXPECTED_FINAL_TICK,
            actual: state.tick,
        });
    }

    Ok(())
}

pub fn validate_scheduled_mining_final_state(state: &WorldState) -> Result<(), ScenarioError> {
    validate_policy_gate_final_state(state)
}

pub fn validate_worker_failure_recovery_final_state(
    state: &WorldState,
) -> Result<(), ScenarioError> {
    let storage = state.storage.get(&MINING_BOOTSTRAP_STORAGE_ID).ok_or(
        ScenarioError::ExpectedStorageMissing(MINING_BOOTSTRAP_STORAGE_ID),
    )?;
    let storage_iron = storage
        .inventory
        .get(&ResourceKind::Iron)
        .copied()
        .unwrap_or(Quantity::ZERO);
    if storage_iron != MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON {
        return Err(ScenarioError::StorageQuantityMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            actual: storage_iron,
        });
    }

    let node = state.resource_nodes.get(&MINING_BOOTSTRAP_NODE_ID).ok_or(
        ScenarioError::ExpectedResourceNodeMissing(MINING_BOOTSTRAP_NODE_ID),
    )?;
    if node.remaining != MINING_BOOTSTRAP_EXPECTED_NODE_IRON {
        return Err(ScenarioError::ResourceQuantityMismatch {
            expected: MINING_BOOTSTRAP_EXPECTED_NODE_IRON,
            actual: node.remaining,
        });
    }

    let worker = state.workers.get(&MINING_BOOTSTRAP_WORKER_ID).ok_or(
        ScenarioError::ExpectedWorkerMissing(MINING_BOOTSTRAP_WORKER_ID),
    )?;
    if worker.carried.is_some() {
        return Err(ScenarioError::WorkerStillCarrying(
            MINING_BOOTSTRAP_WORKER_ID,
        ));
    }
    if worker.status != WorkerStatus::Active {
        return Err(ScenarioError::ActionFailed(SimError::WorkerDisabled(
            MINING_BOOTSTRAP_WORKER_ID,
        )));
    }

    let expected_tick = Tick::new(6);
    if state.tick != expected_tick {
        return Err(ScenarioError::TickMismatch {
            expected: expected_tick,
            actual: state.tick,
        });
    }

    Ok(())
}

pub fn mining_bootstrap_stockpile_quantity(state: &WorldState) -> Quantity {
    stockpile_quantity(state, MINING_BOOTSTRAP_STORAGE_ID, ResourceKind::Iron)
}

fn scheduled_mining_policy() -> ActionPolicy {
    ActionPolicy {
        min_battery_reserve: Some(Quantity::ONE),
        allow_disable_worker: false,
        allow_repair_worker: false,
        max_mine_quantity: Some(MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON),
    }
}

fn scheduled_mining_deposit_task() -> Task {
    Task {
        id: SCHEDULED_MINING_DEPOSIT_TASK_ID,
        objective_id: MINING_BOOTSTRAP_OBJECTIVE_ID,
        decision_id: None,
        kind: TaskKind::DepositResource {
            storage_id: MINING_BOOTSTRAP_STORAGE_ID,
        },
    }
}

fn scheduled_mining_deposit_assignment() -> Assignment {
    Assignment {
        id: SCHEDULED_MINING_DEPOSIT_ASSIGNMENT_ID,
        task_id: SCHEDULED_MINING_DEPOSIT_TASK_ID,
        worker_id: MINING_BOOTSTRAP_WORKER_ID,
    }
}

fn proposal_adaptor_plan_ids() -> ProposalPlanIds {
    ProposalPlanIds {
        objective_id: MINING_BOOTSTRAP_OBJECTIVE_ID,
        decision_id: MINING_BOOTSTRAP_DECISION_ID,
        mine_task_id: MINING_BOOTSTRAP_TASK_ID,
        mine_assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
        deposit_task_id: SCHEDULED_MINING_DEPOSIT_TASK_ID,
        deposit_assignment_id: SCHEDULED_MINING_DEPOSIT_ASSIGNMENT_ID,
    }
}

fn invalid_proposal_adaptor_text() -> &'static str {
    "objective=maintain_stockpile\nresource=copper\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10"
}

fn valid_proposal_adaptor_text() -> &'static str {
    "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10"
}

fn scheduled_step(
    state: &WorldState,
    log: &mut EventLog,
    task: &Task,
    assignment: &Assignment,
    policy: &ActionPolicy,
) -> Result<WorldState, ScenarioError> {
    record_scheduled_step(state, log, task, assignment, policy).map_err(|error| match error {
        ScheduledExecutionError::Execution(ExecutionError::Policy(error)) => {
            ScenarioError::PolicyFailed(error)
        }
        ScheduledExecutionError::Execution(ExecutionError::Sim(error)) => {
            ScenarioError::ActionFailed(error)
        }
    })
}

fn policy_gate_policy() -> ActionPolicy {
    ActionPolicy {
        min_battery_reserve: Some(Quantity::ONE),
        allow_disable_worker: false,
        allow_repair_worker: false,
        max_mine_quantity: Some(MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON),
    }
}

#[cfg(test)]
mod tests {
    use autonomy_core::{EventId, Position, Quantity, SimError, Tick};
    use autonomy_sim::{
        build_mining_bootstrap_world, mining_bootstrap_objective, objective_satisfied, PolicyError,
        ScheduleOutcome, WorkerAction, WorkerStatus, MINING_BOOTSTRAP_ASSIGNMENT_ID,
        MINING_BOOTSTRAP_EXPECTED_FINAL_TICK, MINING_BOOTSTRAP_EXPECTED_NODE_IRON,
        MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON, MINING_BOOTSTRAP_INITIAL_NODE_IRON,
        MINING_BOOTSTRAP_NODE_ID, MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    use crate::{
        assignment_for_action_event,
        event_log::{
            record_action_with_context, record_worker_failure, record_worker_recovery, EventKind,
            EventLog,
        },
        replay::replay_events,
        scenario::{
            mining_bootstrap_stockpile_quantity, run_mining_bootstrap, run_policy_gate,
            run_proposal_adaptor, run_scheduled_mining, run_worker_failure_recovery,
            validate_mining_bootstrap_final_state, validate_policy_gate_final_state,
            validate_scheduled_mining_final_state, validate_worker_failure_recovery_final_state,
        },
        verify_replay,
    };

    #[test]
    fn mining_bootstrap_initial_world_is_deterministic() {
        let first = build_mining_bootstrap_world();
        let second = build_mining_bootstrap_world();

        assert_eq!(first, second);
        assert_eq!(first.tick, Tick::ZERO);
        assert_eq!(
            first
                .workers
                .get(&MINING_BOOTSTRAP_WORKER_ID)
                .expect("worker exists")
                .position,
            Position::new(0, 0)
        );
        assert_eq!(
            first
                .resource_nodes
                .get(&MINING_BOOTSTRAP_NODE_ID)
                .expect("node exists")
                .remaining,
            MINING_BOOTSTRAP_INITIAL_NODE_IRON
        );
        assert_eq!(mining_bootstrap_stockpile_quantity(&first), Quantity::ZERO);
    }

    #[test]
    fn scenario_records_objective_accepted_event_first() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert_eq!(run.events[0].id, EventId::new(1));
        assert!(matches!(
            run.events[0].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
    }

    #[test]
    fn scenario_records_decision_emitted_after_objective_accepted() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert!(matches!(
            run.events[0].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
        assert!(matches!(
            run.events[1].kind,
            EventKind::DecisionEmitted { .. }
        ));
    }

    #[test]
    fn scenario_records_task_created_after_decision_emitted() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert!(matches!(
            run.events[1].kind,
            EventKind::DecisionEmitted { .. }
        ));
        assert!(matches!(run.events[2].kind, EventKind::TaskCreated { .. }));
    }

    #[test]
    fn scenario_records_task_assigned_after_task_created() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert!(matches!(run.events[2].kind, EventKind::TaskCreated { .. }));
        assert!(matches!(run.events[3].kind, EventKind::TaskAssigned { .. }));
    }

    #[test]
    fn scenario_records_assigned_worker_action_events_after_assignment() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert!(matches!(run.events[3].kind, EventKind::TaskAssigned { .. }));
        assert_eq!(run.events.len(), 12);
        for event in &run.events[4..] {
            assert!(matches!(
                event.kind,
                EventKind::ActionRequested { .. } | EventKind::ActionApplied { .. }
            ));
        }
    }

    #[test]
    fn assigned_worker_action_events_retain_assignment_id() {
        let run = run_mining_bootstrap().expect("scenario should run");

        for event in &run.events[4..] {
            assert_eq!(
                assignment_for_action_event(event),
                Some(MINING_BOOTSTRAP_ASSIGNMENT_ID)
            );
        }
    }

    #[test]
    fn scenario_final_state_contains_expected_storage_iron_quantity() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert_eq!(
            mining_bootstrap_stockpile_quantity(&run.final_state),
            MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON
        );
    }

    #[test]
    fn scenario_final_state_reduces_iron_node_quantity() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert_eq!(
            run.final_state
                .resource_nodes
                .get(&MINING_BOOTSTRAP_NODE_ID)
                .expect("node exists")
                .remaining,
            MINING_BOOTSTRAP_EXPECTED_NODE_IRON
        );
    }

    #[test]
    fn scenario_final_state_leaves_worker_carrying_no_resource() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert_eq!(
            run.final_state
                .workers
                .get(&MINING_BOOTSTRAP_WORKER_ID)
                .expect("worker exists")
                .carried,
            None
        );
    }

    #[test]
    fn scenario_tick_advances_by_expected_successful_actions() {
        let run = run_mining_bootstrap().expect("scenario should run");

        assert_eq!(run.final_state.tick, MINING_BOOTSTRAP_EXPECTED_FINAL_TICK);
    }

    #[test]
    fn scenario_replay_reproduces_final_state() {
        let run = run_mining_bootstrap().expect("scenario should run");

        let replayed =
            replay_events(&run.initial_state, &run.events).expect("replay should succeed");

        assert_eq!(replayed, run.final_state);
        verify_replay(&run.initial_state, &run.events, &run.final_state)
            .expect("verification should pass");
    }

    #[test]
    fn running_scenario_twice_produces_identical_events_and_final_state() {
        let first = run_mining_bootstrap().expect("first run should succeed");
        let second = run_mining_bootstrap().expect("second run should succeed");

        assert_eq!(first.initial_state, second.initial_state);
        assert_eq!(first.final_state, second.final_state);
        assert_eq!(first.events, second.events);
    }

    #[test]
    fn objective_satisfaction_helper_returns_true_for_final_state() {
        let run = run_mining_bootstrap().expect("scenario should run");
        let objective = autonomy_sim::mining_bootstrap_objective();

        assert!(objective_satisfied(
            &run.final_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID
        ));
    }

    #[test]
    fn objective_satisfaction_helper_returns_false_for_initial_state() {
        let run = run_mining_bootstrap().expect("scenario should run");
        let objective = autonomy_sim::mining_bootstrap_objective();

        assert!(!objective_satisfied(
            &run.initial_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID
        ));
    }

    #[test]
    fn scenario_final_state_validation_accepts_expected_state() {
        let run = run_mining_bootstrap().expect("scenario should run");

        validate_mining_bootstrap_final_state(&run.final_state)
            .expect("final state should validate");
    }

    #[test]
    fn failure_recording_appends_first_class_failure_event() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();

        let next = record_worker_failure(
            &state,
            &mut log,
            MINING_BOOTSTRAP_WORKER_ID,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        )
        .expect("failure injection should succeed");

        assert!(matches!(
            log.events()[0].kind,
            EventKind::FailureInjected { .. }
        ));
        assert!(matches!(
            log.events()[1].kind,
            EventKind::ActionRequested {
                action: WorkerAction::DisableWorker { .. },
                ..
            }
        ));
        assert!(matches!(
            log.events()[2].kind,
            EventKind::ActionApplied {
                action: WorkerAction::DisableWorker { .. },
                ..
            }
        ));
        assert_eq!(
            next.workers
                .get(&MINING_BOOTSTRAP_WORKER_ID)
                .expect("worker exists")
                .status,
            WorkerStatus::Disabled
        );
    }

    #[test]
    fn recovery_recording_appends_first_class_recovery_event() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        let disabled = record_worker_failure(
            &state,
            &mut log,
            MINING_BOOTSTRAP_WORKER_ID,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        )
        .expect("failure injection should succeed");

        let recovered = record_worker_recovery(
            &disabled,
            &mut log,
            MINING_BOOTSTRAP_WORKER_ID,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        )
        .expect("recovery should succeed");

        assert!(matches!(
            log.events()[3].kind,
            EventKind::RecoveryEmitted { .. }
        ));
        assert!(matches!(
            log.events()[4].kind,
            EventKind::ActionRequested {
                action: WorkerAction::RepairWorker { .. },
                ..
            }
        ));
        assert!(matches!(
            log.events()[5].kind,
            EventKind::ActionApplied {
                action: WorkerAction::RepairWorker { .. },
                ..
            }
        ));
        assert_eq!(
            recovered
                .workers
                .get(&MINING_BOOTSTRAP_WORKER_ID)
                .expect("worker exists")
                .status,
            WorkerStatus::Active
        );
    }

    #[test]
    fn attempted_action_while_disabled_records_requested_and_rejected() {
        let state = build_mining_bootstrap_world();
        let mut log = EventLog::new();
        let disabled = record_worker_failure(
            &state,
            &mut log,
            MINING_BOOTSTRAP_WORKER_ID,
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        )
        .expect("failure injection should succeed");

        let result = record_action_with_context(
            &disabled,
            &mut log,
            WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            },
            Some(MINING_BOOTSTRAP_ASSIGNMENT_ID),
        );

        assert_eq!(
            result,
            Err(SimError::WorkerDisabled(MINING_BOOTSTRAP_WORKER_ID))
        );
        assert!(matches!(
            log.events()[3].kind,
            EventKind::ActionRequested {
                action: WorkerAction::Mine { .. },
                ..
            }
        ));
        assert!(matches!(
            log.events()[4].kind,
            EventKind::ActionRejected {
                error: SimError::WorkerDisabled(_),
                ..
            }
        ));
        assert_eq!(log.events()[3].tick, disabled.tick);
        assert_eq!(log.events()[4].tick, disabled.tick);
    }

    #[test]
    fn worker_failure_scenario_replay_reproduces_final_state() {
        let run = run_worker_failure_recovery().expect("worker failure scenario should run");

        let replayed =
            replay_events(&run.initial_state, &run.events).expect("replay should succeed");

        assert_eq!(replayed, run.final_state);
        verify_replay(&run.initial_state, &run.events, &run.final_state)
            .expect("verification should pass");
    }

    #[test]
    fn worker_failure_scenario_final_state_satisfies_stockpile_objective() {
        let run = run_worker_failure_recovery().expect("worker failure scenario should run");
        let objective = mining_bootstrap_objective();

        assert!(objective_satisfied(
            &run.final_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID
        ));
        validate_worker_failure_recovery_final_state(&run.final_state)
            .expect("final state should validate");
    }

    #[test]
    fn worker_failure_scenario_contains_failure_and_recovery_events_in_expected_order() {
        let run = run_worker_failure_recovery().expect("worker failure scenario should run");

        assert_eq!(run.events.len(), 20);
        assert!(matches!(
            run.events[0].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
        assert!(matches!(
            run.events[1].kind,
            EventKind::DecisionEmitted { .. }
        ));
        assert!(matches!(run.events[2].kind, EventKind::TaskCreated { .. }));
        assert!(matches!(run.events[3].kind, EventKind::TaskAssigned { .. }));
        assert!(matches!(
            run.events[6].kind,
            EventKind::FailureInjected { .. }
        ));
        assert!(matches!(
            run.events[7].kind,
            EventKind::ActionRequested {
                action: WorkerAction::DisableWorker { .. },
                ..
            }
        ));
        assert!(matches!(
            run.events[8].kind,
            EventKind::ActionApplied {
                action: WorkerAction::DisableWorker { .. },
                ..
            }
        ));
        assert!(matches!(
            run.events[9].kind,
            EventKind::ActionRequested {
                action: WorkerAction::Mine { .. },
                ..
            }
        ));
        assert!(matches!(
            run.events[10].kind,
            EventKind::ActionRejected {
                error: SimError::WorkerDisabled(_),
                ..
            }
        ));
        assert!(matches!(
            run.events[11].kind,
            EventKind::RecoveryEmitted { .. }
        ));
        assert!(matches!(
            run.events[12].kind,
            EventKind::ActionRequested {
                action: WorkerAction::RepairWorker { .. },
                ..
            }
        ));
        assert!(matches!(
            run.events[13].kind,
            EventKind::ActionApplied {
                action: WorkerAction::RepairWorker { .. },
                ..
            }
        ));
    }

    #[test]
    fn worker_failure_scenario_actions_retain_assignment_context() {
        let run = run_worker_failure_recovery().expect("worker failure scenario should run");

        for event in &run.events[4..] {
            if matches!(
                event.kind,
                EventKind::ActionRequested { .. }
                    | EventKind::ActionApplied { .. }
                    | EventKind::ActionRejected { .. }
            ) {
                assert_eq!(
                    assignment_for_action_event(event),
                    Some(MINING_BOOTSTRAP_ASSIGNMENT_ID)
                );
            }
        }
    }

    #[test]
    fn running_worker_failure_scenario_twice_produces_identical_events_and_final_state() {
        let first = run_worker_failure_recovery().expect("first run should succeed");
        let second = run_worker_failure_recovery().expect("second run should succeed");

        assert_eq!(first.initial_state, second.initial_state);
        assert_eq!(first.final_state, second.final_state);
        assert_eq!(first.events, second.events);
    }

    #[test]
    fn policy_gate_scenario_final_state_satisfies_stockpile_objective() {
        let run = run_policy_gate().expect("policy gate scenario should run");
        let objective = mining_bootstrap_objective();

        assert!(objective_satisfied(
            &run.final_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID
        ));
        validate_policy_gate_final_state(&run.final_state).expect("final state should validate");
    }

    #[test]
    fn policy_gate_scenario_includes_policy_rejection_before_corrected_action() {
        let run = run_policy_gate().expect("policy gate scenario should run");

        assert_eq!(run.events.len(), 11);
        assert!(matches!(
            run.events[0].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
        assert!(matches!(
            run.events[1].kind,
            EventKind::DecisionEmitted { .. }
        ));
        assert!(matches!(run.events[2].kind, EventKind::TaskCreated { .. }));
        assert!(matches!(run.events[3].kind, EventKind::TaskAssigned { .. }));
        assert!(matches!(
            run.events[4].kind,
            EventKind::PolicyRejected {
                error: PolicyError::MineQuantityLimitExceeded { .. },
                ..
            }
        ));
        assert!(matches!(
            run.events[5].kind,
            EventKind::PolicyAccepted {
                action: WorkerAction::Mine {
                    quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            run.events[6].kind,
            EventKind::ActionRequested {
                action: WorkerAction::Mine {
                    quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            run.events[7].kind,
            EventKind::ActionApplied {
                action: WorkerAction::Mine {
                    quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn policy_gate_scenario_policy_and_action_events_retain_assignment_context() {
        let run = run_policy_gate().expect("policy gate scenario should run");

        for event in &run.events[4..] {
            assert_eq!(
                event.kind.assignment_id(),
                Some(MINING_BOOTSTRAP_ASSIGNMENT_ID)
            );
        }
    }

    #[test]
    fn policy_gate_scenario_replay_reproduces_final_state() {
        let run = run_policy_gate().expect("policy gate scenario should run");

        let replayed =
            replay_events(&run.initial_state, &run.events).expect("replay should succeed");

        assert_eq!(replayed, run.final_state);
        verify_replay(&run.initial_state, &run.events, &run.final_state)
            .expect("verification should pass");
    }

    #[test]
    fn running_policy_gate_scenario_twice_produces_identical_events_and_final_state() {
        let first = run_policy_gate().expect("first run should succeed");
        let second = run_policy_gate().expect("second run should succeed");

        assert_eq!(first.initial_state, second.initial_state);
        assert_eq!(first.final_state, second.final_state);
        assert_eq!(first.events, second.events);
    }

    #[test]
    fn scheduled_mining_scenario_final_state_satisfies_stockpile_objective() {
        let run = run_scheduled_mining().expect("scheduled mining scenario should run");
        let objective = mining_bootstrap_objective();

        assert!(objective_satisfied(
            &run.final_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID
        ));
        validate_scheduled_mining_final_state(&run.final_state)
            .expect("final state should validate");
    }

    #[test]
    fn scheduled_mining_scenario_records_scheduler_policy_and_action_events() {
        let run = run_scheduled_mining().expect("scheduled mining scenario should run");

        assert_eq!(run.events.len(), 14);
        assert!(matches!(
            run.events[0].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
        assert!(matches!(
            run.events[1].kind,
            EventKind::DecisionEmitted { .. }
        ));
        assert!(matches!(run.events[2].kind, EventKind::TaskCreated { .. }));
        assert!(matches!(run.events[3].kind, EventKind::TaskAssigned { .. }));
        assert!(matches!(
            run.events[4].kind,
            EventKind::SchedulerEmitted {
                assignment_id: MINING_BOOTSTRAP_ASSIGNMENT_ID,
                outcome: ScheduleOutcome::Action {
                    action: WorkerAction::Mine { .. },
                    ..
                },
            }
        ));
        assert!(matches!(
            run.events[5].kind,
            EventKind::PolicyAccepted { .. }
        ));
        assert!(matches!(
            run.events[6].kind,
            EventKind::ActionRequested {
                action: WorkerAction::Mine { .. },
                ..
            }
        ));
        assert!(matches!(
            run.events[7].kind,
            EventKind::ActionApplied {
                action: WorkerAction::Mine { .. },
                ..
            }
        ));
        assert!(matches!(run.events[8].kind, EventKind::TaskCreated { .. }));
        assert!(matches!(run.events[9].kind, EventKind::TaskAssigned { .. }));
        assert!(matches!(
            run.events[10].kind,
            EventKind::SchedulerEmitted {
                assignment_id: super::SCHEDULED_MINING_DEPOSIT_ASSIGNMENT_ID,
                outcome: ScheduleOutcome::Action {
                    action: WorkerAction::Deposit { .. },
                    ..
                },
            }
        ));
    }

    #[test]
    fn scheduled_mining_scenario_preserves_assignment_context() {
        let run = run_scheduled_mining().expect("scheduled mining scenario should run");

        for event in &run.events[4..8] {
            assert_eq!(
                event.kind.assignment_id(),
                Some(MINING_BOOTSTRAP_ASSIGNMENT_ID)
            );
        }
        for event in &run.events[10..] {
            assert_eq!(
                event.kind.assignment_id(),
                Some(super::SCHEDULED_MINING_DEPOSIT_ASSIGNMENT_ID)
            );
        }
    }

    #[test]
    fn scheduled_mining_scenario_replay_reproduces_final_state() {
        let run = run_scheduled_mining().expect("scheduled mining scenario should run");

        let replayed =
            replay_events(&run.initial_state, &run.events).expect("replay should succeed");

        assert_eq!(replayed, run.final_state);
        verify_replay(&run.initial_state, &run.events, &run.final_state)
            .expect("verification should pass");
    }

    #[test]
    fn running_scheduled_mining_scenario_twice_produces_identical_events_and_final_state() {
        let first = run_scheduled_mining().expect("first run should succeed");
        let second = run_scheduled_mining().expect("second run should succeed");

        assert_eq!(first.initial_state, second.initial_state);
        assert_eq!(first.final_state, second.final_state);
        assert_eq!(first.events, second.events);
    }

    #[test]
    fn proposal_adaptor_scenario_records_rejected_then_accepted_proposal_flow() {
        let run = run_proposal_adaptor().expect("proposal adaptor scenario should run");

        assert_eq!(run.events.len(), 19);
        assert!(matches!(
            run.events[0].kind,
            EventKind::ProposalReceived { .. }
        ));
        assert!(matches!(
            run.events[1].kind,
            EventKind::ProposalRejected { .. }
        ));
        assert!(matches!(
            run.events[2].kind,
            EventKind::ProposalReceived { .. }
        ));
        assert!(matches!(
            run.events[3].kind,
            EventKind::ProposalParsed { .. }
        ));
        assert!(matches!(
            run.events[4].kind,
            EventKind::ProposalAccepted { .. }
        ));
        assert!(matches!(
            run.events[5].kind,
            EventKind::ObjectiveAccepted { .. }
        ));
        assert!(matches!(
            run.events[6].kind,
            EventKind::DecisionEmitted { .. }
        ));
        assert!(matches!(run.events[7].kind, EventKind::TaskCreated { .. }));
        assert!(matches!(run.events[8].kind, EventKind::TaskAssigned { .. }));
    }

    #[test]
    fn rejected_proposal_path_does_not_create_executable_work() {
        let run = run_proposal_adaptor().expect("proposal adaptor scenario should run");

        assert!(!run.events[..2].iter().any(|event| matches!(
            event.kind,
            EventKind::ObjectiveAccepted { .. }
                | EventKind::TaskCreated { .. }
                | EventKind::TaskAssigned { .. }
                | EventKind::SchedulerEmitted { .. }
                | EventKind::ActionRequested { .. }
                | EventKind::ActionApplied { .. }
        )));
        assert_eq!(run.events[0].tick, Tick::ZERO);
        assert_eq!(run.events[1].tick, Tick::ZERO);
    }

    #[test]
    fn proposal_adaptor_scenario_final_state_satisfies_stockpile_objective() {
        let run = run_proposal_adaptor().expect("proposal adaptor scenario should run");
        let objective = mining_bootstrap_objective();

        assert!(objective_satisfied(
            &run.final_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID
        ));
        validate_scheduled_mining_final_state(&run.final_state)
            .expect("final state should validate");
    }

    #[test]
    fn proposal_adaptor_scenario_replay_reproduces_final_state() {
        let run = run_proposal_adaptor().expect("proposal adaptor scenario should run");

        let replayed =
            replay_events(&run.initial_state, &run.events).expect("replay should succeed");

        assert_eq!(replayed, run.final_state);
        verify_replay(&run.initial_state, &run.events, &run.final_state)
            .expect("verification should pass");
    }

    #[test]
    fn running_proposal_adaptor_scenario_twice_produces_identical_events_and_final_state() {
        let first = run_proposal_adaptor().expect("first run should succeed");
        let second = run_proposal_adaptor().expect("second run should succeed");

        assert_eq!(first.initial_state, second.initial_state);
        assert_eq!(first.final_state, second.final_state);
        assert_eq!(first.events, second.events);
    }
}
