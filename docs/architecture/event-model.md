# Event Model

Autonomy Kernel is planned around an append-only event model. Events describe accepted facts about objectives, validation, decisions, worker action, state changes, failures, and recovery.

Events should be deterministic, structured, and sufficient for causal inspection.

Now extended the in-memory event layer with the first causal lineage records for objectives, decisions, tasks, assignments, and assigned worker actions. Persistence, scheduling, planning, constraint validation, and full graph validation remain future work. 

## Event Properties

Planned event properties include:

    - Stable event identifier.
    - Deterministic ordering key.
    - Event category.
    - Logical tick.
    - Structured payload.
    - Causal parent references where applicable in later work.
    - Actor or layer responsible for emission in later work.
    - State hash or transition hash where applicable in later work.

The implemented envelope contains an `EventId`, a `Tick`, and an event kind. Event IDs start at `EventId(1)` in an `EventLog` and increase deterministically as events are appended.

## Implemented Lifecycle Events

Project records causal lifecycle facts using:

    - `ObjectiveAccepted`, carrying a minimal objective.
    - `DecisionEmitted`, carrying a minimal decision linked to an objective.
    - `TaskCreated`, carrying a minimal task linked to an objective and optionally a decision.
    - `TaskAssigned`, carrying an assignment that links a task to a worker.

These lifecycle events are audit and causal facts at this stage. They do not mutate world state during replay.

## Implemented Action Events

Worker action execution is recorded using:

    - `ActionRequested`, recorded at the pre-action tick.
    - `ActionApplied`, recorded at the post-action tick after the reducer succeeds.
    - `ActionRejected`, recorded at the unchanged pre-action tick after the reducer rejects the action.

Action events may carry an optional assignment reference. Direct actions without assignment context remain valid for low-level reducer and replay testing.

Rejected actions are still events. They are part of the audit history because a failed attempt can explain why state did not change.

## Planned Event Categories

The broader event model should eventually cover:

    - Objective accepted.
    - Proposal validated or rejected.
    - Decision emitted.
    - Task created.
    - Task assigned.
    - Worker action requested.
    - Worker action completed.
    - State delta applied.
    - Constraint violation.
    - Failure detected.
    - Recovery action emitted.

## Append-Only History

Events should not be edited in place. If a correction, cancellation, recovery, or superseding decision is needed, it should be represented as a new event.

This preserves the audit trail and avoids ambiguity about what the system knew at the time a decision was made.

## Deterministic Interpretation

Given the same initial state and the same accepted event sequence, the kernel should derive the same state. Event interpretation should avoid hidden dependencies on wall-clock timing, nondeterministic ordering, or external mutable state.

Where external systems are involved in later work, their observations should be captured as explicit events before they influence kernel state.

Replay applies only `ActionApplied` events to world state. Lifecycle events and `ActionRequested` records do not mutate state. `ActionRejected` verifies that the action would still be rejected and also does not mutate state.

## Causal Inspection

Events should make it possible to answer why a state transition occurred. A task assignment should be traceable to a task, a decision, a validated proposal, and an accepted objective.

This causal chain is a core part of the audit model and should be preserved across replay and failure analysis.