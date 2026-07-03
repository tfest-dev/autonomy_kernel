# Worker Failure Scenario

The worker failure scenario demonstrates deterministic local failure injection, rejected disabled-worker action, explicit recovery, resumed execution, and replay verification.

## Purpose

This scenario proves that worker failures are represented as first-class events and that recovery is explicit and repeatable. It extends the same fixed world used by the mining bootstrap scenario.

It is not a scheduler, planner, distributed supervisor, persistence layer, or runtime fault-tolerance system.

## Initial World

- One miner worker with `WorkerId(1)`.
- One iron resource node with `ResourceNodeId(1)`.
- One storage depot with `StorageId(1)`.
- Miner starts active at `(0, 0)`.
- Iron node is at `(1, 0)`.
- Storage is at `(0, 1)`.
- Iron node starts with `100` iron.
- Storage starts with `0` iron.
- Worker starts with `10` battery.

## Failure Path

The scenario records:

```text
ObjectiveAccepted
DecisionEmitted
TaskCreated
TaskAssigned
ActionRequested / ActionApplied for initial movement
FailureInjected
ActionRequested / ActionApplied for DisableWorker
ActionRequested / ActionRejected for attempted mining while disabled
RecoveryEmitted
ActionRequested / ActionApplied for RepairWorker
ActionRequested / ActionApplied for resumed mining
ActionRequested / ActionApplied for movement to storage
ActionRequested / ActionApplied for deposit
```

The failed mining attempt is recorded and rejected with the worker still disabled. The rejected action does not advance the world tick and does not mutate world state.

## Recovery

Recovery is represented explicitly:

- `RecoveryEmitted` records the recovery decision as an audit fact.
- `RepairWorker` is recorded through the normal action request/apply event path.
- Repair reactivates the worker without changing its position, battery, or carried resource.

## Expected Final State

- Storage contains `10` iron.
- Iron node has `90` iron remaining.
- Worker is active.
- Worker is carrying no resource.
- Replay of the full event stream reproduces the final state.

## What Replay Proves

- Failure and recovery lifecycle events are replay-compatible.
- Lifecycle events do not mutate world state directly.
- Disable and repair state changes occur through applied action events.
- The rejected disabled-worker action remains part of the audit history.
- The final state is reconstructable from the initial world plus the event stream.

## What This Does Not Prove

- Distributed supervision.
- Automatic recovery policy.
- General scheduling.
- General planning.
- Constraint validation beyond reducer rules.
- Persistent event storage.
- External system integration.