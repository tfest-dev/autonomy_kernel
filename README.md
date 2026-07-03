# Autonomy Kernel

Autonomy Kernel is a deterministic control substrate for AI-operated distributed systems.

It allows high-level intent to be decomposed into bounded, traceable, semi-autonomous execution while preserving repeatability, auditability, failure isolation, and constraint enforcement.

Status: initial Rust workspace with deterministic world-state, in-memory action event log, and replay skeleton. Scheduling, planning, persistence, and integration have not started

## Project Purpose

Autonomy Kernel is intended to provide a controlled execution substrate between high-level intent and distributed worker action. Its purpose is to make semi-autonomous operation inspectable, repeatable, and constrained by explicit state transitions rather than implicit model output.

The project is not an agent framework, robotics stack, or simulation game. It is a planned infrastructure layer for coordinating bounded work under deterministic control.

## Core Principle

LLMs may propose. 
The kernel decides. 
Workers execute. 
Events prove what happened.

Reasoning and execution are separate concerns. Proposals may originate from a planner, model, operator, or deterministic rule, but state changes must flow through the kernel and be represented by durable events.

## Problem Statement

Distributed autonomous systems need more than high-level goals and tool access. They need mechanisms that make execution bounded, observable, and recoverable when plans fail or constraints are violated.

Autonomy Kernel is designed around the premise that any system with semi-autonomous workers should be able to answer:

- What objective was accepted?
- Which constraints were applied?
- Why was a decision emitted?
- Which worker was authorsed to act?
- What changed in system state?
- Can the outcome be replayed from recorded events?

## Architecture Summary

The planned architecture separates intent, validation, coordination, supervision, worker execution, and external state:

```text
Human Intent
-> Intent Planner
-> Constraint Validator
-> Coordination Kernel
-> Supervision Layer
-> Worker Runtime
-> World / External System
```

The coordination kernel is planned as the deterministic centre of the system. Events are the source of truth. Workers receive bounded tasks and report structured outcomes. Constraint validation and supervision limit what may be attempted and how failures are handled.

## First Proving Environment

The first proving environment is a deterministic grid-world simulation. It exists to test kernel behaviour, event design, replay, task assignment, worker failure handling, and constraint enforcement in a controlled setting.

The initial objective is to maintain a resource stockpile using a small set of worker roles and explicit constraints. The simulation is a proving environment, not the product.

## Design Goals

- Deterministic execution for comparable runs.
- Append-only event history suitable for causal inspection.
- Replay from event log and initial state.
- Explicit constraints before worker action.
- Bounded worker authority.
- Failure isolation and recoverable task state.
- Clear separation between proposal, decision, execution, and evidence.

## Non-Goals

- No LLM integration in the current implementation
- No runtime clustering in the current implementation.
- No realistic robotics physics in V1.
- No graphics-heavy user interface in V1.
- No uncontrolled worker tool execution.
- No claim that the system is production-ready or generally safe.

## Repository Status

This repository currently contains the initial public documentation, Rust workspace, deterministic core primitives, minimal grid-world state/reducer types, and the first in-memory event sourcing and replay layer for direct worker actions. Schedulers, planners, persistence, distributed runtimes, and integrations have not started. 

## Roadmap Summary

1. Refine event records as objective, task, and decision models become concrete.
2. Add task assignment for miner and hauler workers. 
3. Add constraint checks beyond the initial reducer rules.
4. Add persisted event logs and replay inputs when the in-memory model stabilises.
5. Add audit views for causal inspection and failure diagnosis.
6. Add planner integration only after kernel-side authority boundaries are explicit. 