# Scheduled Mining Scenario

The scheduled mining scenario demonstrates the first deterministic scheduler path in Autonomy Kernel. It uses existing objective, task, assignment, policy, event, reducer, and replay primitives without adding planning or automatic task creation.

## Purpose

This scenario proves that a scheduler can select the next worker action for an existing assignment, record that decision, pass the action through policy gates, and replay the resulting state transitions.

It is intentionally narrow. It is not a general planner, route planner, task generator, persistence layer, or autonomous reasoning system.

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

## Manual Structure

The scenario records a manually created objective, decision, mining task, and mining assignment:

```text
ObjectiveAccepted
DecisionEmitted
TaskCreated
TaskAssigned
```

After the scheduled mining step succeeds, the scenario manually records a deposit task and deposit assignment for the same worker. The scheduler does not create objectives, decisions, tasks, or assignments.

## Deterministic Scheduler Behaviour

For the mining task, the scheduler inspects the current worker and resource-node state. Because the worker is adjacent to the node, it emits a `Mine` action.

For the deposit task, the scheduler inspects the current worker and storage state. Because the worker is carrying iron and is adjacent to storage, it emits a `Deposit` action.

When movement is needed in scheduler tests, movement is one deterministic step toward the target, using the x-axis first and then the y-axis. The scenario itself does not require pathfinding.

## Policy Gate Relationship

Scheduled actions are not authoritative by themselves. The event stream records:

```text
SchedulerEmitted
PolicyAccepted
ActionRequested
ActionApplied
```

If policy rejects a scheduled action, `PolicyRejected` is recorded and no `ActionRequested` event is emitted. This keeps the scheduler subordinate to kernel policy.

## Expected Final State

- Storage contains `10` iron.
- Iron node has `90` iron remaining.
- Worker is carrying no resource.
- World tick is `2`.
- Replay of the full event stream reproduces the final state.

## What Replay Proves

- Scheduler events are replay-compatible audit facts.
- Scheduler events do not mutate world state.
- Policy gates remain between scheduled output and reducer execution.
- Applied action events remain the state reconstruction mechanism.
- The final state is reconstructable from the initial world plus the event stream.

## What This Does Not Prove

- General planning.
- Automatic objective decomposition.
- Automatic task generation.
- Multi-worker optimisation.
- Full pathfinding or route planning.
- Distributed supervision.
- Persistent event storage.
- External system integration.

