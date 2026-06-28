---
type: ADR
title: Drive the TUI from an async event loop (EventStream + tokio::select!)
description: Replace the blocking event::read() TEA loop with tokio::select! over crossterm EventStream, a tokio::sync::mpsc channel, and a frame heartbeat, so background network results render the instant they arrive instead of waiting for the next keypress.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, tui, async, ratatui, tokio, performance]
timestamp: 2026-06-25T12:00:00Z
---

# 0008. Drive the TUI from an async event loop (EventStream + tokio::select!)

## Context

The TUI shell ([ADR 0007](/adr/0007-tui-module-structure.md), `src/tui/mod.rs`)
runs a TEA loop whose input step is the **blocking** `crossterm::event::read()`:

```rust
loop {
    drain_channel(&rx, &mut model, ...); // process background results
    terminal.draw(...);                  // render
    let ev = event::read();              // BLOCKS until a key/mouse event
    // ...map ev -> Msg -> update -> dispatch_cmds
}
```

Network work (`Cmd::LoadTasksByProject`, `Cmd::LoadDetail`) is run on
`tokio::spawn` tasks that push their result back as a `Msg` over a
`std::sync::mpsc` channel. Because the loop is parked inside `event::read()`,
that `Msg` is **not drained or rendered until the user presses a key**. On a
cold open the data frequently arrives within a few hundred milliseconds, yet the
screen stays on "Loading…" until input — the user perceives a slow first load
("primeiro load demora") even though the fetch already completed.

This is an architecture mismatch, not a micro-optimization: S5 (cache), S5.1
(progressive render), and S5.3 (parallel aggregation) each made the *fetch*
faster, but none of them let the UI **react to the fetch finishing**. Two
tells in the existing code confirm the intent was async from the start:

- `Cargo.toml` already enables `crossterm`'s `event-stream` feature — and never
  uses it.
- `run_app_blocking` spawns a **dedicated OS thread with its own tokio runtime**
  solely to `block_on` the async `run_app` without tripping the
  nested-runtime panic — a workaround that exists only because the loop fights
  the runtime instead of living on it.

Force: **responsiveness of network-backed terminal UI** — the screen must repaint
when data arrives, not when the user happens to press a key. The HTTP client is
*not* the force here: `reqwest` (hyper-based, connection-pooled,
[ADR 0003](/adr/0003-http-transport-and-mocked-server-testing.md)) already
provides keep-alive pooling and is the right high-level async client; swapping it
would not change this behavior, because the bottleneck is the render-on-input
loop, not request latency.

## Decision

Drive `run_app` from an **async, multiplexed event loop** built on the canonical
ratatui async pattern:

| Concern | Before | After |
|---|---|---|
| Input | blocking `event::read()` | `crossterm::event::EventStream` polled with `StreamExt::next()` |
| Background results | `std::sync::mpsc` drained only after input | `tokio::sync::mpsc::unbounded_channel`, awaited as a select arm |
| Multiplexing | none (one blocking call) | `tokio::select!` over input + channel + a frame heartbeat |
| Redraw trigger | once per loop, gated on input | after any select branch (event, result, or tick) |
| Runtime | dedicated OS thread + nested runtime (`run_app_blocking`) | a single `async fn` awaited on the main runtime |

The loop shape:

```rust
let mut events = EventStream::new();
let mut heartbeat = tokio::time::interval(FRAME_PERIOD);
loop {
    terminal.draw(|f| view(&model, f))?;
    if model.should_quit { break; }
    tokio::select! {
        maybe_ev = events.next()  => { /* map -> Msg -> update -> dispatch */ }
        Some(msg) = rx.recv()     => { /* update -> dispatch */ }
        _ = heartbeat.tick()      => { /* redraw safety net / future spinner */ }
    }
}
```

Because `rx.recv()` is a first-class select arm, a background result wakes the
loop and is rendered on the very next `terminal.draw` — **no keypress required**.
The dedicated-thread/nested-runtime workaround is removed: `run_app` becomes a
plain `async fn` awaited directly from the already-async `dispatch_browse` /
`dispatch_mine`, so the entry points (`browse`, `run_mine`) become `async`.

The pure TEA core (`model.rs` `update`, `events.rs` mapping) is **unchanged** —
this ADR rewrites only the shell's I/O multiplexing.

### Dependencies

- `tokio` gains the `time` (heartbeat) and `sync` (mpsc) features.
- Add `tokio-stream` for `StreamExt::next()` over `EventStream` (lighter and more
  tokio-native than pulling in the full `futures` crate; `EventStream`
  implements `futures_core::Stream`, which `tokio_stream::StreamExt` extends).

## Alternatives considered

**(a) Minimal poll-based fix — `event::poll(timeout)` + drain + redraw.**
Keep the blocking loop but wake it ~20×/s to drain the channel. ~10 lines, low
risk, and it *does* fix the freeze. Rejected as the destination (kept as the
fallback): it is a timed busy-poll that still owns the thread, keeps the
`std::sync::mpsc` + dedicated-runtime workaround, and leaves the enabled-but-unused
`event-stream` feature dead. It treats the symptom; `select!` removes the
mismatch.

**(b) Swap the HTTP client (ureq/isahc/hyper).** Rejected: the measured
bottleneck (AC_TIMING, S5.2) is the render-on-input loop and cache reads of
0–1 ms, not the client. `reqwest` already gives connection pooling/keep-alive;
a different client cannot render a result the loop refuses to poll.

**(c) Constant high-FPS redraw (30–60 FPS) like the upstream example.**
Rejected as the default cadence: an idle task CLI should not burn CPU
redrawing a static screen 60×/s. A modest heartbeat (≈10 FPS) plus
redraw-on-activity is enough — ratatui only flushes diffs, and correctness
comes from the channel arm, not the tick.

## Consequences

**Positive:**

- Background network results render the instant they arrive — the actual fix for
  "primeiro load demora."
- One runtime, one thread: `run_app_blocking`'s nested-runtime workaround is
  deleted; the no-panic-inside-runtime property is preserved by construction.
- The `event-stream` feature is finally used as intended.
- A clean seam for a loading spinner (the heartbeat already drives periodic
  redraws).

**Accepted trade-offs:**

- Two new tokio features + one small dependency (`tokio-stream`).
- The entry points become `async` (`browse`, `run_mine`), rippling into their
  call sites in `main.rs` and the one loop test (now an awaited async call).
- A modest idle redraw cadence (heartbeat) — negligible cost given ratatui's
  diffed flush.

## Related

- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md)
- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md)
- ADR: [/adr/0003-http-transport-and-mocked-server-testing.md](/adr/0003-http-transport-and-mocked-server-testing.md)
