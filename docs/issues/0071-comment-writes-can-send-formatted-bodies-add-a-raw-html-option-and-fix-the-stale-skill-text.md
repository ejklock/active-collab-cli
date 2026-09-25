---
type: Issue
title: Comment writes can send formatted bodies — add a raw HTML option and fix the stale skill text
description: Let ac comment post a raw HTML body through an opt-in flag, so a comment can show visible section gaps and bold headings, and correct the embedded active-collab skill text and CHANGELOG that still tell agents to pass HTML, which the 0.8.0 encoder escapes. Extends issue 0066.
status: open
timestamp: 2026-09-25T15:14:12Z
---

## 0071. Comment writes can send formatted bodies — add a raw HTML option and fix the stale skill text

Extends [issue 0066](/issues/0066-comment-writes-preserve-paragraph-breaks-encode-newlines-as-html-before-post-put.md) and the non-interactive comment command of [ADR 0040](/adr/0040-non-interactive-comment-write-command.md). Since 0.8.0, `create_comment` and `update_comment` pass every body through `encode_comment_body` (`src/client.rs:124`, `:401`, `:481`). The encoder escapes `&`, `<` and `>`, wraps blank-line-separated paragraphs in `<p>`, and turns a single newline into `<br>`. That fixed the flattened paragraphs of issue 0066. Field evidence gathered on 2026-09-25 against collab.base.digital (ActiveCollab 7.2.25) shows two gaps.

First gap: plain text cannot format a comment. The web UI renders comments inside `div.rich_text`, whose CSS sets `p { margin: 0 }`, so the encoder's consecutive `<p>` blocks show no visible gap between sections (field evidence). A visible gap needs an empty spacer paragraph `<p>&nbsp;</p>` or `<br /><br />`. Plain text cannot produce either. A line holding only U+00A0 is dropped, because `str::trim` treats it as whitespace (`src/client.rs:129`). A typed `&nbsp;` is escaped to `&amp;nbsp;` (`src/client.rs:125`). Bold, lists and mentions (`<span class="new_mention" data-user-id="ID">Name</span>` in the API docs) are out of reach too.

Second gap: the texts that teach agents are stale. `.claude/skills/active-collab/SKILL.md`, compiled in by `include_str!` at `src/commands/skill.rs:13`, says `ac comment` sends the body verbatim and tells agents to pass HTML (`:3`, `:110-147`). That was true when the text landed in 0.5.0 (commit `29e7030`). Since 0.8.0 the encoder escapes the tags, so an agent that follows the skill posts literal `<p>` text; a post through `ac` 0.9.0 showed exactly that (field evidence). `CHANGELOG.md` has no `[0.8.0]` section, because the 0.8.0 bump commit `bcfe20a` touched only `Cargo.toml` and `Cargo.lock`, so nothing records the change. [BDR 0027](/bdr/0027-non-interactive-comment-creation.md) is stale in the same way: its Scenario 2 says the body "is posted verbatim" (`docs/bdr/0027-non-interactive-comment-creation.md:52-54`).

Field evidence that the raw format works on the server: comment 143744 was posted by an `ac` build older than the encoder commit `317a3d9`, and comment 144890 by a direct `POST /api/v1/comments/task/78975`. Both are stored intact as `<p><strong>Heading</strong></p><p>line</p><p>U+00A0</p>…` with no escaped tags, and both render with bold text and visible gaps.

Issue 0066 excluded "letting the user type raw HTML" (issue 0066, Scope). Its goal was paragraph breaks from plain text, and its encoder shipped in 0.8.0. But before 0066, `ac comment` sent the body unchanged, so HTML was the documented route to a formatted comment. The encoder closed that route for every caller. This issue reopens it as an explicit opt-in: the 0066 encoder stays the default, and only a caller that asks for raw HTML skips it.

### Scope

Included:

- `ac comment` gains an opt-in flag, working name `--html` (`CommentArgs` at `src/cli.rs:176-188`, `dispatch_comment` at `src/main.rs:495-524`, `comment_core` at `src/commands/comment.rs:13`). With the flag, the body from `-m/--message` or piped stdin is posted exactly as given, without `encode_comment_body`.
- The ADR 0040 guard holds: "the command calls the **same** `client.create_comment` the TUI uses … it is a non-interactive adapter, not a second write implementation" (ADR 0040, Guard / fitness function). So the body format travels into `create_comment`, which keeps its URL, `post_json_write` (`src/client.rs:366`), the origin-gated `authed_post` ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md), [ADR 0067](/adr/0067-origin-gated-api-token-header-scheme-host-port.md)) and `classify_comment_write` ([ADR 0054](/adr/0054-comment-write-outcome-typed-classification.md)). The TUI caller (`src/tui/mod.rs:760`) keeps the plain-text format.
- `SKILL.md` describes the CLI as it is. The frontmatter `description` (`:3`) and the comment section (`:110-147`) document the plain-text default: a blank line starts a paragraph, a newline is a line break, `<`, `>` and `&` show literally, and the web UI shows no gap between paragraphs. The HTML guidance moves under `--html`, where the body is sent unchanged and newlines carry no meaning. Every example with a tag carries the flag. Installed stubs run `ac skill active-collab` (`install-skill.sh:10-11`, `:47`; [ADR 0057](/adr/0057-agent-skill-served-by-ac-skill-command.md), renamed by [ADR 0059](/adr/0059-rename-skill-to-active-collab.md)), so the fix reaches agents with the binary upgrade.
- `CHANGELOG.md` gains the missing `## [0.8.0] - 2026-09-14` section, dated from the `v0.8.0` tag, with at least the issue 0066 change under Changed: blank lines become paragraphs, and HTML in a body is now escaped. A docs-only backfill outside a bump has precedent in [issue 0055](/issues/0055-reconcile-changelog-with-rust-tag-line.md). The repo writes a section in the version-bump commit and keeps no `[Unreleased]` section, so the flag goes under Added in the bump commit of the release that ships it.

- BDR 0027 is amended in place: the usage line (`:4`) gains `--html`, Scenario 2 describes the plain-text encoding, and a new scenario covers `--html`. The maintenance rule in `CLAUDE.md` makes "the relevant ADR/BDR" follow a structural change.

Explicitly kept: the default encoder and its output, the empty-body guard (`src/commands/comment.rs:23`), the body sources (`src/main.rs:501-510`), the exit codes, the `--json` write result (`src/agent_json.rs:128`), the typed outcome, the TUI compose and edit paths, and `update_comment`, which keeps encoding. The 0.5.0 CHANGELOG entry (`CHANGELOG.md:121-125`) stays as written: it was true for 0.5.0, and released history is not rewritten.

Excluded: HTML sanitizing or validation in the CLI. The server purifies bodies against a tag allowlist ([Research 0001](/research/0001-tui-richtext-links-selection.md), read from the 7.1.141 source; field evidence from a 7.3.70 source mirror; neither tested on 7.2.25). Also excluded: any change to the default encoder, raw HTML in the TUI, a CLI edit command, and Markdown conversion. Known limitation: a TUI edit of an `--html` comment pre-fills plain text (`handle_edit_comment_request`, `src/tui/model.rs:2036`) and `update_comment` re-encodes it, so the formatting is lost. The server lets the author edit a comment only within 30 minutes, Owner and project leader excepted (field evidence from the 7.3.70 source, unverified on 7.2.25).

### Decision

The caller states the body format with an explicit opt-in raw-HTML flag on `ac comment` (owner-approved, working name `--html`). The CLI never guesses the format. The code is cheap to reverse: removing the flag leaves the default path untouched. The contract is not: once a release ships the flag and the skill teaches it, removing or renaming it turns agent calls into usage errors, so the name must be final before that release.

Options not taken here:

- Make the default encoder emit spacer paragraphs, or turn `- item` lines into lists. This changes the output of every plain-text comment and the tested 0066 contract, and it still cannot express bold, links or mentions. It stays an open question for a later issue.
- Detect HTML in the body, for example a leading `<p>`. This breaks issue 0066 AC3, which keeps a literal `<` visible.
- Convert Markdown to HTML. A converter adds a dependency and a dialect to maintain; issue 0066 also excluded a Markdown/rich-text compose editor.
- Revert issue 0066 and send every body unchanged. Plain text loses its paragraph breaks again, and a literal `<` or `&` becomes markup again.

Owner answers (2026-09-25):

- Flag name: `--html`, long form only, no short alias. Options not taken: `--raw`, `--format text|html`.
- Seam shape: a `BodyFormat { Text, Html }` enum parameter on `create_comment`. Options not taken: a boolean, a typed body. A separate client method stays closed without an ADR 0040 amendment.
- Record: amend BDR 0027 in place (usage line, an `--html` scenario, a corrected Scenario 2). No successor BDR, no new ADR.
- CHANGELOG breadth: the full `[0.8.0]` that the `v0.8.0` tag message names (issues 0066, 0067, 0068). `[0.7.3]` and `[0.7.4]` stay out.
- Skill markup: only the field-verified `<p>`, `<strong>` and `<p>&nbsp;</p>`. `<ul><li>` and the mention span stay out until verified.
- Markup-only bodies such as `<p></p>` are accepted under `--html`; the `str::trim` guard is unchanged and the server decides.

### Acceptance

- AC1 — default unchanged: without the flag, the posted and put bodies are byte-identical to today's encoder output. Proof: the diff leaves `encode_comment_body` (`src/client.rs:124-146`) unchanged, and the `encode_comment_body_*` tests (`tests/unit/client.rs:1261-1283`), `create_comment_posts_to_correct_path_with_body` (`:720`), `update_comment_puts_to_correct_path_with_body` (`:1023`) and `comment_core_multiline_stdin_body_encoded_as_br` (`tests/unit/commands.rs:2508`) pass with their assertions unedited.
- AC2 — raw post on the same seam, no sanitizing: with the flag, the POST body equals the input byte for byte, including `<strong>`, `&nbsp;`, newlines, partial markup such as `<p>unclosed` and `a < b & c`, and tags outside the recorded allowlist such as `<hr>` and `<script>`. The request carries the token header, and `comment_core` reaches it through `client.create_comment`. Proof: `comment_core_html_flag_posts_body_verbatim` (wiremock `body_json` plus `header("x-angie-authapitoken", …)`), the unchanged `origin_gated_token_*` negatives (`tests/unit/http.rs:22-72`), and inspection of the call in `src/commands/comment.rs`.
- AC3 — empty body: with the flag, a body that is empty or only whitespace to `str::trim` (spaces, newlines, U+00A0) exits 2 with the existing empty-body message (`t("no comment body")`, `src/commands/comment.rs:24`) and sends no request. Proof: `comment_core_html_flag_whitespace_only_body_returns_exit2`, which asserts that `server.received_requests()` is empty.
- AC4 — flag surface and body sources: the flag parses with `-m` and without it (stdin mode), defaults to off, is listed by `ac comment --help`, and adds no body source (there is no editor source). `dispatch_comment` passes the flag to `comment_core` and reads the body the same way with or without it, so a TTY stdin without `-m` gives an empty body (AC3), and non-UTF-8 stdin reads as empty because the read error is swallowed (`src/main.rs:506`). Proof: `parse_comment_html_flag_with_message`, `parse_comment_html_flag_without_message`, `parse_comment_html_flag_defaults_to_false` and `comment_help_lists_html_flag` in `tests/unit/cli.rs`, plus inspection of `src/main.rs:495-524`.
- AC5 — output and failures: with the flag, `--json` prints the same minified `{"ok":true,"comment_id":N,"task_id":N,"project_id":N}` line, an HTTP 4xx exits non-zero with no success line, and an HTTP 401 prints the reauth message. The flag adds no size cap, so a server rejection of a large body takes this failure path. Proof: `comment_core_html_flag_json_output_is_unchanged`, `comment_core_html_flag_http_4xx_returns_nonzero` and `comment_core_html_flag_401_prints_reauth_message`, modeled on `comment_core_401_prints_reauth_message_and_returns_nonzero` (`tests/unit/commands.rs:3020`).
- AC6 — skill text: in the embedded skill, (1) the sentence "sends whatever you pass **verbatim** — it does not convert newlines" is gone; (2) the frontmatter no longer says the body "must be formatted as HTML"; (3) every `ac comment` example whose body holds a tag carries `--html`; (4) the comment section documents the plain-text default, including that the web UI shows no gap between paragraphs. Proof: `skill_comment_section_documents_plain_default_and_html_flag` in `tests/unit/skill.rs`; `skill_prints_named_body` already ties the `ac skill` output to the file.
- AC7 — changelog: `CHANGELOG.md` has exactly one `## [0.8.0] - 2026-09-14` section, between `[0.9.0]` and `[0.7.2]`, that records the issue 0066 change under Changed and any items the owner's breadth answer adds; the 0.5.0 entry (`CHANGELOG.md:121-125`) is unchanged. Proof: inspection against `git tag -n1 v0.8.0` and `git diff CHANGELOG.md`.

### Plan

1. Flag and seam (AC1–AC5): add the flag to `CommentArgs`, thread it through `dispatch_comment` and `comment_core` into `client.create_comment` in the shape the owner picks, and skip `encode_comment_body` only for raw HTML. The TUI keeps the plain-text format.
2. Skill text (AC6): rewrite `SKILL.md` line 3 and the comment section; add the skill test.
3. Records (AC7): backfill `[0.8.0]` at the chosen breadth; apply the owner's answer on BDR 0027; record the flag under Added in the bump commit of the release that ships it.
4. Gates: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and `cargo test --test comment_policy`, each through `docker compose run --rm dev`, then `living-docs check`.
