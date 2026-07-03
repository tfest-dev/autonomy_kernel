# Mining Bootstrap Scenario

The mining bootstrap scenario is the first deterministic end-to-end scenario for Autonomy Kernel. It demonstrates a bounded execution chain from objective to decision, task, assignment, worker actions, events, state changes, and replay verification.

## Purpose

The scenario proves that the current kernel spine can connect causal intent records to deterministic worker action execution and replayed final state.

It is intentionally small and fixed. It is not a planner, scheduler, simulation game, persistence layer, or robotics model.

## Initial World

- One miner worker with `WorkerId(1)`.
- One iron resource node with `ResourceNodeId(1)`.
- One storage depot with `StorageId(1)`.
- Miner starts at `(0, 0)`.
- Iron node is at `(1, 0)`.
- Storage is at `(0, 1)`.
- Iron node starts with `100` iron.
- Storage starts with `0` iron.
- Worker starts with `10` battery.

## Objective

The objective is to maintain an iron stockpile of at least `10` units.

The scenario uses explicit deterministic IDs:

    - `ObjectiveId(1)`
    - `DecisionId(1)`
    - `TaskId(1)`
    - `AssignmentId(1)`

## Causal Chain

The event stream records:

```text
ObjectiveAccepted
DecisionEmitted
TaskCreated
TaskAssigned
ActionRequested
ActionApplied
...
```

Each worker action event carries `AssignmentId(1)`.

## Expected Result

The fixed action sequence is:

1. Move miner to the iron node.
2. Mine `10` iron.
3. Move miner back adjacent to storage.
4. Deposit carried iron.

Expected final state:

    - Storage contains `10` iron.
    - Iron node has `90` iron remaining.
    - Worker is carrying no resource.
    - World tick is `4`.
    - Replay of the full event stream reproduces the final state.

## What This Proves

- Fixed deterministic inputs produce fixed events and final state.
- Lifecycle events preserve causal order without mutating world state.
- Assigned worker actions retain assignment context.
- State changes occur through recorded worker actions.
- Replay reconstructs the final state from the initial world and event stream.

## What This Does Not Prove

- General planning.
- General scheduling.
- Automatic task decomposition.
- Automatic worker selection.
- Persistent event storage.
- External system integration.
- Runtime distribution or fault tolerance.

