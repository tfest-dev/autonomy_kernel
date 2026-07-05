# Proposal Adaptor Scenario

The `proposal-adaptor` scenario demonstrates the first deterministic boundary for LLM-style proposal input without calling a model or provider.

The scenario uses a constrained local line format:

```text
objective=maintain_stockpile
resource=iron
minimum=10
worker_id=1
resource_node_id=1
storage_id=1
mine_quantity=10
```

## Purpose

The scenario proves that untrusted proposal text can be recorded, parsed, validated, accepted or rejected, and then kept separate from execution authority.

Rejected proposals do not create objectives, tasks, assignments, scheduler outputs, worker actions, or state transitions. Accepted proposals are converted into explicit kernel lifecycle records before any scheduler or policy-gated action execution occurs.

## Initial World

The scenario uses the same deterministic grid-world shape as the scheduled mining path:

- One active miner worker: `WorkerId(1)`.
- One iron resource node: `ResourceNodeId(1)`.
- One storage depot: `StorageId(1)`.
- Fixed positions, quantities, battery level, and identifiers.

## Rejected Proposal Path

The scenario first records an invalid proposal that requests an unsupported resource. The event log records:

```text
ProposalReceived
ProposalRejected
```

No executable work is created from this rejected input.

## Accepted Proposal Path

The scenario then records a valid proposal. The event log records:

```text
ProposalReceived
ProposalParsed
ProposalAccepted
ObjectiveAccepted
DecisionEmitted
TaskCreated
TaskAssigned
SchedulerEmitted
PolicyAccepted
ActionRequested
ActionApplied
```

The accepted proposal is converted into one mining task and one deposit task using explicit deterministic IDs. Scheduler and policy gates remain downstream authority boundaries.

## Expected Result

The final world state contains the requested iron stockpile in storage, the resource node is reduced by the mined quantity, the worker is carrying no resource, and replay reproduces the final state from the initial state plus event stream.

## What This Proves

- Proposal parsing is deterministic and strict.
- Proposal rejection is distinct from policy rejection and action rejection.
- Rejected proposals do not mutate state or create executable work.
- Accepted proposals create lifecycle records, not direct actions.
- Replay treats proposal events as non-mutating audit facts.

## What This Does Not Prove

- No live LLM call is made.
- No prompt template, provider routing, token accounting, or HTTP client is implemented.
- No general natural-language planner is implemented.
- No automatic task decomposition beyond explicit conversion from the constrained proposal format is implemented.
- No persistence, UI, networking, or distributed supervision is implemented.