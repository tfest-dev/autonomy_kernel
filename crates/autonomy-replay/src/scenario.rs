use std::error::Error;
use std::fmt;

use autonomy_core::{Quantity, ResourceNodeId, SimError, StorageId, Tick, WorkerId};
use autonomy_sim::{
    build_mining_bootstrap_world, mining_bootstrap_actions, mining_bootstrap_assignment,
    mining_bootstrap_decision, mining_bootstrap_objective, mining_bootstrap_task,
    objective_satisfied, stockpile_quantity, ResourceKind, WorldState,
    MINING_BOOTSTRAP_ASSIGNMENT_ID, MINING_BOOTSTRAP_EXPECTED_FINAL_TICK,
    MINING_BOOTSTRAP_EXPECTED_NODE_IRON, MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
    MINING_BOOTSTRAP_NODE_ID, MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
};

use crate::{
    event_log::{
        record_ection_with_context, record_decision_emitted, record_objective_accepted,
        record_task_assigned, record_task_created, EventEnvelope, EventLog,
    },
    replay::ReplayError,
    verification::verify_replay,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioRun{
    pub initial_state: WorldState,
    pub final_state: WorldState,
    pub events: Vec<EventEnvelope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioError {
    ActionFailed(SimError),
    ReplayFailed(ReplayError),
    ObjectiveNotSatisfied,
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
            Self::ReplayFailed(error) => write!(f, "scenario replay failed: {error}"),
            Self::ObjectiveNotSatisfied => write!(f, "scenario objective is not satisfied"),
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
            Self::StorageQuantityMismatch { expected, actual } => write! (
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

pub fn mining_bootstrap_stockpile_quantity(state: &WorldState) -> Quantity {
    stockpile_quantity(state, MINING_BOOTSTRAP_STORAGE_ID, ResourceKind::Iron)
}

#[cfg(test)]
mod tests {
    use autonomy_core::{EventId, Position, Quantity, Tick};
    use autonomy_sim::{
        build_mining_bootstrap_world, objective_satisfied, MINING_BOOTSTRAP_ASSIGNMENT_ID,
        MINING_BOOTSTRAP_EXPECTED_FINAL_TICK, MINING_BOOTSTRAP_EXPECTED_NODE_IRON,
        MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON, MINING_BOOTSTRAP_INITIAL_NODE_IRON,
        MINING_BOOTSTRAP_NODE_ID, MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    use crate::{
        assignment_for_action_event,
        event_log::EventKind,
        replay::replay_events,
        scenario::{
            mining_bootstrap_stockpile_quantity, run_mining_bootstrap,
            validate_mining_bootstrap_final_state,
        },
        verify_replay,
    };

    #[test]
    fn mining_bootstrap_initial_worls_is_deterministic() {
        let first = build_mining_bootstrap_world();
        let second = build_mining_bootstrap_world;

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
        asser_eq!(
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
    fn objective_satisfaction_helper_returns_hals_for_initial_state() {
        let run = run_mining_bootstrap().expect("scenario should run");
        let objective = autonomy_sim::mining_bootstrap_objective();

        assert!(objective_satisfied(
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
}