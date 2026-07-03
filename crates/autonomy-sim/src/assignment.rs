use autonomy_core::{AssignmentId, DecisionId, ObjectiveId, TaskId, WorkerId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub id: AssignmentId,
    pub task_id: TaskId,
    pub worker_id: WorkerId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CausalParent {
    Objective(ObjectiveId),
    Decision(DecisionId),
    Task(TaskId),
    Assignment(AssignmentId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionContext {
    pub assignment_id: Option<AssignmentId>,
}

impl ActionContext {
    pub const DIRECT: Self = Self {
        assignment_id: None,
    };

    pub const fn for_assignment(assignment_id: AssignmentId) -> Self {
        Self {
            assignment_id: Some(assignment_id),
        }
    }
}
