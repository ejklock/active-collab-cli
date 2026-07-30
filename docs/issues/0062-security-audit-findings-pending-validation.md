---
type: Issue
title: "Security audit findings (pending independent validation) — terminal-escape injection + token cleartext scheme-downgrade"
description: Output of an on-demand /security-audit recon pass over the shipped Rust `ac` binary (legacy Python tree excluded). Two candidate findings surfaced by a single analyst — C1 terminal ANSI/control-character injection from untrusted task/comment bodies rendered to the terminal, C2 API-token disclosure in cleartext because host-gating pins the host but not the URL scheme. These are UNVALIDATED candidates. A fresh, independent agent must run the Phase-3 disprove pass (build the exact triggering input, confirm the sink is reachable, confirm the impact, check for a mitigating layer) before either is treated as real or routed to a fix.
status: open
labels: [security, audit, needs-validation, candidate]
blocked_by:
tracker:
timestamp: 2026-07-29T00:00:00Z
---

## Security audit — candidate findings (pending validation)

On-demand `/security-audit` run. **Scope:** the shipped Rust binary `ac` (`src/*.rs`)
only; the legacy Python tree under `src/active_collab/` was excluded (in removal).
**Method reached:** Phase 1 (recon + deterministic sweep) and a single-analyst hunt.
**Not yet done:** the Phase-3 adversarial validation (a fresh agent that did NOT
discover these disproving each against the source) and Phase-6 independent
verification. **Everything below is a candidate, not a confirmed vulnerability.**

> Coverage honesty: a single audit pass finds roughly half of what is there. Absence
> of a finding in a module below is not proof it is clean. An LLM is never the
> security boundary — this issue *informs* deterministic fixes (negative tests /
> `code-security-gate`); it does not become the control.

### Deterministic sweep (already run, informational)

`~/.agent-tools/bin/security-gate-run.sh` on the working tree:

- `deps` (trivy / cargo-audit): **pass** — 0 hard, 1 advisory (no fixable crit/high).
- `secrets` (gitleaks): **pass** — 0 verified secrets.
- `sast` (semgrep): **pass** — 0 new high/critical.
- `iac` / `license` / `all`: not completed — the `all` container was OOM-killed
  (exit 137) on the audit host. Re-run `iac`, `license`, and a full `all` when
  validating.

The interesting candidates are business-logic / design issues a scanner cannot
pattern-match — hence this issue.

---

## C1 — Terminal escape / ANSI injection from untrusted task & comment bodies

**Severity (proposed):** MEDIUM · class: injection · verdict: **CANDIDATE**

**Data flow (untrusted → sink):**
1. `cli_render::html_to_text` (`src/render/cli_render.rs:35-46`) strips HTML tags and
   then calls `html_escape::decode_html_entities` **without stripping control
   characters**. `&#27;` / `&#x1b;` in a body decodes to a literal `ESC` (0x1B) byte;
   any C0/C1 control byte survives verbatim.
2. The decoded string flows into `render_task_to_str` / `render_comments_to_str`
   (`src/render/cli_render.rs:114-135,140-201`).
3. Human (non-`--json`) sink: `render::render_task(...)` at
   `src/commands/task.rs:151` writes that string **straight to stdout** (`out`).
   Reachable via `ac get <ref>` and `ac current` without `--json`/`--short`.

**Attacker & scenario:** any ActiveCollab collaborator who can create or comment on a
task the victim views posts a body containing `&#27;[…]` ANSI sequences (HTML entities
survive server-side raw-byte filtering because they are valid HTML text). When the
victim runs `ac get <task>` / `ac current`, the sequences reach their terminal.

**Impact:** reliable output spoofing (forge a "task completed"/success line, hide or
overwrite text, corrupt the display); in terminal emulators that honor them, OSC
sequences (e.g. OSC 52 clipboard write, window-title set/query) raise the ceiling.
Impact above "output spoofing" is terminal-dependent.

**Also unfiltered (validate breadth):** the TUI richtext parser decodes entities the
same way with no control-char strip (`src/richtext.rs:277,283,615`); confirm whether
ratatui's cell buffer neutralizes control bytes or passes them to the backend. The
`--json` path is **safe** — `serde_json::to_string` escapes control chars to `\u001b`
(`src/commands/task.rs:140`). `--short` prints `task.name` raw (`task.rs:145-146`) —
check whether the API can store raw control bytes in a name (a second, weaker vector).

**Confirmed during recon:** no control-character / ESC stripping exists anywhere in the
crate (grepped).

**Remediation direction (for the fix issue, not this one):** strip or escape C0/C1
control characters (keep `\n`/`\t`) at the render boundary before writing untrusted
text to a TTY — a single sanitizer applied in `html_to_text` and/or the terminal write
path. Bind the fix to a negative test: a body with `&#27;[2J` renders with no raw ESC
byte on stdout.

**Validation checklist:**
- [ ] Build the exact body string and confirm a raw 0x1B reaches stdout for `ac get`.
- [ ] Confirm the TUI path (ratatui) does or does not emit the control byte.
- [ ] Decide realistic impact ceiling (spoofing only vs. OSC-52) → confirm severity.

---

## C2 — API token sent in cleartext: host-gating pins host, not scheme

**Severity (proposed):** MEDIUM · class: access-control / credential disclosure · verdict: **CANDIDATE**

**Data flow (untrusted → sink):**
1. `asset.url` is taken **verbatim** from untrusted payload: `<img src>` / `<a href>`
   in task & comment HTML (`src/controller.rs:300-308`, `image_assets_from_html`) and
   the attachment `url` / `download_url` field (`src/controller.rs:311-333`).
2. `Http::host_gated_token_header` (`src/http.rs:30-44`) attaches the
   `x-angie-authapitoken` header when `extract_host(url) == extract_host(instance)`.
   `extract_host` (`src/http.rs:159-162`) returns the **host only** — the comparison
   ignores the URL **scheme and port**.
3. Reachable sink: `download_task_attachments` → `fetch_and_write_asset`
   (`src/controller.rs:554-575`) → `client.fetch_asset_bytes` (`src/client.rs:312-316`)
   → `authed_get`. Triggered by `ac get`/`current --download-attachments` (ADR 0066)
   and the TUI asset download.

**Attacker & scenario:** a collaborator embeds a same-host **`http://`** asset, e.g.
`<img src="http://<instance-host>/x.png">`. Because only the host is compared, the
token header is attached and the request goes out over **cleartext HTTP** to the
instance host. An on-path (MITM) attacker on the victim's network captures the full API
token in the request headers (redirects are disabled, so this is the request itself,
not a redirect leak).

**Impact:** disclosure of a full ActiveCollab API token → account/API compromise.
Likelihood is gated by two conditions (attacker injects a same-host `http://` asset URL
*and* holds a network position), so MEDIUM rather than HIGH.

**What already limits it (do not double-count):** the host-gate deliberately prevents
**cross-host** token exfiltration (a foreign-host asset URL gets **no** token) — that
part is correct and is the reason this is a scheme-downgrade, not an open exfil. The
residual gap is scheme (and, weakly, port).

**Remediation direction (for the fix issue, not this one):** in
`host_gated_token_header`, additionally require the request scheme to be `https` (or to
match the instance scheme) — and consider comparing port — before attaching the token.
Bind to a test: an `http://<instance-host>/…` asset URL yields **no** token header.

**Validation checklist:**
- [ ] Confirm `asset.url` is attacker-controllable end-to-end (payload → fetch).
- [ ] Confirm the token header is emitted on the cleartext request (unit test around
      `host_gated_token_header` / `authed_get` with an `http://` same-host URL).
- [ ] Confirm no other layer (e.g. reqwest refusing http, or the server 301→https
      *before* the header is sent) prevents the header from leaving the client.
- [ ] Re-check the port variant for realism on the target deployment.

---

## Well-defended (recon notes — NOT findings)

Recorded so a validator does not re-walk them; re-confirm if in doubt.

- **SQL** — fully parameterized via `rusqlite params!` across `store/*.rs`; no string
  concatenation into SQL.
- **Credentials at rest** — SQLite DB created `chmod 600`, parent dir `700`
  (`src/store/mod.rs:43-67`); token never logged.
- **TLS** — rustls, redirects disabled (`redirect::Policy::none()`), 30s timeout
  (`src/http.rs:19-26`).
- **`open_asset`** — validates the scheme via the `url` crate and rejects
  `file://`/`javascript://`/`data:` before the OS opener runs
  (`src/render/mod.rs:388-396`, `src/controller.rs:893-898`); spawn uses `.arg(url)`
  (no shell), so no command injection (`src/tui/mod.rs:708-716`).
- **Attachment download path safety (ADR 0066)** — final-component sanitize + reject
  `.`/`..`/all-dots (`src/controller.rs:418-437`), plus a canonicalized containment
  check with symlink-ancestor resolution (`src/controller.rs:580-610`). Strong defense
  in depth.
- **`git rev-parse`** — fixed args, no shell (`src/main.rs:558-573`).

## Unverified leads (not analyzed — for the validating agent to triage)

Surfaced but **not** worked into an exploit; may be nothing.

- **Panic on malformed ref** — `parse_task_ref` does `caps[N].parse().unwrap()` on
  `\d+` groups (`src/commands/resolve.rs:86-90`); a very long digit run overflows
  `i64::parse` → `unwrap()` panics. Input is a user-supplied CLI arg (self-inflicted),
  so likely LOW, but confirm no path feeds it attacker-controlled data.
- **Argument injection via git branch name** — the branch name is fed into
  `cli::normalize_argv` before clap parses (`src/main.rs:33-34`). Check whether a
  crafted branch (e.g. in a repo the victim clones) can inject a CLI flag/arg when the
  victim runs `ac`.
- **`ACTIVE_COLLAB_DB` arbitrary path** (`src/config.rs:18-27`) and predictable
  world-readable temp dir `${TMPDIR}/ac-attachments/{pid}-{tid}`
  (`src/controller.rs:442-446`) — local-only; assess for symlink/DoS on shared hosts.

---

## Next step

An independent agent (not the discoverer) runs the disprove pass on C1 and C2 using the
per-finding validation checklists above, triages the unverified leads, completes the
`iac`/`license`/`all` deterministic sweep, and updates this issue: each candidate →
**confirmed** (then route to `secure-development` for an abuser BDR + a security AC
bound to a deterministic instrument) or **rejected** (with the specific code reason).

---

## Validation pass — 2026-07-30 (independent, source-read)

Both candidates were re-derived from the source rather than accepted from the report.

### C1 — CONFIRMED (severity MEDIUM held); surface is wider than reported

- `html_to_text` (`src/render/cli_render.rs:41`) decodes entities with no control filter;
  the string is written to stdout by `render::render_task` (`src/render/mod.rs:532`),
  reached by `ac get`/`ac current` without `--json`/`--short`.
- A crate-wide sweep found **no** control-character or ESC stripping anywhere
  (no `is_control` / `sanitize` / `strip_control` / `0x1b` site).
- Additional untrusted sinks not listed in the original write-up:
  - `body_plain_text` is used **verbatim** when present and never passes through
    `html_to_text` (`cli_render.rs:114-126`) — a rawer vector than the entity path;
  - `created_by_name` (`cli_render.rs:128`) and the user-map display names
    (`cli_render.rs:55`) print raw;
  - the `ac mine` table emits row name / instance fields raw (`src/render/mod.rs:429`);
  - `task.name` prints raw in both the human view (`cli_render.rs:185`) and `--short`
    (`src/commands/task.rs:145-146`).
- TUI: the three `decode_html_entities` sites (`src/richtext.rs:277,283,615`) are
  unfiltered. Whether ratatui's cell buffer drops a zero-width control char was **not**
  settled — the fix sanitizes there anyway, as defense in depth rather than a claim about
  ratatui.
- `--json` re-confirmed safe (`serde_json` escapes control characters).

→ Routed to fix: [ADR 0068](/adr/0068-control-characters-stripped-at-the-untrusted-text-render-boundary.md).

### C2 — CONFIRMED (severity MEDIUM held)

- `extract_host` (`src/http.rs:159-162`) returns the host only; `host_gated_token_header`
  (`src/http.rs:30-44`) compares hosts and ignores scheme and port.
- The untrusted `asset.url` reaches the authenticated sink end to end:
  `controller.rs:561` → `client.rs:314` (`fetch_asset_bytes`) → `http.rs:56`
  (`authed_get`) → header attached.
- No mitigating layer: redirects are disabled, so the token leaves on the **first**
  request; reqwest does not refuse cleartext.
- The existing "foreign host gets no token" guarantee is real and is preserved by the fix.

→ Routed to fix: [ADR 0067](/adr/0067-origin-gated-api-token-header-scheme-host-port.md).

### Unverified leads

- **Panic on malformed ref** — confirmed reachable: `parse_task_ref`
  (`src/commands/resolve.rs:86-90`) calls `caps[N].parse::<i64>().unwrap()` on `\d+`
  captures, so a URL ref with a digit run that overflows `i64` panics instead of printing
  the normal parse error. Self-inflicted input, so LOW severity — fixed here anyway as a
  no-panic conversion to the existing `Err(2)` error path.
- **Argument injection via git branch name** and **`ACTIVE_COLLAB_DB` / temp-dir**: not
  triaged. They remain open leads on this issue.

### Deterministic sweep

The full `all` run now completes (no OOM): `sast` pass, `deps` advisory (1, no fixable
crit/high), `secrets` pass, `license` pass, `cargo-audit` pass, `policy` skipped (no
`policy/*.rego`), **`iac` fail — 1 hard finding** (trivy, high/critical). The IaC finding is
pre-existing (no fix here touches an IaC file) and the wrapper reports only the count, so it
is routed to [issue 0063](/issues/0063-triage-remaining-security-leads-and-the-pre-existing-iac-misconfiguration-from-issue-0062.md)
together with the untriaged leads.

---

## Resolution — both confirmed findings fixed (2026-07-30)

Three reviewed slices, each with negative tests bound to the vector:

| Finding | Fix | Where |
|---|---|---|
| C2 — cleartext token | Token attached only on full-origin equality (scheme + host + port); `host_gated_token_header` → `origin_gated_token_header` | `src/http.rs`, [ADR 0067](/adr/0067-origin-gated-api-token-header-scheme-host-port.md) |
| C1 — terminal escapes (CLI) | New `sanitize::strip_control_chars` applied to the assembled string at `render_task`, `render_mine_table`, and the `--short` line | `src/sanitize.rs`, `src/render/mod.rs`, `src/commands/task.rs`, [ADR 0068](/adr/0068-control-characters-stripped-at-the-untrusted-text-render-boundary.md) |
| C1 — terminal escapes (TUI) | All three entity-decode sites routed through one `decode_text` helper that sanitizes after decoding | `src/richtext.rs`, ADR 0068 |
| Panic lead | `parse_task_ref` falls through to the existing `Err(2)` path instead of `unwrap`-panicking on an i64 overflow | `src/commands/resolve.rs` |

Negative tests: no `0x1B` byte in `render_task` / `render_mine_table` / parser output for the
entity vector, the raw-byte vector, `body_plain_text`, and the task name; no token header on
a same-host `http://` request against an `https` instance (unit + wiremock); `Err(2)` with
the existing message for an overflowing ref. `--json` output is unchanged — the agent
contract stays byte-stable.

Remaining open items moved to
[issue 0063](/issues/0063-triage-remaining-security-leads-and-the-pre-existing-iac-misconfiguration-from-issue-0062.md).
