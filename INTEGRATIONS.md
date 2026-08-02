# integrations

panels works without any hook or plugin setup.

out of the box, it detects supported agents automatically by combining foreground process detection with screen heuristics. that is enough to give you workspace awareness with zero configuration.

when an agent exposes hooks or plugins, the more robust path is to forward semantic state to panels over the local socket api. the built-in integrations in this document do exactly that.

if you want to inspect the exact files panels installs, they are versioned in this repo:

- [pi extension](./src/integration/assets/pi/panels-agent-state.ts)
- [claude code hook](./src/integration/assets/claude/panels-agent-state.sh)
- [codex hook](./src/integration/assets/codex/panels-agent-state.sh)
- [opencode plugin](./src/integration/assets/opencode/panels-agent-state.js)

## how panels uses integrations

panels uses a hybrid model:

- **process detection** owns pane identity, liveness, and "the process is gone"
- **agent integrations** report semantic state like `working`, `blocked`, and `idle` over the local socket api when the tool exposes those events
- **screen heuristics** remain the fallback for gaps, unsupported tools, or incomplete hook surfaces

that means hooks/plugins do **not** become the source of truth for pane ownership. they enrich state reporting; they do not replace process detection.

## install commands

```bash
panels integration install pi
panels integration install claude
panels integration install codex
panels integration install opencode
```

## uninstall commands

```bash
panels integration uninstall pi
panels integration uninstall claude
panels integration uninstall codex
panels integration uninstall opencode
```

## pi

install:

```bash
panels integration install pi
```

this writes the bundled pi extension to:

```text
~/.pi/agent/extensions/panels-agent-state.ts
```

if `PI_CODING_AGENT_DIR` is set, panels uses that agent directory instead and writes to:

```text
$PI_CODING_AGENT_DIR/extensions/panels-agent-state.ts
```

`~` is expanded in `PI_CODING_AGENT_DIR`.

pi is the cleanest integration. it already has an authoritative hook model, so panels can get direct state reports over the socket api without guessing as much from the terminal.

bundled source: [`src/integration/assets/pi/panels-agent-state.ts`](./src/integration/assets/pi/panels-agent-state.ts)

uninstall:

```bash
panels integration uninstall pi
```

this removes the same extension path panels would install, using `PI_CODING_AGENT_DIR` when it is set.

## claude code

install:

```bash
panels integration install claude
```

this:

- writes the hook script to `~/.claude/hooks/panels-agent-state.sh`
- updates `~/.claude/settings.json`

if `CLAUDE_CONFIG_DIR` is set, panels uses that directory instead, for example `$CLAUDE_CONFIG_DIR/hooks/panels-agent-state.sh` and `$CLAUDE_CONFIG_DIR/settings.json`. `~` is expanded in `CLAUDE_CONFIG_DIR`.

bundled source: [`src/integration/assets/claude/panels-agent-state.sh`](./src/integration/assets/claude/panels-agent-state.sh)

current hook mapping:

- `UserPromptSubmit` → `working`
- `PreToolUse` → `working`
- `PermissionRequest` → `blocked`
- `PostToolUse` → `working`
- `PostToolUseFailure` → `working`
- `SubagentStop` → `working`
- `Stop` → `idle`
- `SessionEnd` → `release`

notes:

- claude code hooks also run inside subagents. panels treats subagent `working` and `blocked` reports as real pane state.
- subagent stop/release events are converted to `working` by the bundled hook script so a completed subagent does not make the parent claude pane look idle.
- `PostToolUse` and `PostToolUseFailure` move the pane back to `working` after a permissioned tool call resolves.

uninstall:

```bash
panels integration uninstall claude
```

this removes the same hook path and settings entries panels would install, using `CLAUDE_CONFIG_DIR` when it is set.

## codex

install:

```bash
panels integration install codex
```

this:

- writes the hook script to `~/.codex/panels-agent-state.sh`
- updates `~/.codex/hooks.json`
- ensures `hooks = true` under `[features]` in `~/.codex/config.toml`
- migrates the deprecated top-level `[features] codex_hooks = true` setting to `hooks = true`

if `CODEX_HOME` is set, panels uses that directory instead, for example `$CODEX_HOME/panels-agent-state.sh`, `$CODEX_HOME/hooks.json`, and `$CODEX_HOME/config.toml`. `~` is expanded in `CODEX_HOME`.

bundled source: [`src/integration/assets/codex/panels-agent-state.sh`](./src/integration/assets/codex/panels-agent-state.sh)

current hook mapping:

- `SessionStart` → `idle`
- `UserPromptSubmit` → `working`
- `PreToolUse` → `working`
- `Stop` → `idle`

notes:

- codex does **not** currently expose a permission-specific hook like claude or opencode, so `blocked` still depends on panels's normal heuristics.
- codex currently renders hook lifecycle messages in its own tui, for example `Running SessionStart hook` and `SessionStart hook (completed)`.
- that noise is an upstream codex limitation, not a panels-specific issue.
- codex has a `suppressOutput` field in its hook output schema, but it is currently not effective for suppressing those tui lifecycle lines.

uninstall:

```bash
panels integration uninstall codex
```

this:

- removes the same hook path panels would install, using `CODEX_HOME` when it is set
- removes panels-owned hook entries from the matching `hooks.json`
- intentionally leaves the matching `config.toml` alone

that last point is deliberate: panels does **not** try to guess whether `hooks = true` is still needed for some other codex hook setup.

## opencode

install:

```bash
panels integration install opencode
```

this writes the bundled plugin to:

```text
~/.config/opencode/plugins/panels-agent-state.js
```

bundled source: [`src/integration/assets/opencode/panels-agent-state.js`](./src/integration/assets/opencode/panels-agent-state.js)

current plugin mapping:

- `permission.asked` → `blocked`
- `permission.replied: once|always` → `working`
- `permission.replied: reject` → `idle`
- `question.asked` → `blocked`
- `question.replied` → `working`
- `question.rejected` → `idle`
- `session.status: busy|retry` → `working`
- `session.status: idle` → `idle`
- `session.idle` → `idle`

notes:

- opencode has the richest event surface of the currently supported integrations.
- panels intentionally does **not** guess that `session.deleted` means process exit. process detection still owns liveness and pane identity.

uninstall:

```bash
panels integration uninstall opencode
```

this removes:

```text
~/.config/opencode/plugins/panels-agent-state.js
```

## amp

panels does not currently install an Amp plugin.

Amp's public plugin API exposes lifecycle and tool-call hooks, but not passive permission/request-blocked events. A Panels Amp plugin that only reports `idle` and `working` would take hook authority for the pane and mask Panels's existing screen heuristics for Amp `blocked` states. Until Amp exposes permission state as an observable plugin event, Amp remains heuristic-only in Panels.

## known limitations

- these integrations only activate inside panels-managed panes.
- if an agent has an incomplete hook surface, panels falls back to process detection and screen heuristics rather than inventing lease or ttl behavior.
- codex currently shows hook lifecycle chatter in its own tui until upstream adds a real silent mode.

## troubleshooting

if an install command succeeds but you do not see improved state reporting:

1. make sure you launched the agent inside a panels pane
2. restart the agent session so it picks up the new hook/plugin config
3. verify the expected config file was written to the path above
4. remember that unsupported transitions still fall back to panels's built-in heuristics
