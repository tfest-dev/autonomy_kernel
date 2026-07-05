use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use autonomy_core::{
    AssignmentId, DecisionId, ObjectiveId, Quantity, ResourceNodeId, StorageId, TaskId, WorkerId,
};

use crate::{
    Assignment, Decision, DecisionKind, Objective, ObjectiveKind, ResourceKind, Task, TaskKind,
    WorldState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalText(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedProposal {
    pub objective: ProposedObjective,
    pub worker_id: WorkerId,
    pub resource_node_id: ResourceNodeId,
    pub storage_id: StorageId,
    pub mine_quantity: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposedObjective {
    MaintainStockpile {
        resource: ResourceKind,
        minimum: Quantity,
    },
}

impl ProposedObjective {
    pub fn resource(&self) -> ResourceKind {
        match self {
            Self::MaintainStockpile { resource, .. } => *resource,
        }
    }

    pub fn minimum(&self) -> Quantity {
        match self {
            Self::MaintainStockpile { minimum, .. } => *minimum,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalError {
    EmptyInput,
    MalformedLine(String),
    UnknownKey(String),
    DuplicateKey(String),
    MissingKey(&'static str),
    InvalidInteger { key: &'static str, value: String },
    UnsupportedObjective(String),
    UnsupportedResource(String),
    InvalidQuantity { key: &'static str },
}

impl fmt::Display for ProposalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "proposal input is empty"),
            Self::MalformedLine(line) => write!(f, "proposal line is malformed: {line}"),
            Self::UnknownKey(key) => write!(f, "proposal contains unknown key: {key}"),
            Self::DuplicateKey(key) => write!(f, "proposal contains duplicate key: {key}"),
            Self::MissingKey(key) => write!(f, "proposal is missing required key: {key}"),
            Self::InvalidInteger { key, value } => {
                write!(f, "proposal key {key} has invalid integer value: {value}")
            }
            Self::UnsupportedObjective(value) => {
                write!(f, "proposal objective is unsupported: {value}")
            }
            Self::UnsupportedResource(value) => {
                write!(f, "proposal resource is unsupported: {value}")
            }
            Self::InvalidQuantity { key } => {
                write!(f, "proposal key {key} must be greater than zero")
            }
        }
    }
}

impl Error for ProposalError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalValidationError {
    UnknownWorker {
        worker_id: WorkerId,
    },
    UnknownResourceNode {
        node_id: ResourceNodeId,
    },
    UnknownStorage {
        storage_id: StorageId,
    },
    ResourceMismatch {
        expected: ResourceKind,
        actual: ResourceKind,
    },
    InvalidQuantity {
        key: &'static str,
    },
    InsufficientResource {
        node_id: ResourceNodeId,
        requested: Quantity,
        remaining: Quantity,
    },
}

impl fmt::Display for ProposalValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWorker { worker_id } => {
                write!(
                    f,
                    "proposal references unknown worker {}",
                    worker_id.value()
                )
            }
            Self::UnknownResourceNode { node_id } => write!(
                f,
                "proposal references unknown resource node {}",
                node_id.value()
            ),
            Self::UnknownStorage { storage_id } => write!(
                f,
                "proposal references unknown storage {}",
                storage_id.value()
            ),
            Self::ResourceMismatch { expected, actual } => write!(
                f,
                "proposal resource mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::InvalidQuantity { key } => {
                write!(f, "proposal key {key} must be greater than zero")
            }
            Self::InsufficientResource {
                node_id,
                requested,
                remaining,
            } => write!(
                f,
                "proposal requests {} from node {} with {} remaining",
                requested.value(),
                node_id.value(),
                remaining.value()
            ),
        }
    }
}

impl Error for ProposalValidationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalRejection {
    Parse(ProposalError),
    Validation(ProposalValidationError),
}

impl fmt::Display for ProposalRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "proposal parse rejected: {error}"),
            Self::Validation(error) => write!(f, "proposal validation rejected: {error}"),
        }
    }
}

impl Error for ProposalRejection {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProposalPlanIds {
    pub objective_id: ObjectiveId,
    pub decision_id: DecisionId,
    pub mine_task_id: TaskId,
    pub mine_assignment_id: AssignmentId,
    pub deposit_task_id: TaskId,
    pub deposit_assignment_id: AssignmentId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalPlan {
    pub objective: Objective,
    pub decision: Decision,
    pub mine_task: Task,
    pub mine_assignment: Assignment,
    pub deposit_task: Task,
    pub deposit_assignment: Assignment,
}

pub fn parse_proposal_text(input: &str) -> Result<ParsedProposal, ProposalError> {
    if input.trim().is_empty() {
        return Err(ProposalError::EmptyInput);
    }

    let mut values = BTreeMap::new();
    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            return Err(ProposalError::MalformedLine(raw_line.to_string()));
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(ProposalError::MalformedLine(raw_line.to_string()));
        };

        let key = raw_key.trim();
        let value = raw_value.trim();
        let canonical_key =
            canonical_key(key).ok_or_else(|| ProposalError::UnknownKey(key.to_string()))?;

        if values.insert(canonical_key, value.to_string()).is_some() {
            return Err(ProposalError::DuplicateKey(canonical_key.to_string()));
        }
    }

    let objective_value = required_value(&values, "objective")?;
    if objective_value != "maintain_stockpile" {
        return Err(ProposalError::UnsupportedObjective(
            objective_value.to_string(),
        ));
    }

    let resource = parse_resource(required_value(&values, "resource")?)?;
    let minimum = parse_quantity(&values, "minimum")?;
    let worker_id = WorkerId::new(parse_u64(&values, "worker_id")?);
    let resource_node_id = ResourceNodeId::new(parse_u64(&values, "resource_node_id")?);
    let storage_id = StorageId::new(parse_u64(&values, "storage_id")?);
    let mine_quantity = parse_quantity(&values, "mine_quantity")?;

    Ok(ParsedProposal {
        objective: ProposedObjective::MaintainStockpile { resource, minimum },
        worker_id,
        resource_node_id,
        storage_id,
        mine_quantity,
    })
}

pub fn validate_proposal_against_world(
    proposal: &ParsedProposal,
    state: &WorldState,
) -> Result<(), ProposalValidationError> {
    if !state.workers.contains_key(&proposal.worker_id) {
        return Err(ProposalValidationError::UnknownWorker {
            worker_id: proposal.worker_id,
        });
    }

    let node = state.resource_nodes.get(&proposal.resource_node_id).ok_or(
        ProposalValidationError::UnknownResourceNode {
            node_id: proposal.resource_node_id,
        },
    )?;

    if !state.storage.contains_key(&proposal.storage_id) {
        return Err(ProposalValidationError::UnknownStorage {
            storage_id: proposal.storage_id,
        });
    }

    let expected_resource = proposal.objective.resource();
    if node.kind != expected_resource {
        return Err(ProposalValidationError::ResourceMismatch {
            expected: expected_resource,
            actual: node.kind,
        });
    }

    if proposal.objective.minimum() == Quantity::ZERO {
        return Err(ProposalValidationError::InvalidQuantity { key: "minimum" });
    }

    if proposal.mine_quantity == Quantity::ZERO {
        return Err(ProposalValidationError::InvalidQuantity {
            key: "mine_quantity",
        });
    }

    if node.remaining < proposal.mine_quantity {
        return Err(ProposalValidationError::InsufficientResource {
            node_id: proposal.resource_node_id,
            requested: proposal.mine_quantity,
            remaining: node.remaining,
        });
    }

    Ok(())
}

pub fn accepted_proposal_to_plan(proposal: &ParsedProposal, ids: ProposalPlanIds) -> ProposalPlan {
    let resource = proposal.objective.resource();
    let minimum = proposal.objective.minimum();

    ProposalPlan {
        objective: Objective {
            id: ids.objective_id,
            kind: ObjectiveKind::MaintainStockpile { resource, minimum },
        },
        decision: Decision {
            id: ids.decision_id,
            objective_id: ids.objective_id,
            kind: DecisionKind::CreateTask {
                task_id: ids.mine_task_id,
            },
        },
        mine_task: Task {
            id: ids.mine_task_id,
            objective_id: ids.objective_id,
            decision_id: Some(ids.decision_id),
            kind: TaskKind::MineResource {
                resource,
                quantity: proposal.mine_quantity,
                node_id: proposal.resource_node_id,
            },
        },
        mine_assignment: Assignment {
            id: ids.mine_assignment_id,
            task_id: ids.mine_task_id,
            worker_id: proposal.worker_id,
        },
        deposit_task: Task {
            id: ids.deposit_task_id,
            objective_id: ids.objective_id,
            decision_id: Some(ids.decision_id),
            kind: TaskKind::DepositResource {
                storage_id: proposal.storage_id,
            },
        },
        deposit_assignment: Assignment {
            id: ids.deposit_assignment_id,
            task_id: ids.deposit_task_id,
            worker_id: proposal.worker_id,
        },
    }
}

fn canonical_key(key: &str) -> Option<&'static str> {
    match key {
        "objective" => Some("objective"),
        "resource" => Some("resource"),
        "minimum" => Some("minimum"),
        "worker_id" => Some("worker_id"),
        "resource_node_id" => Some("resource_node_id"),
        "storage_id" => Some("storage_id"),
        "mine_quantity" => Some("mine_quantity"),
        _ => None,
    }
}

fn required_value<'a>(
    values: &'a BTreeMap<&'static str, String>,
    key: &'static str,
) -> Result<&'a str, ProposalError> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or(ProposalError::MissingKey(key))
}

fn parse_resource(value: &str) -> Result<ResourceKind, ProposalError> {
    match value {
        "iron" => Ok(ResourceKind::Iron),
        _ => Err(ProposalError::UnsupportedResource(value.to_string())),
    }
}

fn parse_quantity(
    values: &BTreeMap<&'static str, String>,
    key: &'static str,
) -> Result<Quantity, ProposalError> {
    let value = parse_u64(values, key)?;
    if value == 0 {
        return Err(ProposalError::InvalidQuantity { key });
    }

    Ok(Quantity::new(value))
}

fn parse_u64(
    values: &BTreeMap<&'static str, String>,
    key: &'static str,
) -> Result<u64, ProposalError> {
    let value = required_value(values, key)?;
    value
        .parse::<u64>()
        .map_err(|_| ProposalError::InvalidInteger {
            key,
            value: value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use autonomy_core::{
        AssignmentId, DecisionId, ObjectiveId, Quantity, ResourceNodeId, StorageId, TaskId,
        WorkerId,
    };

    use crate::{
        accepted_proposal_to_plan, build_mining_bootstrap_world, parse_proposal_text,
        validate_proposal_against_world, ProposalError, ProposalPlanIds, ProposalValidationError,
        ProposedObjective, ResourceKind, TaskKind, MINING_BOOTSTRAP_NODE_ID,
        MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    fn valid_text() -> &'static str {
        "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10"
    }

    #[test]
    fn empty_proposal_input_is_rejected() {
        assert_eq!(parse_proposal_text(""), Err(ProposalError::EmptyInput));
        assert_eq!(parse_proposal_text("  "), Err(ProposalError::EmptyInput));
    }

    #[test]
    fn unknown_key_is_rejected() {
        let input = format!("{}\nextra=value", valid_text());
        assert_eq!(
            parse_proposal_text(&input),
            Err(ProposalError::UnknownKey("extra".to_string()))
        );
    }

    #[test]
    fn duplicate_key_is_rejected() {
        let input = format!("{}\nresource=iron", valid_text());
        assert_eq!(
            parse_proposal_text(&input),
            Err(ProposalError::DuplicateKey("resource".to_string()))
        );
    }

    #[test]
    fn missing_required_key_is_rejected() {
        let input = "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1";
        assert_eq!(
            parse_proposal_text(input),
            Err(ProposalError::MissingKey("mine_quantity"))
        );
    }

    #[test]
    fn invalid_integer_is_rejected() {
        let input = "objective=maintain_stockpile\nresource=iron\nminimum=10\nworker_id=abc\nresource_node_id=1\nstorage_id=1\nmine_quantity=10";
        assert_eq!(
            parse_proposal_text(input),
            Err(ProposalError::InvalidInteger {
                key: "worker_id",
                value: "abc".to_string(),
            })
        );
    }

    #[test]
    fn unsupported_objective_is_rejected() {
        let input = "objective=expand_base\nresource=iron\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10";
        assert_eq!(
            parse_proposal_text(input),
            Err(ProposalError::UnsupportedObjective(
                "expand_base".to_string()
            ))
        );
    }

    #[test]
    fn unsupported_resource_is_rejected() {
        let input = "objective=maintain_stockpile\nresource=copper\nminimum=10\nworker_id=1\nresource_node_id=1\nstorage_id=1\nmine_quantity=10";
        assert_eq!(
            parse_proposal_text(input),
            Err(ProposalError::UnsupportedResource("copper".to_string()))
        );
    }

    #[test]
    fn valid_proposal_parses_deterministically() {
        let first = parse_proposal_text(valid_text()).expect("proposal should parse");
        let second = parse_proposal_text(valid_text()).expect("proposal should parse");

        assert_eq!(first, second);
        assert_eq!(first.worker_id, MINING_BOOTSTRAP_WORKER_ID);
        assert_eq!(first.resource_node_id, MINING_BOOTSTRAP_NODE_ID);
        assert_eq!(first.storage_id, MINING_BOOTSTRAP_STORAGE_ID);
        assert_eq!(first.mine_quantity, Quantity::new(10));
        assert_eq!(
            first.objective,
            ProposedObjective::MaintainStockpile {
                resource: ResourceKind::Iron,
                minimum: Quantity::new(10),
            }
        );
    }

    #[test]
    fn valid_proposal_validates_against_matching_world() {
        let state = build_mining_bootstrap_world();
        let proposal = parse_proposal_text(valid_text()).expect("proposal should parse");

        validate_proposal_against_world(&proposal, &state).expect("proposal should validate");
    }

    #[test]
    fn validation_rejects_unknown_worker() {
        let state = build_mining_bootstrap_world();
        let mut proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        proposal.worker_id = WorkerId::new(99);

        assert_eq!(
            validate_proposal_against_world(&proposal, &state),
            Err(ProposalValidationError::UnknownWorker {
                worker_id: WorkerId::new(99),
            })
        );
    }

    #[test]
    fn validation_rejects_unknown_resource_node() {
        let state = build_mining_bootstrap_world();
        let mut proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        proposal.resource_node_id = ResourceNodeId::new(99);

        assert_eq!(
            validate_proposal_against_world(&proposal, &state),
            Err(ProposalValidationError::UnknownResourceNode {
                node_id: ResourceNodeId::new(99),
            })
        );
    }

    #[test]
    fn validation_rejects_unknown_storage() {
        let state = build_mining_bootstrap_world();
        let mut proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        proposal.storage_id = StorageId::new(99);

        assert_eq!(
            validate_proposal_against_world(&proposal, &state),
            Err(ProposalValidationError::UnknownStorage {
                storage_id: StorageId::new(99),
            })
        );
    }

    #[test]
    fn validation_rejects_zero_quantity_or_minimum() {
        let state = build_mining_bootstrap_world();
        let mut proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        proposal.mine_quantity = Quantity::ZERO;

        assert_eq!(
            validate_proposal_against_world(&proposal, &state),
            Err(ProposalValidationError::InvalidQuantity {
                key: "mine_quantity",
            })
        );

        let mut proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        proposal.objective = ProposedObjective::MaintainStockpile {
            resource: ResourceKind::Iron,
            minimum: Quantity::ZERO,
        };

        assert_eq!(
            validate_proposal_against_world(&proposal, &state),
            Err(ProposalValidationError::InvalidQuantity { key: "minimum" })
        );
    }

    #[test]
    fn validation_rejects_mine_quantity_above_node_remaining() {
        let state = build_mining_bootstrap_world();
        let mut proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        proposal.mine_quantity = Quantity::new(101);

        assert_eq!(
            validate_proposal_against_world(&proposal, &state),
            Err(ProposalValidationError::InsufficientResource {
                node_id: MINING_BOOTSTRAP_NODE_ID,
                requested: Quantity::new(101),
                remaining: Quantity::new(100),
            })
        );
    }

    #[test]
    fn accepted_proposal_conversion_uses_explicit_ids_deterministically() {
        let proposal = parse_proposal_text(valid_text()).expect("proposal should parse");
        let ids = ProposalPlanIds {
            objective_id: ObjectiveId::new(11),
            decision_id: DecisionId::new(12),
            mine_task_id: TaskId::new(13),
            mine_assignment_id: AssignmentId::new(14),
            deposit_task_id: TaskId::new(15),
            deposit_assignment_id: AssignmentId::new(16),
        };

        let first = accepted_proposal_to_plan(&proposal, ids);
        let second = accepted_proposal_to_plan(&proposal, ids);

        assert_eq!(first, second);
        assert_eq!(first.objective.id, ObjectiveId::new(11));
        assert_eq!(first.decision.id, DecisionId::new(12));
        assert_eq!(first.mine_task.id, TaskId::new(13));
        assert_eq!(first.mine_assignment.id, AssignmentId::new(14));
        assert_eq!(first.deposit_task.id, TaskId::new(15));
        assert_eq!(first.deposit_assignment.id, AssignmentId::new(16));
        assert!(matches!(
            first.mine_task.kind,
            TaskKind::MineResource {
                resource: ResourceKind::Iron,
                quantity: Quantity(10),
                node_id: ResourceNodeId(1),
            }
        ));
        assert!(matches!(
            first.deposit_task.kind,
            TaskKind::DepositResource {
                storage_id: StorageId(1),
            }
        ));
    }
}
