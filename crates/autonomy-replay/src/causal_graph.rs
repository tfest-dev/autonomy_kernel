use std::collections::BTreeMap;

use autonomy_core::{AssignmentId, DecisionId, EventId, ObjectiveId, TaskId};
use autonomy_sim::{
    Assignment, Decision, DecisionKind, Objective, ObjectiveKind, PolicyError, ResourceKind,
    ScheduleOutcome, Task, TaskKind, WorkerAction,
};

use crate::event_log::{EventEnvelope, EventKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CausalNodeId(pub u64);

impl CausalNodeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausalGraph {
    pub nodes: Vec<CausalNode>,
    pub edges: Vec<CausalEdge>,
}

impl CausalGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl Default for CausalGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausalNode {
    pub id: CausalNodeId,
    pub kind: CausalNodeKind,
    pub label: String,
    pub event_id: Option<EventId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CausalNodeKind {
    Objective,
    Decision,
    Task,
    Assignment,
    Scheduler,
    Policy,
    Action,
    StateTransition,
    Rejection,
    Failure,
    Recovery,
    ReplayVerification,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausalEdge {
    pub from: CausalNodeId,
    pub to: CausalNodeId,
    pub kind: CausalEdgeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CausalEdgeKind {
    Caused,
    Emitted,
    Assigned,
    Scheduled,
    AcceptedByPolicy,
    RejectedByPolicy,
    RequestedAction,
    AppliedAction,
    RejectedAction,
    VerifiedByReplay,
}

pub fn build_causal_graph(events: &[EventEnvelope], replay_verified: bool) -> CausalGraph {
    let mut builder = GraphBuilder::new();

    for event in events {
        match &event.kind {
            EventKind::ObjectiveAccepted { objective } => builder.add_objective(event, objective),
            EventKind::DecisionEmitted { decision } => builder.add_decision(event, decision),
            EventKind::TaskCreated { task } => builder.add_task(event, task),
            EventKind::TaskAssigned { assignment } => builder.add_assignment(event, assignment),
            EventKind::FailureInjected { worker_id, reason } => {
                let label = format!(
                    "FailureInjected worker={} reason={reason:?}",
                    worker_id.value()
                );
                let node_id = builder.add_node(CausalNodeKind::Failure, label, Some(event.id));
                builder.pending_failure = Some(PendingWorkerEvent {
                    worker_id: *worker_id,
                    node_id,
                });
                builder.last_relevant_node = Some(node_id);
            }
            EventKind::RecoveryEmitted {
                worker_id,
                recovery,
            } => {
                let label = format!(
                    "RecoveryEmitted worker={} recovery={recovery:?}",
                    worker_id.value()
                );
                let node_id = builder.add_node(CausalNodeKind::Recovery, label, Some(event.id));
                builder.pending_recovery = Some(PendingWorkerEvent {
                    worker_id: *worker_id,
                    node_id,
                });
                builder.last_relevant_node = Some(node_id);
            }
            EventKind::SchedulerEmitted {
                assignment_id,
                outcome,
            } => builder.add_scheduler(event, *assignment_id, outcome),
            EventKind::PolicyAccepted { action, context } => {
                builder.add_policy_accepted(event, action, context.assignment_id)
            }
            EventKind::PolicyRejected {
                action,
                context,
                error,
            } => builder.add_policy_rejected(event, action, context.assignment_id, error),
            EventKind::ActionRequested { action, context } => {
                builder.add_action_requested(event, action, context.assignment_id)
            }
            EventKind::ActionApplied {
                action,
                context,
                resulting_tick,
            } => builder.add_action_applied(event, action, context.assignment_id, *resulting_tick),
            EventKind::ActionRejected {
                action,
                context,
                error,
            } => builder.add_action_rejected(event, action, context.assignment_id, error),
        }
    }

    if replay_verified {
        let node_id = builder.add_node(
            CausalNodeKind::ReplayVerification,
            "ReplayVerification verified=true".to_string(),
            None,
        );
        if let Some(previous) = builder.last_relevant_node {
            builder.add_edge(previous, node_id, CausalEdgeKind::VerifiedByReplay);
        }
    }

    builder.finish()
}

pub fn export_causal_graph_text(graph: &CausalGraph) -> String {
    let mut output = String::from("# Causal Graph\n\nNodes:\n");
    for node in &graph.nodes {
        output.push_str("- n");
        output.push_str(&node.id.value().to_string());
        output.push_str(" [");
        output.push_str(node_kind_name(node.kind));
        output.push_str("] event=");
        output.push_str(&event_label(node.event_id));
        output.push_str(" label=\"");
        output.push_str(&escape_text(&node.label));
        output.push_str("\"\n");
    }

    output.push_str("\nEdges:\n");
    for edge in &graph.edges {
        output.push_str("- n");
        output.push_str(&edge.from.value().to_string());
        output.push_str(" -> n");
        output.push_str(&edge.to.value().to_string());
        output.push_str(" [");
        output.push_str(edge_kind_name(edge.kind));
        output.push_str("]\n");
    }

    output
}

pub fn export_causal_graph_lines(graph: &CausalGraph) -> String {
    let mut output = String::new();
    for node in &graph.nodes {
        output.push_str("node|");
        output.push_str(&node.id.value().to_string());
        output.push('|');
        output.push_str(node_kind_name(node.kind));
        output.push_str("|event=");
        output.push_str(&event_label(node.event_id));
        output.push('|');
        output.push_str(&escape_line_field(&node.label));
        output.push('\n');
    }

    for edge in &graph.edges {
        output.push_str("edge|");
        output.push_str(&edge.from.value().to_string());
        output.push('|');
        output.push_str(&edge.to.value().to_string());
        output.push('|');
        output.push_str(edge_kind_name(edge.kind));
        output.push('\n');
    }

    output
}

pub fn node_kind_name(kind: CausalNodeKind) -> &'static str {
    match kind {
        CausalNodeKind::Objective => "Objective",
        CausalNodeKind::Decision => "Decision",
        CausalNodeKind::Task => "Task",
        CausalNodeKind::Assignment => "Assignment",
        CausalNodeKind::Scheduler => "Scheduler",
        CausalNodeKind::Policy => "Policy",
        CausalNodeKind::Action => "Action",
        CausalNodeKind::StateTransition => "StateTransition",
        CausalNodeKind::Rejection => "Rejection",
        CausalNodeKind::Failure => "Failure",
        CausalNodeKind::Recovery => "Recovery",
        CausalNodeKind::ReplayVerification => "ReplayVerification",
    }
}

pub fn edge_kind_name(kind: CausalEdgeKind) -> &'static str {
    match kind {
        CausalEdgeKind::Caused => "Caused",
        CausalEdgeKind::Emitted => "Emitted",
        CausalEdgeKind::Assigned => "Assigned",
        CausalEdgeKind::Scheduled => "Scheduled",
        CausalEdgeKind::AcceptedByPolicy => "AcceptedByPolicy",
        CausalEdgeKind::RejectedByPolicy => "RejectedByPolicy",
        CausalEdgeKind::RequestedAction => "RequestedAction",
        CausalEdgeKind::AppliedAction => "AppliedAction",
        CausalEdgeKind::RejectedAction => "RejectedAction",
        CausalEdgeKind::VerifiedByReplay => "VerifiedByReplay",
    }
}

struct GraphBuilder {
    graph: CausalGraph,
    next_node_id: u64,
    objectives: BTreeMap<ObjectiveId, CausalNodeId>,
    decisions: BTreeMap<DecisionId, CausalNodeId>,
    decision_task_ids: BTreeMap<TaskId, CausalNodeId>,
    tasks: BTreeMap<TaskId, CausalNodeId>,
    assignments: BTreeMap<AssignmentId, CausalNodeId>,
    latest_scheduler: BTreeMap<AssignmentId, CausalNodeId>,
    latest_policy: BTreeMap<AssignmentId, CausalNodeId>,
    latest_action: BTreeMap<AssignmentId, CausalNodeId>,
    latest_action_request: Option<CausalNodeId>,
    pending_failure: Option<PendingWorkerEvent>,
    pending_recovery: Option<PendingWorkerEvent>,
    last_relevant_node: Option<CausalNodeId>,
}

impl GraphBuilder {
    fn new() -> Self {
        Self {
            graph: CausalGraph::new(),
            next_node_id: 1,
            objectives: BTreeMap::new(),
            decisions: BTreeMap::new(),
            decision_task_ids: BTreeMap::new(),
            tasks: BTreeMap::new(),
            assignments: BTreeMap::new(),
            latest_scheduler: BTreeMap::new(),
            latest_policy: BTreeMap::new(),
            latest_action: BTreeMap::new(),
            latest_action_request: None,
            pending_failure: None,
            pending_recovery: None,
            last_relevant_node: None,
        }
    }

    fn add_objective(&mut self, event: &EventEnvelope, objective: &Objective) {
        let node_id = self.add_node(
            CausalNodeKind::Objective,
            objective_label(objective),
            Some(event.id),
        );
        self.objectives.insert(objective.id, node_id);
        self.last_relevant_node = Some(node_id);
    }

    fn add_decision(&mut self, event: &EventEnvelope, decision: &Decision) {
        let node_id = self.add_node(
            CausalNodeKind::Decision,
            decision_label(decision),
            Some(event.id),
        );
        self.decisions.insert(decision.id, node_id);
        match decision.kind {
            DecisionKind::CreateTask { task_id } => {
                self.decision_task_ids.insert(task_id, node_id);
            }
        }
        if let Some(objective) = self.objectives.get(&decision.objective_id).copied() {
            self.add_edge(objective, node_id, CausalEdgeKind::Caused);
        }
        self.last_relevant_node = Some(node_id);
    }

    fn add_task(&mut self, event: &EventEnvelope, task: &Task) {
        let node_id = self.add_node(CausalNodeKind::Task, task_label(task), Some(event.id));
        self.tasks.insert(task.id, node_id);

        if let Some(decision_id) = task.decision_id {
            if let Some(decision) = self.decisions.get(&decision_id).copied() {
                self.add_edge(decision, node_id, CausalEdgeKind::Emitted);
            }
        } else if let Some(decision) = self.decision_task_ids.get(&task.id).copied() {
            self.add_edge(decision, node_id, CausalEdgeKind::Emitted);
        }

        self.last_relevant_node = Some(node_id);
    }

    fn add_assignment(&mut self, event: &EventEnvelope, assignment: &Assignment) {
        let node_id = self.add_node(
            CausalNodeKind::Assignment,
            assignment_label(assignment),
            Some(event.id),
        );
        self.assignments.insert(assignment.id, node_id);
        if let Some(task) = self.tasks.get(&assignment.task_id).copied() {
            self.add_edge(task, node_id, CausalEdgeKind::Assigned);
        }
        self.last_relevant_node = Some(node_id);
    }

    fn add_scheduler(
        &mut self,
        event: &EventEnvelope,
        assignment_id: AssignmentId,
        outcome: &ScheduleOutcome,
    ) {
        let node_id = self.add_node(
            CausalNodeKind::Scheduler,
            schedule_outcome_label(outcome),
            Some(event.id),
        );
        self.latest_scheduler.insert(assignment_id, node_id);
        if let Some(assignment) = self.assignments.get(&assignment_id).copied() {
            self.add_edge(assignment, node_id, CausalEdgeKind::Scheduled);
        }
        self.last_relevant_node = Some(node_id);
    }

    fn add_policy_accepted(
        &mut self,
        event: &EventEnvelope,
        action: &WorkerAction,
        assignment_id: Option<AssignmentId>,
    ) {
        let node_id = self.add_node(
            CausalNodeKind::Policy,
            format!("PolicyAccepted {}", action_label(action)),
            Some(event.id),
        );
        if let Some(assignment_id) = assignment_id {
            self.latest_policy.insert(assignment_id, node_id);
            if let Some(scheduler) = self.latest_scheduler.get(&assignment_id).copied() {
                self.add_edge(scheduler, node_id, CausalEdgeKind::AcceptedByPolicy);
            } else if let Some(assignment) = self.assignments.get(&assignment_id).copied() {
                self.add_edge(assignment, node_id, CausalEdgeKind::AcceptedByPolicy);
            }
        }
        self.last_relevant_node = Some(node_id);
    }

    fn add_policy_rejected(
        &mut self,
        event: &EventEnvelope,
        action: &WorkerAction,
        assignment_id: Option<AssignmentId>,
        error: &PolicyError,
    ) {
        let node_id = self.add_node(
            CausalNodeKind::Rejection,
            format!(
                "PolicyRejected {} error={}",
                action_label(action),
                policy_error_label(error)
            ),
            Some(event.id),
        );
        if let Some(assignment_id) = assignment_id {
            if let Some(scheduler) = self.latest_scheduler.get(&assignment_id).copied() {
                self.add_edge(scheduler, node_id, CausalEdgeKind::RejectedByPolicy);
            } else if let Some(assignment) = self.assignments.get(&assignment_id).copied() {
                self.add_edge(assignment, node_id, CausalEdgeKind::RejectedByPolicy);
            }
        }
        self.last_relevant_node = Some(node_id);
    }

    fn add_action_requested(
        &mut self,
        event: &EventEnvelope,
        action: &WorkerAction,
        assignment_id: Option<AssignmentId>,
    ) {
        let node_id = self.add_node(
            CausalNodeKind::Action,
            format!("ActionRequested {}", action_label(action)),
            Some(event.id),
        );

        if let Some(assignment_id) = assignment_id {
            self.latest_action.insert(assignment_id, node_id);
            if let Some(policy) = self.latest_policy.get(&assignment_id).copied() {
                self.add_edge(policy, node_id, CausalEdgeKind::RequestedAction);
            } else if let Some(assignment) = self.assignments.get(&assignment_id).copied() {
                self.add_edge(assignment, node_id, CausalEdgeKind::RequestedAction);
            }
        }

        self.latest_action_request = Some(node_id);
        self.link_pending_worker_event(action, node_id);
        self.last_relevant_node = Some(node_id);
    }

    fn add_action_applied(
        &mut self,
        event: &EventEnvelope,
        action: &WorkerAction,
        assignment_id: Option<AssignmentId>,
        resulting_tick: autonomy_core::Tick,
    ) {
        let node_id = self.add_node(
            CausalNodeKind::StateTransition,
            format!(
                "ActionApplied {} resulting_tick={}",
                action_label(action),
                resulting_tick.value()
            ),
            Some(event.id),
        );
        if let Some(action_node) = self.action_parent(assignment_id) {
            self.add_edge(action_node, node_id, CausalEdgeKind::AppliedAction);
        }
        self.last_relevant_node = Some(node_id);
    }

    fn add_action_rejected(
        &mut self,
        event: &EventEnvelope,
        action: &WorkerAction,
        assignment_id: Option<AssignmentId>,
        error: &autonomy_core::SimError,
    ) {
        let node_id = self.add_node(
            CausalNodeKind::Rejection,
            format!("ActionRejected {} error={error}", action_label(action)),
            Some(event.id),
        );
        if let Some(action_node) = self.action_parent(assignment_id) {
            self.add_edge(action_node, node_id, CausalEdgeKind::RejectedAction);
        }
        self.last_relevant_node = Some(node_id);
    }

    fn action_parent(&self, assignment_id: Option<AssignmentId>) -> Option<CausalNodeId> {
        assignment_id
            .and_then(|assignment_id| self.latest_action.get(&assignment_id).copied())
            .or(self.latest_action_request)
    }

    fn link_pending_worker_event(&mut self, action: &WorkerAction, action_node_id: CausalNodeId) {
        match action {
            WorkerAction::DisableWorker { worker_id } => {
                if let Some(pending) = self.pending_failure {
                    if pending.worker_id == *worker_id {
                        self.add_edge(pending.node_id, action_node_id, CausalEdgeKind::Emitted);
                        self.pending_failure = None;
                    }
                }
            }
            WorkerAction::RepairWorker { worker_id } => {
                if let Some(pending) = self.pending_recovery {
                    if pending.worker_id == *worker_id {
                        self.add_edge(pending.node_id, action_node_id, CausalEdgeKind::Emitted);
                        self.pending_recovery = None;
                    }
                }
            }
            _ => {}
        }
    }

    fn add_node(
        &mut self,
        kind: CausalNodeKind,
        label: String,
        event_id: Option<EventId>,
    ) -> CausalNodeId {
        let node_id = CausalNodeId::new(self.next_node_id);
        self.next_node_id += 1;
        self.graph.nodes.push(CausalNode {
            id: node_id,
            kind,
            label,
            event_id,
        });
        node_id
    }

    fn add_edge(&mut self, from: CausalNodeId, to: CausalNodeId, kind: CausalEdgeKind) {
        self.graph.edges.push(CausalEdge { from, to, kind });
    }

    fn finish(self) -> CausalGraph {
        self.graph
    }
}

#[derive(Clone, Copy)]
struct PendingWorkerEvent {
    worker_id: autonomy_core::WorkerId,
    node_id: CausalNodeId,
}

fn objective_label(objective: &Objective) -> String {
    match objective.kind {
        ObjectiveKind::MaintainStockpile { resource, minimum } => format!(
            "MaintainStockpile {} >= {} objective={}",
            resource_label(resource),
            minimum.value(),
            objective.id.value()
        ),
    }
}

fn decision_label(decision: &Decision) -> String {
    match decision.kind {
        DecisionKind::CreateTask { task_id } => format!(
            "CreateTask decision={} objective={} task={}",
            decision.id.value(),
            decision.objective_id.value(),
            task_id.value()
        ),
    }
}

fn task_label(task: &Task) -> String {
    match task.kind {
        TaskKind::MineResource {
            resource,
            quantity,
            node_id,
        } => format!(
            "MineResource task={} objective={} {} qty={} node={}",
            task.id.value(),
            task.objective_id.value(),
            resource_label(resource),
            quantity.value(),
            node_id.value()
        ),
        TaskKind::DepositResource { storage_id } => format!(
            "DepositResource task={} objective={} storage={}",
            task.id.value(),
            task.objective_id.value(),
            storage_id.value()
        ),
    }
}

fn assignment_label(assignment: &Assignment) -> String {
    format!(
        "Assignment assignment={} task={} worker={}",
        assignment.id.value(),
        assignment.task_id.value(),
        assignment.worker_id.value()
    )
}

fn schedule_outcome_label(outcome: &ScheduleOutcome) -> String {
    match outcome {
        ScheduleOutcome::Action {
            assignment_id,
            action,
        } => format!(
            "SchedulerEmitted assignment={} action={}",
            assignment_id.value(),
            action_label(action)
        ),
        ScheduleOutcome::Complete { assignment_id } => {
            format!(
                "SchedulerEmitted assignment={} complete",
                assignment_id.value()
            )
        }
        ScheduleOutcome::Blocked {
            assignment_id,
            reason,
        } => format!(
            "SchedulerEmitted assignment={} blocked={reason:?}",
            assignment_id.value()
        ),
    }
}

fn action_label(action: &WorkerAction) -> String {
    match action {
        WorkerAction::Move { worker_id, to } => {
            format!("Move worker={} to=({}, {})", worker_id.value(), to.x, to.y)
        }
        WorkerAction::Mine {
            worker_id,
            node_id,
            quantity,
        } => format!(
            "Mine worker={} node={} qty={}",
            worker_id.value(),
            node_id.value(),
            quantity.value()
        ),
        WorkerAction::Deposit {
            worker_id,
            storage_id,
        } => format!(
            "Deposit worker={} storage={}",
            worker_id.value(),
            storage_id.value()
        ),
        WorkerAction::Recharge { worker_id, amount } => format!(
            "Recharge worker={} amount={}",
            worker_id.value(),
            amount.value()
        ),
        WorkerAction::Wait { worker_id } => format!("Wait worker={}", worker_id.value()),
        WorkerAction::DisableWorker { worker_id } => {
            format!("DisableWorker worker={}", worker_id.value())
        }
        WorkerAction::RepairWorker { worker_id } => {
            format!("RepairWorker worker={}", worker_id.value())
        }
    }
}

fn policy_error_label(error: &PolicyError) -> String {
    match error {
        PolicyError::BatteryReserveViolation {
            worker_id,
            current,
            required_reserve,
            action_cost,
        } => format!(
            "BatteryReserveViolation worker={} current={} reserve={} cost={}",
            worker_id.value(),
            current.value(),
            required_reserve.value(),
            action_cost.value()
        ),
        PolicyError::DisableWorkerNotAllowed { worker_id } => {
            format!("DisableWorkerNotAllowed worker={}", worker_id.value())
        }
        PolicyError::RepairWorkerNotAllowed { worker_id } => {
            format!("RepairWorkerNotAllowed worker={}", worker_id.value())
        }
        PolicyError::MineQuantityLimitExceeded { requested, maximum } => format!(
            "MineQuantityLimitExceeded requested={} maximum={}",
            requested.value(),
            maximum.value()
        ),
        PolicyError::UnknownWorker { worker_id } => {
            format!("UnknownWorker worker={}", worker_id.value())
        }
    }
}

fn resource_label(resource: ResourceKind) -> &'static str {
    match resource {
        ResourceKind::Iron => "Iron",
    }
}

fn event_label(event_id: Option<EventId>) -> String {
    event_id
        .map(|event_id| event_id.value().to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn escape_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_line_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use crate::{
        causal_graph::{
            build_causal_graph, export_causal_graph_lines, export_causal_graph_text,
            CausalEdgeKind, CausalGraph, CausalNode, CausalNodeKind,
        },
        scenario::{run_policy_gate, run_scheduled_mining, run_worker_failure_recovery},
    };

    #[test]
    fn empty_event_stream_produces_empty_graph() {
        let graph = build_causal_graph(&[], false);

        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn scheduled_mining_events_create_core_causal_nodes() {
        let run = run_scheduled_mining().expect("scheduled-mining should run");
        let graph = build_causal_graph(&run.events, true);

        assert!(has_node_kind(&graph, CausalNodeKind::Objective));
        assert!(has_node_kind(&graph, CausalNodeKind::Decision));
        assert!(has_node_kind(&graph, CausalNodeKind::Task));
        assert!(has_node_kind(&graph, CausalNodeKind::Assignment));
        assert!(has_node_kind(&graph, CausalNodeKind::Scheduler));
        assert!(has_node_kind(&graph, CausalNodeKind::Policy));
        assert!(has_node_kind(&graph, CausalNodeKind::Action));
        assert!(has_node_kind(&graph, CausalNodeKind::StateTransition));
        assert!(has_node_kind(&graph, CausalNodeKind::ReplayVerification));
    }

    #[test]
    fn lifecycle_edges_are_created_from_explicit_ids() {
        let run = run_scheduled_mining().expect("scheduled-mining should run");
        let graph = build_causal_graph(&run.events, false);

        let objective = node_with_label(&graph, "MaintainStockpile");
        let decision = node_with_label(&graph, "CreateTask");
        let mine_task = node_with_label(&graph, "MineResource task=1");
        let mine_assignment = node_with_label(&graph, "Assignment assignment=1");

        assert_has_edge(&graph, objective, decision, CausalEdgeKind::Caused);
        assert_has_edge(&graph, decision, mine_task, CausalEdgeKind::Emitted);
        assert_has_edge(&graph, mine_task, mine_assignment, CausalEdgeKind::Assigned);
    }

    #[test]
    fn scheduled_mining_chain_links_assignment_scheduler_policy_action_and_state() {
        let run = run_scheduled_mining().expect("scheduled-mining should run");
        let graph = build_causal_graph(&run.events, false);

        let assignment = node_with_label(&graph, "Assignment assignment=1");
        let scheduler = node_with_label(&graph, "SchedulerEmitted assignment=1");
        let policy = node_with_label(&graph, "PolicyAccepted Mine");
        let action = node_with_label(&graph, "ActionRequested Mine");
        let applied = node_with_label(&graph, "ActionApplied Mine");

        assert_has_edge(&graph, assignment, scheduler, CausalEdgeKind::Scheduled);
        assert_has_edge(&graph, scheduler, policy, CausalEdgeKind::AcceptedByPolicy);
        assert_has_edge(&graph, policy, action, CausalEdgeKind::RequestedAction);
        assert_has_edge(&graph, action, applied, CausalEdgeKind::AppliedAction);
    }

    #[test]
    fn policy_rejected_event_creates_rejection_without_action_request_edge() {
        let run = run_policy_gate().expect("policy-gate should run");
        let graph = build_causal_graph(&run.events, false);

        let rejection = node_with_label(&graph, "PolicyRejected Mine");
        assert_eq!(rejection.kind, CausalNodeKind::Rejection);
        assert!(graph
            .edges
            .iter()
            .all(|edge| edge.from != rejection.id || edge.kind != CausalEdgeKind::RequestedAction));
    }

    #[test]
    fn action_rejected_event_creates_rejection_edge() {
        let run = run_worker_failure_recovery().expect("worker-failure should run");
        let graph = build_causal_graph(&run.events, false);

        let action = node_with_label(&graph, "ActionRequested Mine");
        let rejection = node_with_label(&graph, "ActionRejected Mine");

        assert_eq!(rejection.kind, CausalNodeKind::Rejection);
        assert_has_edge(&graph, action, rejection, CausalEdgeKind::RejectedAction);
    }

    #[test]
    fn failure_and_recovery_events_create_nodes_and_edges_to_actions() {
        let run = run_worker_failure_recovery().expect("worker-failure should run");
        let graph = build_causal_graph(&run.events, false);

        let failure = node_with_label(&graph, "FailureInjected");
        let disable = node_with_label(&graph, "ActionRequested DisableWorker");
        let recovery = node_with_label(&graph, "RecoveryEmitted");
        let repair = node_with_label(&graph, "ActionRequested RepairWorker");

        assert_eq!(failure.kind, CausalNodeKind::Failure);
        assert_eq!(recovery.kind, CausalNodeKind::Recovery);
        assert_has_edge(&graph, failure, disable, CausalEdgeKind::Emitted);
        assert_has_edge(&graph, recovery, repair, CausalEdgeKind::Emitted);
    }

    #[test]
    fn replay_verification_node_and_edge_are_included_when_requested() {
        let run = run_scheduled_mining().expect("scheduled-mining should run");
        let graph = build_causal_graph(&run.events, true);

        let replay = graph
            .nodes
            .iter()
            .find(|node| node.kind == CausalNodeKind::ReplayVerification)
            .expect("replay verification node exists");

        assert_eq!(replay.event_id, None);
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.to == replay.id && edge.kind == CausalEdgeKind::VerifiedByReplay));
    }

    #[test]
    fn human_readable_export_is_deterministic() {
        let run = run_scheduled_mining().expect("scheduled-mining should run");
        let first = build_causal_graph(&run.events, true);
        let second = build_causal_graph(&run.events, true);

        assert_eq!(
            export_causal_graph_text(&first),
            export_causal_graph_text(&second)
        );
        assert!(export_causal_graph_text(&first).contains("# Causal Graph"));
        assert!(export_causal_graph_text(&first).contains("[Objective]"));
        assert!(export_causal_graph_text(&first).contains("[VerifiedByReplay]"));
    }

    #[test]
    fn line_based_export_is_deterministic() {
        let run = run_scheduled_mining().expect("scheduled-mining should run");
        let first = build_causal_graph(&run.events, true);
        let second = build_causal_graph(&run.events, true);

        assert_eq!(
            export_causal_graph_lines(&first),
            export_causal_graph_lines(&second)
        );
        assert!(export_causal_graph_lines(&first).contains("node|1|Objective|event=1|"));
        assert!(export_causal_graph_lines(&first).contains("edge|"));
    }

    fn has_node_kind(graph: &CausalGraph, kind: CausalNodeKind) -> bool {
        graph.nodes.iter().any(|node| node.kind == kind)
    }

    fn node_with_label<'a>(graph: &'a CausalGraph, label: &str) -> &'a CausalNode {
        graph
            .nodes
            .iter()
            .find(|node| node.label.contains(label))
            .unwrap_or_else(|| panic!("missing node label containing {label:?}"))
    }

    fn assert_has_edge(
        graph: &CausalGraph,
        from: &CausalNode,
        to: &CausalNode,
        kind: CausalEdgeKind,
    ) {
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from == from.id && edge.to == to.id && edge.kind == kind),
            "missing edge {:?} -> {:?} [{kind:?}]",
            from.label,
            to.label
        );
    }
}
