# active-collab-agent

Gives AI coding agents your self-hosted [ActiveCollab](https://activecollab.com)
tasks through the [`ac` CLI](https://github.com/ejklock/active-collab-cli):

- **OpenCode**: registers the `ac mcp` server (tools `ac_read`, `ac_write`,
  `ac_download_attachments`) and the `active-collab` skill.
- **Pi**: adds the `active-collab` skill, which drives `ac` with `--json`.

Unofficial; not affiliated with ActiveCollab.

## Install

Install `ac` 0.12.0 or later first and register your instance:

```sh
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.sh | sh
ac setup add
```

OpenCode, in `opencode.json`:

```json
{ "plugin": ["active-collab-agent"] }
```

Pi:

```sh
pi install npm:active-collab-agent
```

A key in your own `mcp` config named `active-collab` wins over the one this
plugin registers.

Claude Code and Codex install the same tools from the repository's plugin
marketplace; see the [project README](https://github.com/ejklock/active-collab-cli#plugins).

## License

MIT
