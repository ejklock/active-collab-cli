---
type: Architecture View
title: Architecture — Rust app structure and data flow
description: Living Mermaid views of the Rust module structure and the read/browse data flow.
tags: [architecture, rust]
timestamp: 2026-06-25T00:00:00Z
---

# Architecture

Living diagrams of the Rust app ([ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md),
[ADR 0006](/adr/0006-promote-crate-to-repo-root.md),
[ADR 0007](/adr/0007-tui-module-structure.md)).
Node names use [context-index](/context/index.md) vocabulary. All slices R0–R8 are
complete; the crate is at the repo root (`src/`). This view is updated as each
structural change lands (maintenance invariant: structural change updates this diagram).

## Module structure

```mermaid
flowchart TD
    main["shell (main.rs)\ntokio + crossterm lifecycle"] --> tui["tui/mod.rs\nrun_app (async select!)\nbrowse · run_mine"]
    tui --> model["tui/model.rs\nModel · Msg · Cmd · Screen · update\nmine_model · init_browse · TaskRow (project_id)"]
    tui --> events["tui/events.rs\ncrossterm Event → Msg mapping"]
    tui --> view["tui/view.rs\nview()\nframe Layout split: header + content + footer"]
    view --> screens["tui/screens/\nprojects.rs · tasks.rs · detail.rs\neach owns its draw_* fn\n(responsive Table · detail wraps text + assets panel)"]
    view --> drawer["tui/drawer.rs\nshared widget builders (render_table)"]
    view --> theme["tui/theme.rs\ncentralized Style / Color constants"]
    screens --> drawer
    screens --> theme
    main --> cli["cli (clap)"]
    cli --> commands["commands\nsetup · get · current · mine · browse"]
    commands --> controller["controller\n(async orchestration)"]
    model --> controller
    controller --> client["client\n(ActiveCollab API)"]
    client --> http["http\n(reqwest + rustls)"]
    controller --> store["store\n(rusqlite: instances · settings · cache)"]
    commands --> render["render\ndomain string rendering\n(get/current/mine non-TTY)"]
    commands --> i18n["i18n (en · pt-BR)"]
    client --> models["models (serde)"]
```

**Boundaries / fitness:**

- **tui/model.update** is pure — no terminal, no async, no I/O. Gate-checked by unit
  tests (BDR 0001) and `cargo test` running headless.
- **client/http** is the only outbound-network boundary; **token host isolation**
  is enforced here and gate-checked by a negative test (PRD NFR).
- **store** owns all persistence; no other module opens the SQLite file.
- **mine and browse share one TEA core**: `run_app` (async) seeds from `mine_model`
  (rows already fetched, no init_cmds) or `init_browse` (LoadTasksByProject on start).
  Enter/click on the mine Tasks screen opens Detail through the same `update` path.
- **the view layer is responsive and theme-centralized**: `view()` splits the frame
  vertically into three regions — a one-line identity header (`app_header_style`:
  white on cyan, bold), a variable-height content area, and a one-line footer.  The
  too-small guard (width < 24 or height < 6) bypasses all three and renders only a
  centered `"Terminal too small"` message.  List screens render a ratatui `Table`
  driven by width `Constraint`s (no fixed-width truncation) with a
  `TableState`-driven selection highlight; the detail screen wraps long lines and
  renders assets in a dedicated panel. All colors live in `theme.rs` — no inline
  `Color`/`Style` literals in the screen or drawer modules.

## Read / browse data flow

```mermaid
sequenceDiagram
    actor User
    participant Shell as shell (event loop)
    participant App as app (update)
    participant Ctl as controller (tokio task)
    participant Cache as store (cache)
    participant API as client/http

    User->>Shell: key / mouse / scroll
    Shell->>App: Msg
    App-->>Shell: new Model (pure)
    Shell->>Ctl: request load (async)
    Ctl->>Cache: read cached?
    alt cache hit and not refresh
        Cache-->>Ctl: cached task
    else miss or refresh
        Ctl->>API: fetch (token only to instance host)
        API-->>Ctl: payload
        Ctl->>Cache: write
    end
    Ctl-->>Shell: Msg::Loaded(data)
    Shell->>App: Msg::Loaded
    App-->>Shell: Model with data
    Shell-->>User: re-render (loader hidden)
```

The refresh path is **single-flight**: a refresh requested while a load is in
flight is dropped, not queued.

Background results (e.g. `Msg::LoadedTasksByProject`) are delivered over a
`tokio::sync::mpsc` channel that is a first-class arm of the `tokio::select!`
loop. The model is updated and the screen repainted as soon as the result
arrives — no input event is required.

## Quality gates

The Rust crate enforces a comment policy via the `comment_policy` integration test (`tests/comment_policy.rs`), run as part of `cargo test`. It forbids banner/divider comments (e.g. `// ----`, `// === Section ===`, box-drawing chars) and commented-out Rust code, while allowing doc comments (`///`, `//!`) and ordinary prose why-comments that explain non-obvious intent.
