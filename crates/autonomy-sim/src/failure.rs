#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureReason {
    Injected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryKind {
    RepairWorker,
}
