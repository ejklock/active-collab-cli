---
type: Architecture View
title: Architecture — Rust app structure and data flow
description: Living Mermaid views of the Rust module structure and the read/browse data flow.
tags: [architecture, rust]
timestamp: 2026-06-25T00:00:00Z
---

# Architecture

Living diagrams of the Rust app ([ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md),
[ADR 0006](/adr/0006-promote-crate-to-repo-root.md)).
Node names use [context-index](/context/index.md) vocabulary. All slices R0–R8 are
complete; the crate is at the repo root (`src/`). This view is updated as each
structural change lands (maintenance invariant: structural change updates this diagram).

## Module structure

```mermaid
flowchart TD
    main["shell (main.rs)\ntokio + crossterm lifecycle"] --> app["app (TEA core)\nModel · Msg · update · view"]
    main --> cli["cli (clap)"]
    cli --> commands["commands\nsetup · get · current · mine · browse"]
    commands --> controller["controller\n(async orchestration)"]
    app --> controller
    controller --> client["client\n(ActiveCollab API)"]
    client --> http["http\n(reqwest + rustls)"]
    controller --> store["store\n(rusqlite: instances · settings · cache)"]
    commands --> render["render"]
    commands --> i18n["i18n (en · pt-BR)"]
    commands --> assets["assets\n(open · download)"]
    client --> models["models (serde)"]
```

**Boundaries / fitness:**

- **app.update** is pure — no terminal, no async, no I/O. Gate-checked by unit
  tests (BDR 0001) and `cargo test` running headless.
- **client/http** is the only outbound-network boundary; **token host isolation**
  is enforced here and gate-checked by a negative test (PRD NFR).
- **store** owns all persistence; no other module opens the SQLite file.

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

## Quality gates

The Rust crate enforces a comment policy via the `comment_policy` integration test (`tests/comment_policy.rs`), run as part of `cargo test`. It forbids banner/divider comments (e.g. `// ----`, `// === Section ===`, box-drawing chars) and commented-out Rust code, while allowing doc comments (`///`, `//!`) and ordinary prose why-comments that explain non-obvious intent.
