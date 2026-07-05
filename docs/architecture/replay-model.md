# Replay Model

Replay is a core safety primitive for Autonomy Kernel, not just a logging feature. The system is planned so that execution can be reconstructed from an initial state and an event sequence.

Replay consumes an initial `WorldState` and an in-memory sequence of event envelopes that may include objective, decision, task, assignment, and worker action events. It does not yet include persistence, scheduling, planning, causal graph export, or distributed runtime support.

Added a deterministic mining bootstrap scenario that verifies replay of a full objective-to-action event stream. The verification remains in-memory and uses fixed scenario inputs.

Deterministic failure recovery path implemented with added replay coverage for local failure and recovery. The replay path reconstructs worker disablement, rejected disabled-worker action, explicit repair, resumed work, and final objective satisfaction from the event stream. 

Replay coverage added for deterministic policy gates. Policy events are non-mutating audit facts; state reconstruction still occurs through applied worker action events. Scheduler events now also covered with replay coverage where scheduler events are non-mutating audit facts; replay does not currently re-run scheduling.

Causal graph artifacts that include replay verification metadata when verification has been performed. The graph artifact is derived from events and deos not perform state reconstruction.

Added replay coverage for proposal events. Proposal events are non-mutating audit facts; replace does not re-run proposal parsing or validation in this work packge. 

## Replay Goal

The primary replay goal is:

```text
same initial state + same event sequence -> same final state
```

If replay produces a different final state, that difference should be treated as evidence of nondeterminism, an implementation defect, incompatible schema interpretation, or an incomplete event record.

## Uses of Replay

Replay is expected to support:

    - Debugging unexpected behaviour.
    - Validating deterministic equivalence.
    - Comparing behaviour across implementation changes.
    - Reconstructing incidents after failure.
    - Inspecting causal chains from objective to worker action.
    - Verifying that recovery actions produced the intended state transition.

## Event Log as Input

Replay should consume the accepted event log and initial state. Runtime-only state, transient process memory, or unrecorded external observations should not be required to reconstruct the result.

This requirement influences event design: any observation or decision that can affect state must be recorded explicitly.

For the current direct-action model, replay treats events as reconstruction data:
    - `ObjectiveAccepted`, `DecisionEmitted`, `TaskCreated`, and `TaskAssigned` verify their event tick and do not mutate world state.
    - `ProposalReceived`, `ProposalParsed`, `ProposalAccepted`, and `ProposalRejected` verify their event tick and do not mutate world state.
    - `SchedulerEmitted` verifies its event tick and does not mutate world state.
    - `PolicyAccepted` and `PolicyRejected` verify their event tick and do not mutate world state.
    - `ActionRequested` verifies the pre-action tick and does not mutate state.
    - `ActionApplied` reapplies the contained action through the reducer and checks the resulting tick.
    - `ActionRejected` verifies that the contained action still fails and leaves state unchanged.

Malformed or inconsistent action event sequences are rejected rather than skipped. Examples include duplicate event IDs, non-monotonic event IDs, tick mismatches, applied actions that fail during replay, rejected actions that now succeed, and resulting tick mismatches.

Lifecycle events are currently causal and audit facts. Replay tolerates them as non-mutating records but does not yet validate a complete objective/task/assignment graph.

The mining bootstrap scenario verifies:

    - Lifecycle events do not mutate world state.
    - Assigned action events preserve assignment context.
    - Replayed state matches the scenario final state exactly.

The worker-failure scenario additionally verifies:

    - Failure and recovery lifecycle events do not mutate state directly.
    - Disabled-worker actions are rejected deterministically.
    - Repair is explicit and replayable.
    - Replayed state matches the recovered final state exactly.

The policy-gate scenario additionally verifies:

    - Policy rejection happens before reducer execution.
    - Policy rejection does not emit `ActionRequested`.
    - Corrected policy-accepted actions proceed through the normal action event path.
    - Replayed state matches the policy-gated final state exactly.

Replay does not re-run policy validation. Policy events are accepted audit facts, while action events remain the reconstruction mechanism.

The scheduled-mining scenario additionally verifies:

    - Scheduler output is recorded before policy validation.
    - Policy-accepted scheduled actions proceed through the normal action event path.
    - Scheduler events do not mutate world state during replay.
    - Replayed state matches the scheduler-driven final state exactly.

Replay does not re-run scheduling. Scheduler events are accepted audit facts, while action events remain the reconstruction mechanism.

The causal graph artifact additionally verifies:

    - Event streams can be transformed into deterministic inspectable proof artifacts.
    - Replay verification metadata can be included in exported evidence.
    - Graph extraction does not mutate world state.

Replay remains the only state reconstruction mechanism. Graph artifacts are exports over the event stream, not replay inputs and not execution mechanisms.

The proposal-adaptor scenario additionally verifies:

    - Rejected proposals do not create executable work.
    - Accepted proposals create lifecycle records separately from proposal acceptance.
    - Scheduler and policy gates remain downstream of accepted proposal conversion.
    - Replayed state matches the proposal-driven final state exactly.

Replay does not re-run proposal parsing or validation. Proposal events are accepted audit facts, while action events remain the reconstruction mechanism.

## State Hashing

State hashing may be introduced later to verify deterministic equivalence between live execution and replay. Hashes can provide compact checkpoints for detecting divergence, comparing runs, and validating state transitions.

Hashing is not a substitute for structured events. It is a verification aid that depends on a deterministic state representation.

## Replay Boundaries

Replay should reconstruct the kernel-visible state and decision history. It does not need to reproduce wall-clock timing, incidental logging, process scheduling, or non-authoritative worker internals unless those details affected accepted state.

The boundary between replayed state and external side effects must remain explicit.
