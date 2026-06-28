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
    screens --> asset_panel["tui/screens/asset_panel.rs\nAnexos/Artefatos composition source\nlayout → Vec&lt;PanelRow&gt; → inline content lines\n+ pure line→asset-index map"]
    render --> asset_panel
    model --> asset_panel
    screens --> drawer
    screens --> theme
    main --> cli["cli (clap)"]
    cli --> commands["commands\nsetup · get · current · mine · browse"]
    commands --> controller["controller\n(async orchestration)"]
    model --> controller
    controller --> client["client\n(ActiveCollab API)"]
    client --> http["http\n(reqwest + rustls)"]
    controller --> store["store\n(rusqlite: instances · settings ·\ncache: TaskCache · UserMapCache · ProjectNamesCache · TaskListCache)"]
    commands --> render["render\ndomain string rendering\n(get/current/mine non-TTY)\n+ TUI detail content (panels · style runs)"]
    render --> richtext["richtext\nHTML → structured rich text\n(TUI detail: lists · headings · emphasis · links)"]
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
  renders the assets inline at the end of the single globally-scrollable content
  (ADR 0029 — no fixed panel). All colors live in `theme.rs` — no inline
  `Color`/`Style` literals in the screen or drawer modules.
- **the Anexos/Artefatos assets are part of the global scroll, from one composition
  source** ([ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md),
  amending [ADR 0028](/adr/0028-asset-panel-single-layout-source.md)):
  `screens/asset_panel.rs` owns a pure `layout(assets, width) -> Vec<PanelRow>`;
  `build_detail_content` (`render.rs`) splices that vector into the scrollable
  `lines`/`line_styles` (so every attachment is reachable by scrolling — no fixed
  panel, no height cap), and the same vector yields a pure line→asset-index map the
  click hit-test (`model.rs`) reads. The asset click is **scroll-aware**, sharing
  the body-link `offset + (row − text_top)` translation
  ([ADR 0020](/adr/0020-body-links-inline-url-native-click.md)). Fitness: the
  rendered asset lines and the click map both derive from the one `layout` vector
  (they cannot drift), gate-checked by a unit test on the `Vec<PanelRow>` + the map
  and a TestBackend render derived from the real buffer
  ([BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md)).

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

The **browse/mine list load** is **stale-while-revalidate** for the project
directory ([ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md),
[BDR 0008](/bdr/0008-browse-list-refresh-cached-directory.md)): per instance,
`controller::tasks_by_project` **always** fetches the open tasks but serves
project **names** from the per-instance `ProjectNamesCache` (TTL), issuing
`list_projects` only on a cache miss or a stale entry. A warm refresh therefore
hits the network for open tasks alone — the directory fetch is the cached, slow
call. Fitness: a warm refresh issues **zero** `list_projects` requests
(gate-checked against the mocked server).

The **detail load** enriches the task with its **project name** before rendering
([ADR 0022](/adr/0022-detail-title-as-meta-row.md),
[BDR 0016](/bdr/0016-detail-title-row-project-name.md)): the task JSON carries only
`project_id`, so the load path resolves the name from the **same** per-instance
`ProjectNamesCache` the browse/mine list uses and injects `project_name`, which the
`Detalhes` panel renders in the `Projeto` row (with a fallback on a cache miss). No new
network call — the name comes from the existing cache.

Background results (e.g. `Msg::LoadedTasksByProject`) are delivered over a
`tokio::sync::mpsc` channel that is a first-class arm of the `tokio::select!`
loop. The model is updated and the screen repainted as soon as the result
arrives — no input event is required.

## Write / comment-mutation data flow

The app's first **write** path ([PRD 0002](/prd/0002-task-comment-authoring.md))
creates/edits/deletes a comment on the open task. It reuses the same TEA effect
machinery as reads: a pure `update()` emits a write `Cmd`, the shell spawns it as a
background task, and the 2xx result feeds a **server-truth refresh**
([ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md)) rather than an
optimistic local edit.

```mermaid
sequenceDiagram
    actor User
    participant Shell as shell (event loop)
    participant App as app (update, pure)
    participant Spawn as spawn_submit/delete (tokio task)
    participant API as client/http (authed write)
    participant Server as ActiveCollab

    User->>Shell: c / type / Ctrl+S (or [editar]/[excluir] click)
    Note over Shell: compose active → map_compose_key_event<br/>(else map_browse_key_event)
    Shell->>App: Msg (ComposeInput · ComposeSubmit · DeleteCommentRequest · confirm)
    App-->>Shell: Model + Cmd::SubmitComment / Cmd::DeleteComment
    Shell->>Spawn: dispatch_cmds → spawn write
    Spawn->>API: create/update/delete_comment (token only to instance host)
    API->>Server: POST/PUT/DELETE /api/v1/comments/...
    Server-->>API: 2xx | 4xx/5xx
    alt 2xx
        Spawn-->>Shell: Msg::CommentMutationOk
        Shell->>App: CommentMutationOk
        App-->>Shell: clear compose + Cmd::LoadDetail { refresh: true }
        Note over Shell,Server: thread re-derived from a fresh fetch (read flow above)
    else failure
        Spawn-->>Shell: Msg::CommentMutationErr(reason)
        Shell->>App: CommentMutationErr
        App-->>Shell: keep buffer + compose status = Error (no refresh)
    end
```

**Write boundaries / fitness:**

- **`client/http` stays the only outbound-network boundary**, and **token
  host-isolation extends to writes**: `authed_post`/`authed_put`/`authed_delete` attach
  `X-Angie-AuthApiToken` only via the same `host_gated_token_header` gate as
  `authed_get` ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)).
  Gate-checked by a negative test (no token off-host).
- **`tui/model.update` stays pure** through the write path: it owns the compose state
  machine (`Screen::Detail.compose`, `confirm_delete`) and emits write `Cmd`s, but never
  performs I/O. The shell owns the mode-aware key mapping (which keys are *text*) and the
  spawned write ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md)).
- **No optimistic mutation:** the mutation arms construct no synthetic comment; the
  thread is always re-derived from the server after a 2xx
  ([ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md)). Gate-checked by
  a unit test asserting `CommentMutationOk` emits exactly one `LoadDetail { refresh:true }`.
- **Edit/delete target a comment** via permission-aware `[editar]`/`[excluir]` click
  targets rendered only on the user's own comments (`created_by_id == instance.user_id`),
  reusing the scroll-aware asset click-map
  ([ADR 0036](/adr/0036-permission-aware-comment-targeting.md)). The local own-check is an
  affordance filter; the **server** (`canEdit`/`canDelete`) is the authorization boundary.

## Quality gates

The Rust crate enforces a comment policy via the `comment_policy` integration test (`tests/comment_policy.rs`), run as part of `cargo test`. It forbids banner/divider comments (e.g. `// ----`, `// === Section ===`, box-drawing chars) and commented-out Rust code, while allowing doc comments (`///`, `//!`) and ordinary prose why-comments that explain non-obvious intent.
