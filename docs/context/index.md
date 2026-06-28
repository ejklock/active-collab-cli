# Context Index

Domain and module vocabulary for active-collab-cli. Terms and acronyms are defined
once in the [glossary](/context/glossary.md); this index groups the concepts.

## Domain

- **Instance**, **Setting**, **CachedTask**, **Comment**, **Asset** — the core
  entities; see the data model in the [constitution](/constitution.md).
- **Parity**, **Token host isolation** — see [glossary](/context/glossary.md).

## Application structure (Rust, planned)

See [architecture](/architecture.md) for the diagrams.

- **app (TEA core)** — `Model`, `Msg`, pure `update`, `view`. Terminal/async-free.
- **shell** — `main`: tokio runtime, crossterm terminal lifecycle, event→`Msg` loop.
- **controller** — orchestrates fetches for the TUI over tokio.
- **client / http** — ActiveCollab API client and HTTP transport.
- **store** — rusqlite-backed instances/settings/task-cache.
- **i18n**, **assets**, **render** — localization, attachment open/download, CLI rendering.

## Conventions

- **Slice (R0–R8)**, **Single-flight refresh** — see [glossary](/context/glossary.md).
