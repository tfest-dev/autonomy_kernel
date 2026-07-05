# Simulation Scope

The V1 proving environment is a deterministic grid-world simulation. Its purpose is to test kernel behaviour, event design, replay, task assignment, worker constraints, and failure handling.

The simulation is intentionally limited. It is not the product and should not define the long-term domain of the project.

First deterministic Rust world-state skeleton introduced for the environment with: typed identifiers, ticks, positions, quantities, workers, resource nodes, storage, direct worker actions, and a pure reducer. It does not yet implement event sourcing, replay logs, schedulers, or planners. 

First deterministic scenario added for `mining-bootstrap`. It uses fixed IDs, fixed positions, fixed quantities, and a fixed worker action sequence to prove end-to-end traceability through objective, decision, task, assignment, action events, state transition, and replay.

`worker-failure` added as a deterministic local failure and recovery scenario. It injects a worker failure, records a rejected action while disabled, records repair, resumes work, and verifies replay of the full event stream.

`policy-gate` added as a deterministic validation scenario. It records an oversized mine action as `PolicyRejected`, then records corrected bounded actions through the normal policy-accepted action path and verifies replay of the full event stream.

`scheduled-mining` added as a deterministic scheduler-driven scenario. It records scheduler output for manually created mining and deposit tasks, executes scheduled actions through policy gates, and verifies replay of the full event stream.

Added deterministic causal graph artifacts derived from recorded scenario event streams. The scheduled-mining scenario is used to generate the first artifact.

`proposal-adaptor`, a deterministic scenario for the local proposal boundary, now implemented. It records a rejected constrained proposal, records an accepted constrained proposal, converts the accepted proposal into lifecycle records, then executes through the existing scheduler and policy gates.

## Included in V1

V1 includes:

    - Deterministic tick loop.
    - Grid map.
    - Resource node.
    - Storage depot.
    - Miner worker.
    - Hauler worker.
    - Battery level.
    - Structured event log.
    - Replay from event log.
    - Basic task assignment.
    - Initial objective: maintain stockpile.
    - Deterministic mining bootstrap scenario with replay verification.
    - Deterministic worker failure and local recovery scenario with replay verification.
    - Deterministic policy-gated action scenario with replay verification.
    - Minimal deterministic scheduler for existing mining and deposit assignments.
    - Deterministic scheduled-mining scenario with replay verification.
    - Deterministic causal graph artifact export for recorded event streams.

## Excluded from V1

V1 excludes:

    - LLM planning.
    - Erlang/Elixir runtime.
    - Distributed supervision.
    - Multi-region clustering.
    - Realistic robotics physics.
    - Combat.
    - Graphics-heavy UI.
    - Mars time.
    - Slime mould topology.
    - Advanced routing.
    - Dynamic hierarchy.
    - General rule engine or dynamic constraint learning.
    - General planner or automatic objective decomposition.
    - Full pathfinding or route planning.
    - Graph viewer or interactive UI.

## Initial Scenario Shape

The initial objective is to maintain a stockpile at a storage depot. A miner worker extracts resources from a resource node. A hauler worker moves resources between locations. Battery level constrains action availability.

The kernel should produce structured events for objective acceptance, task creation, task assignment, worker action, state changes, constraint violations, failures, and recovery decisions.

## Determinism Requirements

The tick loop should advance in deterministic steps. Worker action outcomes in V1 should be deterministic given the same initial state and event sequence. Any randomness introduced in later work must be controlled by explicit seed events or equivalent deterministic inputs.

## Proving Value

The grid world should provide enough complexity to expose coordination problems without introducing unnecessary domain noise. It should exercise:

    - State transition rules.
    - Task assignment.
    - Worker authority.
    - Battery constraints.
    - Policy gates.
    - Minimal scheduler output for existing assignments.
    - Causal graph extraction from event streams.
    - Failure reporting.
    - Replay.
    - Causal inspection.

Features outside that proving value should be deferred.