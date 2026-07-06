# BEAM Adaptor Experiment

This directory contains an optional BEAM/Elixir supervision boundary experiment for Autonomy Kernel.

The Rust crates remain the deterministic kernel. They own world state, reducer semantics, policy gates, scheduler logic, replay, event interpretation, and causal graph extraction. The BEAM adaptor does not replace or duplicate those responsibilities.

## Purpose

The adaptor demonstrates how a BEAM supervision tree can supervise a process that receives deterministic scenario-style requests and delegates supported execution to the Rust CLI. It is connective tissue for future cross-runtime orchestration experiments, not a replacement execution engine.

BEAM is useful here because OTP supervision provides well-understood process lifecycle handling and restart behaviour. This uses that property only at the adaptor boundary.

## Current Behaviour

The adaptor defines:

- An OTP application.
- A supervisor with one supervised scenario worker.
- Deterministic request and response structs.
- A controlled Rust CLI boundary module using `System.cmd/3` with argument lists.
- Supported scenario names:
  - `mining-bootstrap`
  - `worker-failure`
  - `policy-gate`
  - `scheduled-mining`
  - `proposal-adaptor`
- Supported artifact formats:
  - `text`
  - `lines`

The scenario worker returns deterministic boundary responses:

  - `:ok` with parsed Rust CLI summary fields for supported scenario runs.
  - `:unsupported_scenario` for unsupported scenario names.
  - `:unsupported_artifact_format` for unsupported artifact formats.
  - `:rust_cli_failed` for non-zero CLI exits or command invocation failure.
  - `:parse_failed` when Rust CLI stdout does not match the expected deterministic shape.

The worker preserves caller-supplied request IDs. It does not generate random IDs and does not use wall-clock time for request handling.

## Boundary Rules

The adaptor invokes Rust only through an allowlisted CLI process boundary:

```elixir
System.cmd("cargo", ["run", "-p", "autonomy-cli", "--", ...], cd: repo_root)
```

Unsupported scenario names and artifact formats are rejected before command invocation. Commands are built from fixed argument lists, not shell strings.

The adaptor does not:

  - Own Rust world state.
  - Reimplement reducers, policy gates, schedulers, proposal parsing, replay, artifacts, or event semantics.
  - Infer or reinterpret proposal text.
  - Bypass the Rust CLI.
  - Accept arbitrary command arguments.
  - Execute shell strings.
  - Start ports or NIFs.
  - Use Rustler or FFI.
  - Start networking or HTTP services.
  - Claim production distributed supervision.

Rust remains authoritative for state transitions, policy decisions, scheduler output, proposal handling, replay, and artifact generation.

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

Those paths are not currently implemented.
