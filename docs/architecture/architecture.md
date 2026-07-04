# Architecture

Autonomy Kernel is planned as a layered control substrate. Each layer has a bounded responsibility, and execution authority narrows as work moves from intent toward external action.

```text
Human Intent
-> Intent Planner
-> Constraint Validator
-> Coordination Kernel
-> Supervision Layer
-> Worker Runtime
-> World / External System
```

## Human Intent

Human intent is the operator-level request. It describes the desired outcome but does not directly mutate system state or authorise worker actions.

Intent should be recorded, normalised, and linked to later objectives and decisions.

## Intent Planner

The intent planner decomposes human intent into candidate objectives, tasks, or plans. The planner may eventually use a language model, deterministic rules, or another planning mechanism.

Planner output is advisory. It proposes possible work but does not execute actions and does not directly change the authoritative state.

## Constraint Validator

The constraint validator checks proposed objectives, tasks, and actions against schemas, policies, current state, and authority boundaries.

Invalid proposals should be rejected before they reach execution. Rejections should be represented as events with enough context for later inspection.

The first narrow action policy gate implemented for the grid-world worker model. It validates direct worker actions before reducer execution and records policy acceptance or rejection as structured events. This is not yet a general constraint engine.

## Coordination Kernel

The coordination kernel is the deterministic centre of the system. It accepts validated inputs, emits decisions, creates tasks, applies state deltas, and records events.

State changes flow through the kernel. Events are the source of truth. Runtime state should be derivable from the initial state plus the accepted event sequence.

Includes a minimal deterministic scheduler inside the current grid-world boundary. It selects the next worker action for existing tasks and assignments. It does not create objectives, decisions, tasks, or assignments.

## Supervision Layer

The supervision layer observes execution, detects failures, coordinates retries or recovery actions, and maintains worker lifecycle state.

Future supervision layers may use fault-tolerant message-passing runtimes, but V1 does not commit to a specific runtime or language ecosystem.

## Worker Runtime

The worker runtime executes bounded tasks issued by the kernel. Workers have limited authority and must report structured outcomes.

Workers do not redefine objectives, bypass policy, or directly mutate authoritative state. They request or complete actions, and the kernel decides whether resulting state changes are accepted.

## World / External System

The world or external system is the environment affected by worker action. In V1, this is a deterministic grid-world simulation. Later environments may represent real services, infrastructure, devices, or other distributed systems.

## Initial Crate Split

The initial Rust workspace separates shared deterministic primitives from the V1 proving environment:

  - `autonomy-core` contains typed identifiers, event identifiers, objective/decision/task/assignment identifiers, ticks, positions, quantities, and deterministic reducer errors.
  - `autonomy-sim` contains grid-world entities, worker status, direct worker and failure actions, action policy validation, minimal scheduler logic, objective/task/assignment data contracts, world state, the pure action reducer, and deterministic scenario construction helpers.
  - `autonomy-replay` contains the in-memory append-only event log, causal lifecycle recording helpers, scheduler decision recording, policy-aware action recording, failure/recovery recording helpers, assigned action recording flow, deterministic replay, replay verification, and scenario runners.

Future crates for audit and command-line workflows remain scaffolded but unimplemented.

Future crates for replay, audit, and command-line workflows remain scaffolded but unimplemented. 

## Separation of Reasoning and Execution

Reasoning is separated from execution by design. LLMs or other planners may propose, but they do not directly execute actions. Workers execute only bounded tasks. The kernel records decisions and state transitions as events.

This separation supports replay, auditability, and failure isolation.

Data contracts for objective, decision, task, and assignment lineage now integrated. It does not implement scheduling or planning. Direct action execution remains available for low-level reducer and replay tests. Now added fixed mining bootstrap scenario that proves end-to-end traceability through the current kernel. It uses explicit IDs, fixed positions, fixed quantities, and a fixed action sequence rather than a scheduler or planner.

Deterministic local worker failure and repair semantics now in place. This is not distributed supervision and does not introduce BEAM/Erlang/Elixir integration yet.

Deterministic action policy gates in place. Policy rejection happens before reducer execution and is distinct from reducer rejection. Scheduling, planning, and automatic replanning remain unimplemented.

Now added scheduler output for existing assignments only. Policy gates remain authoritative over scheduled actions. This is not a general planner or autonomous reasoning layer. 