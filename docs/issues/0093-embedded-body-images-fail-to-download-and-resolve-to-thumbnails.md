---
type: Issue
title: Embedded body images fail to download and resolve to thumbnails
description: The attachment download takes each embedded image's original from its download_file anchor, resolves relative sources against the instance and names the file from original_file_name.
status: closed
timestamp: 2026-11-17T14:48:25Z
---

## 0093. Embedded body images fail to download and resolve to thumbnails

### Problem

`ac get --download-attachments` and the MCP tool `ac_download_attachments` save only the task attachments; every image embedded in the task body fails with `"error": "builder error"`. Proof: `ac get 722/82293 --download-attachments --json` and `ac_download_attachments {"ref":"722/82293"}` return four failed entries and one saved `.txt`.

The body HTML (from the local `ticket_cache` row of task 82293) shows why. ActiveCollab embeds each image as `<a href="https://<instance>/proxy.php?proxy=download_file&amp;…&amp;context=attachments&amp;id=…"><img src="/proxy.php?proxy=forward_thumbnail&…&original_file_name=image.png&width=800&height=800…" alt="image.png" /></a>`. `downloadable_assets` in `src/controller.rs` takes only the `<img src>`: a host-less URL that `reqwest` refuses to build, and a thumbnail scaled to 800×800 even when it resolves. The asset name is empty, so the file is saved as `asset_N` with no extension.

### Scope

Included: the body and comment image assets that `downloadable_assets` returns, meaning the URL, the name and HTML-entity decoding. The CLI, MCP and TUI share this function.

Excluded: other anchor hyperlinks (they stay out of the download); the asset list the TUI detail view shows (`extract_assets`).

### Decision

- An embedded image downloads its original: when the `<img>` is wrapped in an `<a href>` that is a `proxy=download_file` URL with `context=attachments`, the asset is that decoded `href`; otherwise it is the `<img src>`, decoded and resolved against the instance base URL. Its name comes from `original_file_name`, then `alt`, then the existing label fallback (owner, 2026-11-17). Not taken: downloading only the fixed thumbnail; downloading both the thumbnail and the original.

### Acceptance

- AC1: an `<img>` wrapped in a `download_file` anchor yields one downloadable asset whose URL is the decoded `href` and whose name is the image's `original_file_name`. (`verify_by: test`)
- AC2: a bare `<img>` with a relative `src` is fetched from the instance host with the token header; the `url` reported in `downloaded_attachments` stays as written in the body. (`verify_by: test`)
- AC3: `ac get 722/82293 --download-attachments --json` saves the four body images and the attachment with no `error`. (`verify_by: manual`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Outcome

Delivered in PR #47 (merge 1ebceeb). `downloadable_assets` in `src/controller.rs` takes an embedded image's original from its wrapping `download_file` anchor; `fetch_asset_bytes` in `src/client.rs` resolves a relative URL against the instance base with `url::Url::join`, and the origin gate still decides the token header after resolution.

Proof: `docker compose run --rm dev cargo test --bin ac downloadable_assets` (13 passed) covers AC1; `cargo test --bin ac fetch_asset_bytes` (7 passed) covers AC2; `cargo test -- --test-threads=1` 1880 passed; `comment_policy`, `clippy --all-targets -D warnings`, `fmt --check` and `se-gates check` pass. AC3 checked by hand on 2026-11-17 with `target/debug/ac`: four PNGs saved as `image.png`…`image_4.png` with byte sizes equal to the anchors' `size=` (86937, 50391, 12841, 14665), plus the `.txt` attachment, no error.

Not covered: a `src` holding a literal space is no longer matched (ActiveCollab percent-encodes its URLs); an image nested deeper than directly inside the anchor falls back to the thumbnail; whether ActiveCollab also lists a pasted image in `task.attachments` under another URL (not seen on 722/82293).
