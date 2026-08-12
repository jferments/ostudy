# ADR 0029: Lifecycle, time, and resource observation

- **Status:** Accepted
- **Date:** 2026-08-11
- **Scope:** Cross-platform preparation phases P0–P2

## Context

Desktop close interception currently protects active PDF work, calendar code discovers time-zone identity from Unix files, and document worker planning reads CPU/RAM directly through sysinfo. Mobile platforms can suspend or terminate a process without a window-close event and provide additional low-memory, thermal, battery, and foreground signals.

## Decision

### Lifecycle

A `LifecycleCoordinator` publishes ordered snapshots and events such as foreground, background, suspending, resuming, termination requested, and terminating. Native and frontend services may use those events to flush or checkpoint work.

Lifecycle notification is best effort. Correctness must survive abrupt termination without receiving an event. Durable drafts, managed-file ledgers, document checkpoints, and idempotent operations remain the safety boundary.

### Time

`Clock` supplies UTC milliseconds. `TimeZoneProvider` supplies a stable current zone identifier and offset snapshot. Calendar-sensitive commands capture one operation context and pass it through subordinate calculations. FSRS due instants remain UTC; local study-day, burial, rescheduling, and analytics semantics remain shared Rust behavior.

Direct operating-system clock and zone discovery is confined to adapters. Tests use deterministic fakes.

### Resources

`ResourceProbe` supplies a snapshot containing logical CPUs, optional available memory, low-memory state, thermal state, battery-saver/charging facts when known, and foreground state. Existing worker and queue calculations remain pure Rust policy over that snapshot.

Unknown information produces conservative behavior. Platform observations may lower concurrency but may not bypass task, memory, or package safety bounds.

## Consequences

Desktop behavior can remain unchanged while Android and iOS later provide richer lifecycle/resource signals. Calendar tests no longer depend on the host machine, and document planning can be tested independently from sysinfo.

## Non-goals

This decision does not promise continuous mobile background execution or add Android foreground services/WorkManager/iOS background tasks.

## Validation

Tests cover event ordering, lifecycle-triggered flush requests, deterministic DST/calendar behavior, unknown-zone handling, CPU/memory/thermal planning, and Linux equivalence under nominal snapshots.
