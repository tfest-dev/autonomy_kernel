# BEAM Adaptor Experiment

This directory contains an optional BEAM/Elixir supervision boundary experiment for Autonomy Kernel.

The Rust crates remain the deterministic kernel. They own world state, reducer semantics, policy gates, scheduler logic, replay, event interpretation, and causal graph extraction. The BEAM adaptor does not replace or duplicate those responsibilities.

## Purpose

The adaptor demonstrates how a BEAM supervision tree could supervise a process that receives deterministic scenario-style requests. It is connective tissue for future cross-runtime orchestration experiments, not an execution engine.

BEAM is useful here because OTP supervision provides well-understood process lifecycle handling and restart behaviour. This uses that property only at the adaptor boundary.

## Current Behaviour

The adaptor defines:

- An OTP application.
- A supervisor with one supervised scenario worker.
- Deterministic request and response structs.
- Supported scenario names:
  - `scheduled-mining`
  - `proposal-adaptor`

The scenario worker returns deterministic boundary responses:

- `:accepted` for supported scenario names.
- `:unsupported_scenario` for unsupported scenario names.

The worker preserves caller-supplied request IDs. It does not generate random IDs and does not use wall-clock time for request handling.

## Boundary Rules

The adaptor does not:

- Execute Rust code.
- Start ports or NIFs.
- Use Rustler or FFI.
- Execute shell commands.
- Start networking or HTTP services.
- Own Rust world state.
- Reimplement reducers, policy gates, schedulers, proposal parsing, replay, or event semantics.
- Claim production distributed supervision.

## Running Tests

From this directory:

```bash
mix format
mix test
```

No external dependencies are required.

## Future Paths

Possible later integration paths include:

- Port-based invocation of a Rust CLI.
- A NIF/Rustler boundary with strict ownership rules.
- A message bus boundary where Rust remains the authority for state transitions.
- BEAM regional supervisor simulations for process isolation experiments.

Those paths are not implemented in WP10.
