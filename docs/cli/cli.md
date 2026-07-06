# CLI Boundary

A minimal deterministic Rust command-line boundary for known Autonomy Kernel scenarios.

The CLI is an external process interface over existing Rust scenario and artifact APIs. It does not change kernel semantics and does not introduce networking, persistence, UI, or live model calls.

## Purpose

The CLI provides stable commands that external supervisors can invoke. It gives a narrow way to run known deterministic scenarios and export causal artifacts while keeping replay, policy, scheduler, proposal, reducer, and event authority inside Rust. It does not make the optional BEAM adaptor call the CLI. It only creates the Rust process boundary that such adaptors may use later.

Connection is now provided optionally through the BEAM adaptor to the CLI using a controlled process boundary. The adaptor allowlists scenario names and artifact formats before invoking the CLI and uses argument-list process calls rather than shell strings.

## Commands

List supported scenarios:

```bash
cargo run -p autonomy-cli -- list-scenarios
```

Run a supported scenario:

```bash
cargo run -p autonomy-cli -- run-scenario scheduled-mining
```

Export a causal artifact:

```bash
cargo run -p autonomy-cli -- export-artifact scheduled-mining text
cargo run -p autonomy-cli -- export-artifact scheduled-mining lines
```

## Supported Scenarios

The scenario list is stable:

```text
mining-bootstrap
worker-failure
policy-gate
scheduled-mining
proposal-adaptor
```

## Artifact Formats

The CLI supports:

- `text`: human-readable causal artifact export.
- `lines`: deterministic line-based export.

Artifact export uses existing causal artifact APIs. It does not write files by default.

## Output Determinism

CLI output is designed to be deterministic:

- No wall-clock timestamps.
- No random IDs.
- No machine-specific absolute paths.
- Stable scenario ordering.
- Stable summary keys and error messages.

`run-scenario` output uses this shape:

```text
scenario: scheduled-mining
status: ok
replay_verified: true
events: 14
final_tick: 2
objective_satisfied: true
```

## Exit Codes

The current exit code mapping is:

```text
0 = success
1 = invalid usage
2 = unsupported scenario
3 = scenario execution failed
4 = artifact export failed
```

Normal invalid input returns a stable error message and a non-zero exit code. The CLI should not panic for expected user errors.

## Boundaries

The CLI does not:

    - Start a server.
    - Open network connections.
    - Run asynchronously.
    - Execute arbitrary scripts.
    - Persist event logs or artifacts.
    - Provide an interactive UI.
    - Call live LLM providers.
    - Transfer kernel authority to the BEAM adaptor.
    - Invoke or modify the BEAM adaptor.
    - Implement planning or new scenario semantics.

The CLI is a process boundary over existing deterministic Rust APIs, not a general operator console.

