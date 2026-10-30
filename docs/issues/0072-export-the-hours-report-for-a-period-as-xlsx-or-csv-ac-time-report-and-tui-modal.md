---
type: Issue
title: Export the hours report for a period as XLSX or CSV (ac time report and TUI modal)
description: Export the logged-in user's hours report for a period, by preset or range, as XLSX or CSV, from the CLI and a TUI modal, because me-report.php accepts only a browser session.
status: closed
timestamp: 2026-10-30T15:47:46Z
---

## 0072. Export the hours report for a period as XLSX or CSV (ac time report and TUI modal)

### Problem

The hours report for a period is today downloaded from the instance's custom `me-report.php?start=DD-MM-YYYY&end=DD-MM-YYYY` page. That page authenticates only through a browser session cookie: it answers `401` ("Voce precisa estar logado no ActiveCollab neste navegador") to an anonymous request and to the CLI's API token sent as the `x-angie-authapitoken` header, as `Authorization: Bearer`, or as a `token`/`auth_api_token` query parameter, while the same token answers `200` on `/api/v1/user-session` (probed with curl on 2026-10-30 against collab.base.digital). So the CLI cannot download that file.

The ActiveCollab 7 API returns the same data to the token the CLI already holds: `GET /api/v1/users/{user_id}/time-records/filtered-by-date?from=YYYY-MM-DD&to=YYYY-MM-DD` returned, for user 69 and 2026-08-01..2026-08-30, 32 records summing 79 hours: the same rows and total as the owner's `me-report.php` CSV for that period.

### Scope

Included:

- `ac time report --start DD-MM-YYYY --end DD-MM-YYYY [--instance NAME]`: fetches the logged-in user's time records for the period and writes `hours-report-<start>-<end>.<ext>` in the current directory, with `<start>` and `<end>` as `DD-MM-YYYY`.
- `--period month|fortnight|week` as the alternative to `--start`/`--end`, resolving the current period from today.
- `--format xlsx|csv`, default `xlsx`.
- TUI: a modal opened with `H` in `ac browse`, offering the presets Monthly, Biweekly, Weekly and a Custom range (start and end dates), plus the format; confirming it writes the same file as the CLI and shows its path.
- A read method on the client for the filtered-by-date endpoint, over the existing host-gated `authed_get`.
- The CSV layout of `me-report.php` with English text: a `Total hours for the period;<total>` line, an empty line, the header `Project;Activity;Hours;Date`, then one row per record.

Excluded: other users' reports, grouping or totals per project, a `--json` output, choosing the output directory, and the `me-report.php` page itself.

### Decision

- Verb `ac time report`, under the existing `time` group, over a top-level `report`.
- Field separator `;`, the pt-BR Excel default, over `,` or tab; the file starts with a UTF-8 BOM so Excel reads the accented names. Hours and total use a decimal comma with trailing zeros dropped (`1`, `1,5`, `5,42`, `79`).
- Rows sort by record date, then project name, then record id, which reproduces the order of the owner's August CSV.
- Report text is English, as the owner chose; the separator, decimal comma and `dd/mm/yy` date of `me-report.php` stay. The TUI modal labels are English source strings (Monthly, Biweekly, Weekly, Custom) translated by the existing i18n when the display language is pt_BR.
- `Activity` is the task name for a task record; for a record logged on the project itself it is the record's summary, empty when the summary is empty.
- `Date` is the record date as `dd/mm/yy`, read in UTC: `record_date` is a UTC-midnight timestamp, and a local-time conversion would shift it to the previous day in Brazil.
- An existing file of the same name is overwritten; the report is regenerable.
- Presets resolve the current period from today (local date): month = day 1 to the last day of the month; fortnight = 1–15 or 16–last day of the month; week = Monday to Sunday. Over the previous closed period or the last N days, as the owner chose. `--period` and `--start`/`--end` are mutually exclusive; one of them is required.
- The TUI report modal is a model-level overlay (`Model.report`), not a `DetailOverlay` variant: the report is not tied to a task, so `H` opens it from the Projects, Tasks and Detail screens alike, over the per-screen overlays of [ADR 0047](/adr/0047-detail-overlay-as-one-typed-state.md). It opens only from browse mode, so it never coexists with a Detail overlay, and the shell routes keys to it first while it is open. The report runs for the instance of the open Detail task, else the first configured instance, and the modal names that instance; an instance picker is left out until someone has two instances to report on. The file goes to the process's current directory, like the CLI. On success the modal stays open showing the saved path and total until Esc; an error shows inline and keeps the form editable.
- The CLI and the TUI share one export function that returns the written path and total (or a typed failure); the CLI maps it to exit codes and messages, the TUI to `Msg`s. Two paths that each fetch, render and write would drift.
- XLSX is the default format, as the owner chose; CSV stays available for the `me-report.php` layout. The XLSX holds the same rows as the CSV on one sheet, with Hours and the total as numbers and Date as a real date shown `dd/mm/yy`. Written with `rust_xlsxwriter` (MIT OR Apache-2.0, 0.102.0 on 2026-10-29, ~5.9M downloads, MSRV 1.88; the build uses stable Rust): a hand-written OOXML zip is not worth owning.
- Facts behind these choices (probed 2026-10-30): `to` is inclusive; the endpoint does not paginate (515 records over 2025-01-01..2026-10-30 in one response); it answers `200` to a malformed date, so the CLI validates dates itself; another user's id answers `404`.

### Acceptance

- AC1: the report rows, total, order and formatting match the Decision rules for task records and project records. (`verify_by: test`)
- AC2: `ac time report` validates `--start`/`--end` (malformed date, impossible date, start after end are usage errors, exit 2), uses the instance's stored user id, writes the named file, and maps 401 to the re-auth message and another failure to a non-zero exit. (`verify_by: test`)
- AC3: run against collab.base.digital for 01-08-2026..30-08-2026, the file holds the owner's 32 August rows (same cells and order as the owner's `me-report.php` export) and the total 79. (`verify_by: command`)
- AC4: `--period` resolves month, fortnight (both halves, including a 28-, 30- and 31-day month) and week (on a Monday, a Sunday, and across a month and a year boundary) from an injected today; combining it with `--start`/`--end`, or giving neither, is a usage error. (`verify_by: test`)
- AC5: `--format xlsx` (the default) writes `.xlsx` with the same rows, numeric Hours and total, and dates as dates; `--format csv` writes the CSV unchanged. (`verify_by: test`)
- AC6: the TUI modal opens with `H`, selects a preset or a custom range and a format, rejects an invalid custom date inline, and on confirm writes the file and shows its path; the transitions live in the pure update core. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Outcome

Shipped `ac time report` (`--start`/`--end` or `--period month|fortnight|week`, `--format xlsx|csv`, xlsx by default) and the TUI `H` modal (Monthly, Biweekly, Weekly, Custom), both through one export function in `src/commands/hours_report.rs`.

- AC3 was checked live against collab.base.digital: the August CSV matched the owner's 32 rows and total 79; the XLSX held sheet `Hours`, B1 = 79 and `dd/mm/yy` date cells; `--period month` for October gave 90h over 39 records, as the API reports.
- The `filtered-by-date` response keys `related.Project` and `related.Task` by id as an object, and sends `related` as `[]` when empty; the parser takes both shapes.
- The TUI spawn functions that resolve an instance and build a client share one helper, and pointer input is ignored while the modal is open.
- Not run live: the TUI modal export, covered by the pure-core and render tests.
