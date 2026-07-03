use std::error::Error;
use std::fmt;

use autonomy_core::{Quantity, WorkerId};

use crate::{WorkerAction, WorldState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionPolicy {
    pub min_battery_reserve: Option<Quantity>,
    pub allow_disable_worker: bool,
    pub allow_repair_worker: bool,
    pub max_mine_quantity: Option<Quantity>,
}

impl ActionPolicy {
    pub fn permissive() -> Self {
        Self::default()
    }
}

impl Default for ActionPolicy {
    fn default() -> Self {
        Self {
            min_battery_reserve: None,
            allow_disable_worker: true,
            allow_repair_worker: true,
            max_mine_quantity: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyError {
    BatteryReserveViolation {
        worker_id: WorkerId,
        current: Quantity,
        required_reserve: Quantity,
        action_cost: Quantity,
    },
    DisableWorkerNotAllowed {
        worker_id: WorkerId,
    },
    RepairWorkerNotAllowed {
        worker_id: WorkerId,
    },
    MineQuantityLimitExceeded {
        requested: Quantity,
        maximum: Quantity,
    },
    UnknownWorker {
        worker_id: WorkerId,
    },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BatteryReserveViolation {
                worker_id,
                current,
                required_reserve,
                action_cost,
            } => write!(
                f,
                "worker {} would breach battery reserve: current {}, reserve {}, action cost {}",
                worker_id.value(),
                current.value(),
                required_reserve.value(),
                action_cost.value()
            ),
            Self::DisableWorkerNotAllowed { worker_id } => {
                write!(
                    f,
                    "policy does not allow disabling worker {}",
                    worker_id.value()
                )
            }
            Self::RepairWorkerNotAllowed { worker_id } => {
                write!(
                    f,
                    "policy does not allow repairing worker {}",
                    worker_id.value()
                )
            }
            Self::MineQuantityLimitExceeded { requested, maximum } => write!(
                f,
                "mine quantity exceeds policy maximum: requested {}, maximum {}",
                requested.value(),
                maximum.value()
            ),
            Self::UnknownWorker { worker_id } => {
                write!(
                    f,
                    "policy validation found unknown worker {}",
                    worker_id.value()
                )
            }
        }
    }
}

impl Error for PolicyError {}

pub fn validate_action_policy(
    state: &WorldState,
    action: &WorkerAction,
    policy: &ActionPolicy,
) -> Result<(), PolicyError> {
    match action {
        WorkerAction::DisableWorker { worker_id } if !policy.allow_disable_worker => {
            return Err(PolicyError::DisableWorkerNotAllowed {
                worker_id: *worker_id,
            });
        }
        WorkerAction::RepairWorker { worker_id } if !policy.allow_repair_worker => {
            return Err(PolicyError::RepairWorkerNotAllowed {
                worker_id: *worker_id,
            });
        }
        WorkerAction::Mine { quantity, .. } => {
            if let Some(maximum) = policy.max_mine_quantity {
                if *quantity > maximum {
                    return Err(PolicyError::MineQuantityLimitExceeded {
                        requested: *quantity,
                        maximum,
                    });
                }
            }
        }
        _ => {}
    }

    if let Some(required_reserve) = policy.min_battery_reserve {
        if let Some((worker_id, action_cost)) = battery_cost(action) {
            let worker = state
                .workers
                .get(&worker_id)
                .ok_or(PolicyError::UnknownWorker { worker_id })?;

            let remaining = worker.battery.checked_sub(action_cost);
            if !matches!(remaining, Some(remaining) if remaining >= required_reserve) {
                return Err(PolicyError::BatteryReserveViolation {
                    worker_id,
                    current: worker.battery,
                    required_reserve,
                    action_cost,
                });
            }
        }
    }

    Ok(())
}

fn battery_cost(action: &WorkerAction) -> Option<(WorkerId, Quantity)> {
    match action {
        WorkerAction::Move { worker_id, .. }
        | WorkerAction::Mine { worker_id, .. }
        | WorkerAction::Deposit { worker_id, .. } => Some((*worker_id, Quantity::ONE)),
        WorkerAction::Recharge { .. }
        | WorkerAction::Wait { .. }
        | WorkerAction::DisableWorker { .. }
        | WorkerAction::RepairWorker { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use autonomy_core::{Position, Quantity, WorkerId};

    use super::validate_action_policy;
    use crate::{
        apply_action, build_mining_bootstrap_world, mining_bootstrap_actions, ActionPolicy,
        PolicyError, WorkerAction, MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
        MINING_BOOTSTRAP_NODE_ID, MINING_BOOTSTRAP_STORAGE_ID, MINING_BOOTSTRAP_WORKER_ID,
    };

    #[test]
    fn default_permissive_policy_allows_existing_mining_bootstrap_actions() {
        let policy = ActionPolicy::default();
        let mut state = build_mining_bootstrap_world();

        for action in mining_bootstrap_actions() {
            validate_action_policy(&state, &action, &policy).expect("policy should allow action");
            state = apply_action(&state, &action).expect("action should apply");
        }
    }

    #[test]
    fn battery_reserve_policy_rejects_move_that_would_breach_reserve() {
        let mut state = build_mining_bootstrap_world();
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .battery = Quantity::ONE;
        let policy = ActionPolicy {
            min_battery_reserve: Some(Quantity::ONE),
            ..ActionPolicy::default()
        };

        let result = validate_action_policy(
            &state,
            &WorkerAction::Move {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                to: Position::new(1, 0),
            },
            &policy,
        );

        assert_eq!(
            result,
            Err(PolicyError::BatteryReserveViolation {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                current: Quantity::ONE,
                required_reserve: Quantity::ONE,
                action_cost: Quantity::ONE,
            })
        );
    }

    #[test]
    fn battery_reserve_policy_rejects_mine_that_would_breach_reserve() {
        let mut state = build_mining_bootstrap_world();
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .battery = Quantity::ONE;
        let policy = ActionPolicy {
            min_battery_reserve: Some(Quantity::ONE),
            ..ActionPolicy::default()
        };

        let result = validate_action_policy(
            &state,
            &WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            },
            &policy,
        );

        assert!(matches!(
            result,
            Err(PolicyError::BatteryReserveViolation { .. })
        ));
    }

    #[test]
    fn battery_reserve_policy_rejects_deposit_that_would_breach_reserve() {
        let mut state = build_mining_bootstrap_world();
        state = apply_action(
            &state,
            &WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: MINING_BOOTSTRAP_EXPECTED_STORAGE_IRON,
            },
        )
        .expect("mine should apply");
        state
            .workers
            .get_mut(&MINING_BOOTSTRAP_WORKER_ID)
            .expect("worker exists")
            .battery = Quantity::ONE;
        let policy = ActionPolicy {
            min_battery_reserve: Some(Quantity::ONE),
            ..ActionPolicy::default()
        };

        let result = validate_action_policy(
            &state,
            &WorkerAction::Deposit {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                storage_id: MINING_BOOTSTRAP_STORAGE_ID,
            },
            &policy,
        );

        assert!(matches!(
            result,
            Err(PolicyError::BatteryReserveViolation { .. })
        ));
    }

    #[test]
    fn disable_worker_is_rejected_when_policy_disallows_it() {
        let state = build_mining_bootstrap_world();
        let policy = ActionPolicy {
            allow_disable_worker: false,
            ..ActionPolicy::default()
        };

        let result = validate_action_policy(
            &state,
            &WorkerAction::DisableWorker {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
            },
            &policy,
        );

        assert_eq!(
            result,
            Err(PolicyError::DisableWorkerNotAllowed {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
            })
        );
    }

    #[test]
    fn repair_worker_is_rejected_when_policy_disallows_it() {
        let state = build_mining_bootstrap_world();
        let policy = ActionPolicy {
            allow_repair_worker: false,
            ..ActionPolicy::default()
        };

        let result = validate_action_policy(
            &state,
            &WorkerAction::RepairWorker {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
            },
            &policy,
        );

        assert_eq!(
            result,
            Err(PolicyError::RepairWorkerNotAllowed {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
            })
        );
    }

    #[test]
    fn mine_above_max_quantity_is_rejected() {
        let state = build_mining_bootstrap_world();
        let policy = ActionPolicy {
            max_mine_quantity: Some(Quantity::new(10)),
            ..ActionPolicy::default()
        };

        let result = validate_action_policy(
            &state,
            &WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: Quantity::new(20),
            },
            &policy,
        );

        assert_eq!(
            result,
            Err(PolicyError::MineQuantityLimitExceeded {
                requested: Quantity::new(20),
                maximum: Quantity::new(10),
            })
        );
    }

    #[test]
    fn mine_at_max_quantity_is_accepted() {
        let state = build_mining_bootstrap_world();
        let policy = ActionPolicy {
            max_mine_quantity: Some(Quantity::new(10)),
            ..ActionPolicy::default()
        };

        validate_action_policy(
            &state,
            &WorkerAction::Mine {
                worker_id: MINING_BOOTSTRAP_WORKER_ID,
                node_id: MINING_BOOTSTRAP_NODE_ID,
                quantity: Quantity::new(10),
            },
            &policy,
        )
        .expect("mine at policy maximum should be accepted");
    }

    #[test]
    fn battery_reserve_policy_reports_unknown_worker_for_consuming_action() {
        let state = build_mining_bootstrap_world();
        let policy = ActionPolicy {
            min_battery_reserve: Some(Quantity::ONE),
            ..ActionPolicy::default()
        };
        let missing_worker = WorkerId::new(99);

        let result = validate_action_policy(
            &state,
            &WorkerAction::Move {
                worker_id: missing_worker,
                to: Position::new(1, 0),
            },
            &policy,
        );

        assert_eq!(
            result,
            Err(PolicyError::UnknownWorker {
                worker_id: missing_worker,
            })
        );
    }
}
