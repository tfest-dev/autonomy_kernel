# Policy Gate Scenario

The policy gate scenario demonstrates deterministic action validation before reducer execution. It records a policy rejection for an oversized mining action, then records corrected bounded actions that satisfy the stockpile objective.

## Purpose

This scenario proves that kernel policy can reject a proposed worker action before it is attempted, while preserving an audit event and replay-compatible final state.

It is intentionally fixed and manual. It is not a scheduler, planner, general rule engine, persistence layer, or automatic replanning system.

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

## Policy

The scenario uses a deterministic action policy:

- Minimum battery reserve is `1`.
- Maximum mine quantity is `10`.
- Worker disable actions are not allowed.
- Worker repair actions are not allowed.

The policy is evaluated before reducer execution. A policy rejection does not emit `ActionRequested`, does not mutate world state, and does not advance the world tick.

## Rejected Action

The scenario first proposes mining `20` iron. This exceeds the configured maximum mine quantity, so the event stream records:

```text
PolicyRejected
```

No worker action is attempted for that rejected proposal.

## Corrected Allowed Actions

The scenario then supplies corrected bounded actions manually:

```text
PolicyAccepted
ActionRequested
ActionApplied
PolicyAccepted
ActionRequested
ActionApplied
```

The accepted actions mine `10` iron and deposit it into storage. Each policy and action event retains `AssignmentId(1)`.

## Expected Final State

- Storage contains `10` iron.
- Iron node has `90` iron remaining.
- Worker is carrying no resource.
- World tick is `2`.
- Replay of the full event stream reproduces the final state.

## What Replay Proves

- Policy events are replay-compatible audit facts.
- Policy rejection remains distinct from reducer rejection.
- Rejected policy events do not mutate world state.
- Applied action events remain the only state reconstruction mechanism.
- The final state is reconstructable from the initial world plus the event stream.

## What This Does Not Prove

- General constraint solving.
- Automatic replanning.
- General scheduling.
- Dynamic policy learning.
- Distributed supervision.
- Persistent event storage.
- External system integration.

