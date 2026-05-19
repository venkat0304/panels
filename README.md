# panels

<p align="center">
  <img src="assets/logo.png" alt="panels" width="100" />
</p>

<p align="center">
  <a href="#install">install</a> ·
  <a href="#quick-start">quick start</a> ·
  <a href="#supported-agents">agents</a> ·
  <a href="./INTEGRATIONS.md">integrations</a> ·
  <a href="./CONFIGURATION.md">configuration</a> ·
  <a href="./SOCKET_API.md">socket api</a>
</p>

---

**panels is an agent multiplexer that lives in your terminal.**

Run many AI coding agents side by side — each in its own real terminal, organized into workspaces, tabs, and panes. See at a glance which agents are blocked, working, or done. Detach and your agents keep running; reattach from anywhere, even over SSH from your phone. No GUI app, no Electron, no wrapped view of someone else's idea of your terminal. You see the agent's actual terminal.

> **Credit:** panels is a renamed build of **[herdr](https://github.com/ogulcancelik/herdr)** by [ogulcancelik](https://github.com/ogulcancelik) ([herdr.dev](https://herdr.dev)). All of the original design and engineering is theirs. This fork keeps the AGPL-3.0 license and exists only under a different name. Please star and support the upstream project.

---

## install

Build from source (the supported path for this build):

```bash
git clone https://github.com/venkat0304/herdr panels
cd panels
cargo build --release
./target/release/panels
```

Move the single binary anywhere on your `PATH` and run `panels` from any directory:

```bash
cp target/release/panels ~/.local/bin/
```

Requires Linux or macOS. For prebuilt binaries of the original project, see upstream [herdr releases](https://github.com/ogulcancelik/herdr/releases).

## quick start

```bash
panels
```

By default `panels` launches or attaches to one background session server. Press `ctrl+b q` to detach the client — agents keep running. Use `panels server stop` to stop the default server, or `--no-session` for single-process mode.

Named sessions are separate persistent servers, each with its own panes, tabs, workspaces, and state, sharing one global config file:

```bash
panels session list
panels session attach work
panels session stop work
panels session delete side-project
```

First steps inside the app:

1. press `n` to create a workspace
2. run an agent in the root pane
3. press `ctrl+b` to enter navigate mode
4. use `v` or `-` to split panes, or `c` for a new tab
5. watch the sidebar for blocked / working / done states

On first run panels shows a short onboarding flow. Fresh sessions start in **navigate mode**; restored sessions land in terminal mode.

## agent awareness

The sidebar shows which agents are blocked, working, or done. Workspaces roll up to their most urgent state so you can scan the whole list at a glance.

- 🔴 **blocked** — agent needs input or approval
- 🟡 **working** — agent is actively running
- 🔵 **done** — work finished, not yet looked at
- 🟢 **idle** — done and seen

Detection works by reading the foreground process and terminal output — zero config, no hooks required. Agents that expose hooks get more robust state reporting through the socket API integration.

## persistence

Start panels on your desktop or a server. Run agents, split panes, work. Press `ctrl+b q` to detach — close the terminal, close the laptop, agents keep running. Open a new terminal, run `panels`, and you're back: same session, same panes, same agents.

Reach a remote session from anywhere:

```bash
ssh you@yourserver
panels
```

Or attach through SSH from your local terminal:

```bash
panels --remote workbox
panels --remote ssh://you@yourserver:2222
```

## agents can drive panels too

A local Unix socket lets agents create workspaces, split panes, spawn helpers, read output, and wait for state changes:

```bash
panels workspace create --cwd ~/project --label "api"
panels tab create --label "logs"
panels pane split 1-1 --direction right
panels pane run 1-2 "npm test"
panels wait agent-status 1-1 --status done
panels pane read 1-2 --source recent --lines 50
panels pane read 1-2 --source visible --ansi
```

Full reference: [`SOCKET_API.md`](./SOCKET_API.md) and [`SKILL.md`](./SKILL.md).

## supported agents

Automatic detection works out of the box via process-name matching plus terminal-output heuristics.

| agent | idle / done | working | blocked |
|-------|-------------|---------|---------|
| [pi](https://pi.dev) | ✓ | ✓ | partial |
| [claude code](https://docs.anthropic.com/en/docs/claude-code) | ✓ | ✓ | ✓ |
| [codex](https://github.com/openai/codex) | ✓ | ✓ | ✓ |
| [droid](https://factory.ai) | ✓ | ✓ | ✓ |
| [amp](https://ampcode.com) | ✓ | ✓ | ✓ |
| [opencode](https://github.com/anomalyco/opencode) | ✓ | ✓ | ✓ |

Detected but not fully tested: gemini cli, cursor agent, cline, kimi, github copilot cli. Any other agent still works — panels remains a full terminal multiplexer with workspaces, panes, and tiling.

### direct integrations

The pi, claude code, codex, and opencode integrations forward semantic state over the socket API:

```bash
panels integration install pi
panels integration install claude
panels integration install codex
panels integration install opencode
```

See [`INTEGRATIONS.md`](./INTEGRATIONS.md).

## keybindings

Press `ctrl+b` to enter navigate mode.

| key | action |
|-----|--------|
| `n` | new workspace |
| `shift+n` | rename workspace |
| `shift+d` | close workspace |
| `c` | new tab |
| `v` / `-` | split pane |
| `x` | close pane |
| `b` | toggle sidebar |
| `f` | zoom pane |
| `r` | resize mode |
| `q` | detach (quit client) |

Resize mode: `h`/`l` width, `j`/`k` height, `esc` to exit. Mouse is supported throughout — click, drag borders, select to copy, right-click menus. Custom command keybindings can launch shell helpers or temporary panes; see [`CONFIGURATION.md`](./CONFIGURATION.md).

## configuration

Config file: `~/.config/panels/config.toml`

```bash
panels --default-config   # print the full default config
```

There is also an in-app settings screen for theme, sound, and toast preferences, plus 17 built-in themes (catppuccin, tokyo night, gruvbox, one, solarized, kanagawa, rosé pine, vesper, and light variants).

## logs

panels writes logs under `~/.config/panels/`:

```text
~/.config/panels/panels.log
~/.config/panels/panels-client.log
~/.config/panels/panels-server.log
```

Logs rotate automatically. Raise the level only when needed:

```bash
PANELS_LOG=panels=debug panels
```

## docs

- [`CONFIGURATION.md`](./CONFIGURATION.md) — keybindings, themes, notifications, environment variables
- [`INTEGRATIONS.md`](./INTEGRATIONS.md) — pi, claude code, codex, opencode integrations
- [`SKILL.md`](./SKILL.md) — reusable agent skill
- [`SOCKET_API.md`](./SOCKET_API.md) — socket protocol and CLI reference

## testing

```bash
just test       # unit tests
just check      # lint + tests + maintenance script tests
```

## pi, ghostty, and shift+enter

panels does not require or install terminal keybinds for pi. Ghostty does not ship a default `shift+enter` keybind; if `shift+enter=text:\n` lines exist in your ghostty config they were added by other tooling (commonly claude code) and collapse shift+enter into legacy bytes. If shift+enter behaves oddly in pi inside panels, remove those custom terminal keybinds and retest before filing it as a panels bug.

## license

AGPL-3.0 — free to use, modify, and distribute. Modified versions must be open-sourced under the same license. This requirement is inherited from the upstream [herdr](https://github.com/ogulcancelik/herdr) project, whose copyright and authorship are gratefully acknowledged.
