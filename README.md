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
- `codex` CLI if you want to register MCP tools (`codex mcp add ...`).

## What you get
- **Skill**: `conductor` (`skills/conductor/SKILL.md`)
- **Commands**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go helper**: `conductor` binary for install + MCP server + delegation tools
- **Config**: `~/.conductor-kit/conductor.json` (role -> CLI/model mapping)

## Usage
### 1) Use the skill
Trigger by saying: `conductor`, `ultrawork` / `ulw`, or “orchestration”.

### 2) Use commands (Codex + Claude)
- `/conductor-plan`
- `/conductor-search`
- `/conductor-implement`
- `/conductor-release`
- `/conductor-ultrawork`

### 3) Parallel delegation (MCP-only)
```bash
codex mcp add conductor -- conductor mcp
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
- `roles.<name>.cli`: executable to run (must be on PATH)
- `roles.<name>.args`: argv template; include `{prompt}` where the prompt should go
- `roles.<name>.model_flag`: model flag (e.g. `-m` for codex, `--model` for claude/gemini)
- `roles.<name>.model`: default model string (optional)
- `roles.<name>.models`: fan-out list for `conductor.run_batch` (string or `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: optional reasoning config (codex supports `-c model_reasoning_effort`)

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

## MCP tools (recommended for tool-calling UI)
```bash
codex mcp add conductor -- conductor mcp
```
Tools:
- `conductor.run`
- `conductor.run_batch`

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
