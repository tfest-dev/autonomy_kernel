use autonomy_core::{ObjectiveId, Quantity};

use crate::entity::ResourceKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Objective {
    pub id: ObjectiveId,
    pub kind: ObjectiveKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectiveKind {
    MaintainStockpile {
        resource: ResourceKind,
        minimum: Quantity,
    },
}
