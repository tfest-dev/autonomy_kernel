# Safety Model

Autonomy Kernel is planned as a safety-oriented architecture. It does not claim that a system is safe by default. Instead, it defines structural boundaries intended to make autonomous execution more constrained, inspectable, and recoverable.

## Proposal Without Execution

Models may propose plans, tasks, or actions, but they do not directly execute them. Proposed work must pass through validation and kernel decision-making before it can affect workers or state.

This separation reduces the risk that fluent output becomes implicit authority.

Now introduced the first local proposal adaptor boundary. It accepts only a constrained structured text format, parses it deterministically, validates references against the current world state, and records acceptance or rejection. It does not call a live model, provider, HTTP client, or planner.

A rejected proposal does not mutate state, advance tick, create lifecycle records, emit scheduler output, or request worker actions.

## Schemas and Policies

Actions should be constrained by schemas and policies. A proposed action that cannot be represented safely, does not match the expected schema, exceeds authority, or violates policy should be rejected before execution.

Unsafe actions should be unrepresentable where possible and rejected where representation is necessary for diagnosis.

First deterministic action policy gate implemented. The current policy can enforce a minimum battery reserve, disable or allow worker disable/repair actions, and cap mine quantity. Policy checks are pure functions of state, action, and policy. Policy rejection is recorded as `PolicyRejected` and prevents reducer execution. This is separate from `ActionRejected`, which records a reducer-level state-rule failure after an action was attempted.

Proposal rejection is an earlier boundary. `ProposalRejected` means untrusted proposal input did not become kernel work. Policy gates and reducer checks remain separate downstream boundaries.

## Bounded Worker Authority

Workers execute assigned tasks within bounded authority. They cannot redefine objectives, grant themselves additional permissions, bypass validation, or directly mutate authoritative state.

Worker outputs are reports or action results. The kernel decides whether state deltas are accepted.

The current implementation can record assignment context on worker action events. That context is causal evidence, not an implemented scheduler or permission system. It also models local worker disablement and repair. Disabled workers cannot perform normal worker actions. Failure injection and recovery are explicit events and replay-compatible, but this is not a distributed supervision system. 

Now includes a minimal scheduler that emits next worker actions for existing assignments. Scheduler output is not execution authority. Scheduled actions must still pass through policy gates before reducer execution.

Accepted proposals create objective, decision, task, and assignment records through explicit lifecycle events. They do not directly execute actions and do not bypass scheduler or policy validation.

## Layered Authority

Authority is bounded by layer:

- Human intent describes desired outcomes.
- Planners propose candidate work.
- Validators accept or reject proposals against constraints.
- The kernel emits decisions and applies state changes.
- Supervisors handle lifecycle and recovery.
- Workers execute bounded tasks and report outcomes.

No layer should silently assume the authority of another.

## Failure Isolation

Failures should be represented explicitly and isolated to the smallest practical scope. A worker failure, invalid action, depleted battery, or unreachable resource should become a structured event that can be supervised and replayed.

The system should avoid hidden partial success where possible. If partial progress affects state, it should be recorded.

Failure recovery path records injected worker failure as a first-class audit event and applies disablement through the reducer. Recovery is recorded explicitly before a repair action is applied.

Policy rejection is recorded as a first-class audit event. A policy-rejected action leaves state unchanged, does not advance tick, and does not produce an `ActionRequested` event. Similarly, scheduler output is also recorded as a first-class audit event. Scheduler events are non-mutating facts, and policy gtaes remain authoritative over scheduled actions.

The causal graph artifacts are derived from those audit events. These artifacts support inspection and diagnosis, but they do not make safety decisions, authorise actions, mutate state, or replace replay verification. 

Proposal acceptance and rejection is recorded as first-class audit events. Proposal events are replay-compatible audit facts and do not perform state reconstruction.

Optional BEAM supervision adaptor experiment added which demonstrates process restart behavior at the adaptor boundary but does not own kernel state, policy decisions, scheduler decisions, replay, event semantics, or execution. Rust remains the deterministic authority. The optional BEAM adaptor is allowed to invoke the Rust CLI through a controlled process boundary. Unsupported inputs are rejected before invocation, commands are built from fixed argument lists, and Rust remains authoritative for state, policy, scheduler, proposal, replay, and artifact semantics.

## Audit and Diagnosis

Audit and replay support diagnosis by preserving the causal chain from objective to decision to action to state change. This makes it possible to inspect whether a failure resulted from planning, validation, coordination, worker execution, environmental state, or recovery logic.

The safety model depends on durable evidence, deterministic interpretation, and explicit boundaries rather than trust in any single component.

The BEAM adaptor is not a production supervision system. It does not add clustering, networking, persistence, FFI, NIFs, ports, or distributed runtime semantics. 