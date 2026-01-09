# conductor-kit

A global skills pack and lightweight Go helper for **Codex CLI** and **Claude Code**.
It enforces a consistent orchestration loop (search -> plan -> execute -> verify -> cleanup) and supports parallel delegation via MCP tool calls.

**Language**: English | [한국어](README.ko.md)

## Quick start (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit

# Homebrew post_install links skills/commands into Codex + Claude
# Re-run if needed:
conductor install --mode link --repo "$(brew --prefix)/Caskroom/conductor-kit/$(brew list --cask --versions conductor-kit | awk '{print $2}')" --force
```

## Manual install
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

Project-local install:
```bash
conductor install --mode link --repo ~/.conductor-kit --project
```

## Requirements
- A host CLI: Codex CLI or Claude Code (skills/commands run inside these hosts).
- For delegation, install at least one agent CLI on PATH: `codex`, `claude`, or `gemini` (match your config roles).
- Go 1.23+ (only if building from source).
- Homebrew cask install is macOS-only (Linux users should use manual install).
- MCP registration:
  - Codex CLI: `codex mcp add ...`
  - Claude Code: `~/.claude/.mcp.json` (see below)

## What you get
- **Skill**: `conductor` (`skills/conductor/SKILL.md`)
- **Commands**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go helper**: `conductor` binary for install + MCP server + delegation tools
- **Optional runtime**: local daemon for queued/approved async runs
- **Config**: `~/.conductor-kit/conductor.json` (role -> CLI/model mapping)

## Usage
### 1) Use the skill
Trigger by saying: `conductor`, `ultrawork` / `ulw`, or “orchestration”.

### 2) Use commands
Claude Code (slash commands):
- `/conductor-plan`
- `/conductor-search`
- `/conductor-implement`
- `/conductor-release`
- `/conductor-ultrawork`

Codex CLI (custom prompts):
- `/prompts:conductor-plan`
- `/prompts:conductor-search`
- `/prompts:conductor-implement`
- `/prompts:conductor-release`
- `/prompts:conductor-ultrawork`
Prompts are installed in `~/.codex/prompts` (or `$CODEX_HOME/prompts`).

### 3) Parallel delegation (MCP-only)
Codex CLI:
```bash
codex mcp add conductor -- conductor mcp
```

Claude Code (`~/.claude/.mcp.json`):
```json
{
  "mcpServers": {
    "conductor": {
      "command": "conductor",
      "args": ["mcp"]
    }
  }
}
```

Then use tools:
- `conductor.run` with `{ "role": "oracle", "prompt": "<task>" }` (async; returns run_id)
- `conductor.run_batch_async` with `{ "roles": "oracle,librarian,explore", "prompt": "<task>" }`
- Poll status: `conductor.run_status` with `{ "run_id": "<id>" }`

Note: Delegation tools are MCP-only; CLI subcommands are only for daemon setup.

### 4) Async delegation (default)
- Start async run: `conductor.run` or `conductor.run_async` with `{ "role": "oracle", "prompt": "<task>" }`
- Batch async: `conductor.run_batch_async` with `{ "roles": "oracle,librarian", "prompt": "<task>" }`
- Poll status: `conductor.run_status` with `{ "run_id": "<id>" }`
- Wait for completion: `conductor.run_wait` (note: host tool-call timeouts may cut this off)
- Cancel: `conductor.run_cancel` with `{ "run_id": "<id>", "force": false }`

### 5) Local daemon (queue + approvals, optional)
Start a local daemon to queue async runs, enforce approvals, and expose run listings.

```bash
conductor daemon --mode start --detach
conductor daemon --mode status
conductor daemon --mode stop
```

When the daemon is running, async MCP tools automatically route through it (unless `no_daemon: true`).
New tools:
- `conductor.queue_list` with `{ "status": "queued|running|awaiting_approval", "limit": 50 }`
- `conductor.approval_list`
- `conductor.approval_approve` with `{ "run_id": "<id>" }`
- `conductor.approval_reject` with `{ "run_id": "<id>" }`
- `conductor.daemon_status`

Optional flags for async tools:
- `require_approval: true` (force approval even if defaults say no)
- `mode: "string"` (override mode hash for batching)
- `no_daemon: true` (bypass daemon)
- `summary_only: true` (hide stdout/stderr and return read/changed file summaries)

Set `CONDUCTOR_DAEMON_URL` to target a remote daemon.


## Model setup (roles)
`~/.conductor-kit/conductor.json` controls role -> CLI/model routing (installed from `config/conductor.json`).
The repo file is the default template; `conductor install` links/copies it into `~/.conductor-kit/`.
If `model` is empty, no model flag is passed and the CLI default is used.

Key fields:
- `defaults.timeout_ms` / `defaults.idle_timeout_ms` / `defaults.max_parallel` / `defaults.retry` / `defaults.retry_backoff_ms`: runtime defaults
- `defaults.log_prompt`: store prompt text in run history (default: false)
- `defaults.summary_only`: hide raw stdout/stderr in MCP results and return read/changed file summaries only
- `daemon.host` / `daemon.port`: local daemon bind address
- `daemon.max_parallel`: daemon worker limit (defaults to `defaults.max_parallel`)
- `daemon.queue.on_mode_change`: `none` | `cancel_pending` | `cancel_running`
- `daemon.approval.required`: force approvals for all runs
- `daemon.approval.roles` / `daemon.approval.agents`: require approval for specific roles or CLI agents
- `roles.<name>.cli`: executable to run (must be on PATH)
- `roles.<name>.args`: argv template; include `{prompt}` where the prompt should go (optional for codex/claude/gemini)
- `roles.<name>.model_flag`: model flag (optional for codex/claude/gemini)
- `roles.<name>.model`: default model string (optional)
- `roles.<name>.models`: fan-out list for `conductor.run_batch` (string or `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: optional reasoning config (codex supports `-c model_reasoning_effort`)
- `roles.<name>.env` / `roles.<name>.cwd`: env/cwd overrides
- `roles.<name>.ready_cmd` / `ready_args` / `ready_timeout_ms`: optional preflight command to confirm the CLI is logged in (prevents interactive login prompts)
- `roles.<name>.timeout_ms` / `roles.<name>.idle_timeout_ms` / `roles.<name>.max_parallel` / `roles.<name>.retry` / `roles.<name>.retry_backoff_ms`: role overrides

Defaults (if omitted):
- `codex`: args `["exec","{prompt}"]`, model flag `-m`, reasoning flag `-c model_reasoning_effort`
- `claude`: args `["-p","{prompt}"]`, model flag `--model`
- `gemini`: args `["{prompt}"]`, model flag `--model`

Template defaults (CLI-native model names):
- `oracle`: codex `gpt-5.2-codex` + `reasoning: "medium"`
- `librarian`: gemini `gemini-3-flash-preview`
- `explore`: gemini `gemini-3-flash-preview`
- `frontend-ui-ux-engineer`: gemini `gemini-3-pro-preview`
- `document-writer`: gemini `gemini-3-flash-preview`
- `multimodal-looker`: gemini `gemini-3-flash-preview`

Minimal example:
```json
{
  "roles": {
    "oracle": {
      "cli": "codex"
    }
  }
}
```

Overrides:
- `conductor.run` with `{ "role": "<role>", "model": "<model>", "reasoning": "<level>", "timeout_ms": 120000, "idle_timeout_ms": 30000, "prompt": "<task>" }` (async)
- `conductor.run_batch_async` with `{ "roles": "<role(s)>", "model": "<model[,model]>", "reasoning": "<level>", "timeout_ms": 120000, "idle_timeout_ms": 30000, "prompt": "<task>" }`
- `conductor.run_batch_async` with `{ "config": "/path/to/conductor.json", "prompt": "<task>" }` or `CONDUCTOR_CONFIG=/path/to/conductor.json`
Tip: customize `~/.conductor-kit/conductor.json` directly; re-run `conductor install` only if you want to reset to defaults.
Schema: `config/conductor.schema.json` (optional for tooling).

## Setup helpers
- `conductor settings` (TUI wizard; use `--no-tui` for plain prompts)
- `conductor settings --list-models --cli codex` (show available models)
- `conductor settings --role <role> --cli <cli> --model <model> --reasoning <effort>`
- `conductor login codex|claude|gemini` (runs CLI login flow)
- `conductor uninstall` (removes installed skills/commands/config from home)

## Project-local overrides
- Place a local config at `./.conductor-kit/conductor.json` to override the global config.
- Use `conductor install --project` to link skills/commands into `./.claude` and prompts into `./.codex`.

## Diagnostics
- `conductor config-validate` (validates `~/.conductor-kit/conductor.json`)
- `conductor doctor` (checks config + CLI availability + model name sanity)

## Uninstall (Homebrew)
```bash
brew uninstall --cask conductor-kit
```
This runs `conductor uninstall --force` via a cask uninstall hook to clean user-level installs.

## Observability
- `conductor.run_history` with `{ "limit": 20 }`
- `conductor.run_info` with `{ "run_id": "<id>" }`
- `conductor.queue_list` with `{ "status": "queued|running|awaiting_approval" }`
- `conductor.approval_list` to list pending approvals

## Optional MCP bundles
```bash
# Claude Code (.claude/.mcp.json)
conductor mcp-bundle --host claude --bundle core --repo /path/to/conductor-kit --out .claude/.mcp.json

# Codex CLI (prints codex mcp add commands)
conductor mcp-bundle --host codex --bundle core --repo /path/to/conductor-kit
```
Bundle config lives at `~/.conductor-kit/mcp-bundles.json` (installed by `conductor install`).

## MCP tools (recommended for tool-calling UI)
```bash
codex mcp add conductor -- conductor mcp
```
Tools:
- `conductor.run`
- `conductor.run_batch`
- `conductor.run_async`
- `conductor.run_batch_async`
- `conductor.run_status`
- `conductor.run_wait`
- `conductor.run_cancel`
- `conductor.run_history`
- `conductor.run_info`
- `conductor.queue_list` (daemon)
- `conductor.approval_list` (daemon)
- `conductor.approval_approve` (daemon)
- `conductor.approval_reject` (daemon)
- `conductor.daemon_status` (daemon)
Note: if the host supplies a progress token, Conductor emits MCP progress notifications during batch/async runs.
Note: `conductor.run_batch` is synchronous; prefer `conductor.run_batch_async` to avoid host tool-call timeouts.
Note: `role: auto` is not supported; specify roles explicitly.

## Repo layout
```
conductor-kit/
  cmd/conductor/         # Go helper CLI
  commands/              # Codex + Claude commands
  config/                # default role/model config
  skills/conductor/      # main skill
```

## License
MIT
