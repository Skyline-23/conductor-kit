# conductor-kit

A global skills pack and lightweight Go helper for **Codex CLI** and **Claude Code**.
It enforces a consistent orchestration loop (search -> plan -> execute -> verify -> cleanup) and supports parallel delegation via MCP tool calls.

**Language**: English | [한국어](README.ko.md)

## Quick start (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install conductor-kit

# Homebrew post_install links skills/commands into Codex + Claude
# Re-run if needed:
conductor install --mode link --repo $(brew --prefix)/share/conductor-kit --force
```

## Manual install
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

## Requirements
- A host CLI: Codex CLI or Claude Code (skills/commands run inside these hosts).
- For delegation, install at least one agent CLI on PATH: `codex`, `claude`, or `gemini` (match your config roles).
- Go 1.23+ (only if building from source).
- MCP registration:
  - Codex CLI: `codex mcp add ...`
  - Claude Code: `~/.claude/.mcp.json` (see below)

## What you get
- **Skill**: `conductor` (`skills/conductor/SKILL.md`)
- **Commands**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go helper**: `conductor` binary for install + MCP server + delegation tools
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
- `conductor.run` with `{ "role": "oracle", "prompt": "<task>" }`
- Run multiple `conductor.run` tool calls in parallel (host handles concurrency)
- `conductor.run_batch` with `{ "roles": "oracle,librarian,explore", "prompt": "<task>" }`

Note: Delegation tools are MCP-only; no CLI subcommands are provided.


## Model setup (roles)
`~/.conductor-kit/conductor.json` controls role -> CLI/model routing (installed from `config/conductor.json`).
The repo file is the default template; `conductor install` links/copies it into `~/.conductor-kit/`.
If `model` is empty, no model flag is passed and the CLI default is used.

Key fields:
- `defaults.timeout_ms` / `defaults.max_parallel` / `defaults.retry` / `defaults.retry_backoff_ms`: runtime defaults
- `defaults.log_prompt`: store prompt text in run history (default: false)
- `roles.<name>.cli`: executable to run (must be on PATH)
- `roles.<name>.args`: argv template; include `{prompt}` where the prompt should go
- `roles.<name>.model_flag`: model flag (e.g. `-m` for codex, `--model` for claude/gemini)
- `roles.<name>.model`: default model string (optional)
- `roles.<name>.models`: fan-out list for `conductor.run_batch` (string or `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: optional reasoning config (codex supports `-c model_reasoning_effort`)
- `roles.<name>.env` / `roles.<name>.cwd`: env/cwd overrides
- `roles.<name>.timeout_ms` / `roles.<name>.max_parallel` / `roles.<name>.retry` / `roles.<name>.retry_backoff_ms`: role overrides

Example:
```json
{
  "roles": {
    "oracle": {
      "cli": "codex",
      "args": ["exec", "{prompt}"],
      "model_flag": "-m",
      "model": "gpt-5.2-codex",
      "reasoning_flag": "-c",
      "reasoning_key": "model_reasoning_effort",
      "reasoning": "high"
    }
  }
}
```

Overrides:
- `conductor.run` with `{ "role": "<role>", "model": "<model>", "reasoning": "<level>", "prompt": "<task>" }`
- `conductor.run_batch` with `{ "roles": "<role(s)>", "model": "<model[,model]>", "reasoning": "<level>", "prompt": "<task>" }`
  (model overrides apply only to `roles` mode, not `agents`)
- `conductor.run_batch` with `{ "config": "/path/to/conductor.json", "prompt": "<task>" }` or `CONDUCTOR_CONFIG=/path/to/conductor.json`
Tip: customize `~/.conductor-kit/conductor.json` directly; re-run `conductor install` only if you want to reset to defaults.
Schema: `config/conductor.schema.json` (optional for tooling).

## Diagnostics
- `conductor config-validate` (validates `~/.conductor-kit/conductor.json`)
- `conductor doctor` (checks config + CLI availability)

## Observability
- `conductor.run_history` with `{ "limit": 20 }`
- `conductor.run_info` with `{ "run_id": "<id>" }`

## MCP tools (recommended for tool-calling UI)
```bash
codex mcp add conductor -- conductor mcp
```
Tools:
- `conductor.run`
- `conductor.run_batch`
- `conductor.run_history`
- `conductor.run_info`

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
