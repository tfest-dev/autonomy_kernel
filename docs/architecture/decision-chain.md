# Decision Chain Artifacts

WP08 introduces deterministic causal graph artifacts for recorded event streams. This is extended with adaptor boundary layer extending the artifcats with proposal accepted and rejected paths. 

A causal graph artifact is an inspectable export derived from the event log. It turns recorded facts into nodes and edges that show how a proposal or objective led to decisions, tasks, assignments, scheduler output, policy outcomes, worker actions, state transitions, rejections, and replay verification.

The artifact is proof-oriented. It is not an execution mechanism, planner, scheduler, graph database, UI, or visualisation layer.

## Relationship to the Event Log

The event log remains the source of truth. Causal graph extraction reads an existing event sequence and creates a deterministic view over it.

The graph does not change events, mutate world state, or influence replay. It only exposes relationships already present in event payloads or safely adjacent event ordering.

Current graph nodes include:

- Proposal.
- Proposal accepted.
- Proposal rejected.
- Objective.
- Decision.
- Task.
- Assignment.
- Scheduler.
- Policy.
- Action.
- State transition.
- Rejection.
- Failure.
- Recovery.
- Replay verification.

Current graph edges are created from explicit IDs, assignment context, or tightly adjacent lifecycle/action pairs such as proposal accepted to objective accepted, failure to disable-worker action, and recovery to repair-worker action.

If a relationship is not explicit enough to establish safely, the exporter omits it.

## What It Proves

A causal graph artifact can help answer:

- Which objective started the chain?
- Whether a constrained proposal was accepted or rejected before becoming kernel work.
- Which decision emitted a task?
- Which task was assigned to a worker?
- Which scheduler output selected a worker action?
- Whether policy accepted or rejected the scheduled action.
- Whether an action was applied or rejected by reducer rules.
- Whether replay verification was included for the event stream.

The scheduled-mining artifact demonstrates the current chain:

```text
Objective
-> Decision
-> Task
-> Assignment
-> Scheduler decision
-> Policy decision
-> Worker action
-> State transition
-> Replay verification
```

The proposal-adaptor artifact demonstrates the additional proposal boundary:

```text
Proposal received
-> Proposal parsed
-> Proposal accepted or rejected
-> Objective
-> Decision
-> Task
-> Assignment
-> Scheduler decision
-> Policy decision
-> Worker action
-> State transition
-> Replay verification
```

## Replay Verification

Replay remains the state reconstruction mechanism:

```text
initial state + event sequence -> final state
```

The artifact can include a replay verification node and metadata when replay has been verified separately. This records that replay verification was part of the exported evidence. The graph itself does not perform replay and does not replace replay.

## Export Formats

WP08 provides two deterministic string exports:

- A human-readable text format for review.
- A line-based machine-readable format for tooling.

Both formats use stable ordering, deterministic node IDs, and no wall-clock timestamps, random IDs, machine-specific paths, or filesystem writes by default.

## Current Limitations

The graph is limited to the current event model.

It does not provide:

- Interactive graph viewing.
- Graph layout.
- Graph database storage.
- File-backed event logs.
- General causal inference.
- Planner reasoning.
- Live model behavior.
- Proposal parsing or validation replay.
- Policy replay validation.
- Scheduler replay validation.
- Production audit guarantees.

Future work may add richer exports or persisted artifacts once event schemas and replay checkpoints are more stable.