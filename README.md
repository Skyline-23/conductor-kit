# conductor-kit

A global skills pack and lightweight Go helper for **Codex CLI** and **Claude Code**, with optional **OpenCode** command/skill install.
It enforces a consistent orchestration loop (search -> plan -> execute -> verify -> cleanup) and supports CLI MCP bridges for delegation.

**Language**: English | [한국어](README.ko.md)

## Quick start (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit

# Homebrew post_install links skills/commands and registers MCP for Codex + Claude + OpenCode
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

Project-local install (links into .claude/.codex/.opencode):
```bash
conductor install --mode link --repo ~/.conductor-kit --project
```

## Requirements
- A host CLI: Codex CLI, Claude Code, or OpenCode (commands/skills load inside these hosts).
- For delegation, install at least one agent CLI on PATH: `codex`, `claude`, or `gemini` (match your config roles).
- Go 1.23+ (only if building from source).
- Homebrew cask install is macOS-only (Linux users should use manual install).
- MCP registration: `conductor install` auto-registers Codex + Claude + OpenCode (gemini-cli + claude-cli + codex-cli bundles).

## What you get
- **Skill**: `conductor` (`skills/conductor/SKILL.md`)
- **Commands**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go helper**: `conductor` binary for install + MCP bridge servers
- **CLI MCP bridges**: gemini/claude/codex CLI wrappers
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

OpenCode (slash commands):
- `/conductor-plan`
- `/conductor-search`
- `/conductor-implement`
- `/conductor-release`
- `/conductor-ultrawork`
Commands are installed in `~/.config/opencode/command` (or `./.opencode/command`).
Skills are installed in `~/.config/opencode/skill` (or `./.opencode/skill`).

### 3) CLI MCP bridges
Codex CLI:
```bash
codex mcp add gemini-cli -- conductor mcp-gemini
codex mcp add claude-cli -- conductor mcp-claude
codex mcp add codex-cli -- conductor mcp-codex
```

Claude Code (`~/.claude/.mcp.json`):
```json
{
  "mcpServers": {
    "gemini-cli": {
      "command": "conductor",
      "args": ["mcp-gemini"]
    },
    "claude-cli": {
      "command": "conductor",
      "args": ["mcp-claude"]
    },
    "codex-cli": {
      "command": "conductor",
      "args": ["mcp-codex"]
    }
  }
}
```

Then use tools:
- `gemini.prompt` with `{ "prompt": "<task>", "model": "gemini-2.5-flash" }`
- `gemini.batch` with `{ "prompt": "<task>", "models": "gemini-2.5-flash,gemini-2.5-pro" }`
- `gemini.auth_status`
- `claude.prompt` with `{ "prompt": "<task>", "model": "claude-3-5-sonnet" }`
- `claude.batch` with `{ "prompt": "<task>", "models": "claude-3-5-sonnet,claude-3-5-haiku" }`
- `claude.auth_status`
- `codex.prompt` with `{ "prompt": "<task>", "model": "gpt-5.2-codex" }`
- `codex.batch` with `{ "prompt": "<task>", "models": "gpt-5.2-codex,gpt-4.1" }`
- `codex.auth_status`

Note: Gemini MCP uses the Gemini CLI login (no gcloud ADC).
Note: Claude MCP uses the Claude CLI login (permission-mode defaults to dontAsk).
Note: Codex MCP uses the Codex CLI login (codex exec --json).
Note: CLI bridges are MCP-only; CLI subcommands cover install/config/diagnostics.

## Model setup (roles)
`~/.conductor-kit/conductor.json` controls role -> CLI/model routing (installed from `config/conductor.json`).
The repo file is the default template; `conductor install` links/copies it into `~/.conductor-kit/`.
If `model` is empty, no model flag is passed and the CLI default is used.

Key fields:
- `defaults.timeout_ms` / `defaults.idle_timeout_ms` / `defaults.max_parallel` / `defaults.retry` / `defaults.retry_backoff_ms`: runtime defaults
- `defaults.log_prompt`: store prompt text in run history (default: false)
- `roles.<name>.cli`: executable to run (must be on PATH)
- `roles.<name>.args`: argv template; include `{prompt}` where the prompt should go (optional for codex/claude/gemini)
- `roles.<name>.model_flag`: model flag (optional for codex/claude/gemini)
- `roles.<name>.model`: default model string (optional)
- `roles.<name>.models`: fan-out list for batch usage (string or `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: optional reasoning config (codex supports `-c model_reasoning_effort`)
- `roles.<name>.env` / `roles.<name>.cwd`: env/cwd overrides
- `conductor status` checks CLI auth state using each CLI's local storage (codex: `~/.codex/auth.json`; gemini: `~/.gemini/oauth_creds.json` or keychain; claude: keychain `Claude Code-credentials`) and never invokes the CLI
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
- Use `CONDUCTOR_CONFIG=/path/to/conductor.json` to point to a custom config
Tip: customize `~/.conductor-kit/conductor.json` directly; re-run `conductor install` only if you want to reset to defaults.
Schema: `config/conductor.schema.json` (optional for tooling).

## Setup helpers
- `conductor settings` (TUI wizard; use `--no-tui` for plain prompts)
- `conductor settings --list-models --cli codex` (show available models)
- `conductor settings --role <role> --cli <cli> --model <model> --reasoning <effort>`
- `conductor status` (check CLI availability and readiness)
- `conductor uninstall` (removes installed skills/commands/config from home, including OpenCode)

## Project-local overrides
- Place a local config at `./.conductor-kit/conductor.json` to override the global config.
- Use `conductor install --project` to link skills/commands into `./.claude`, prompts into `./.codex`, and OpenCode assets into `./.opencode`.

## Diagnostics
- `conductor config-validate` (validates `~/.conductor-kit/conductor.json`)
- `conductor doctor` (checks config + CLI availability + model name sanity)

## Uninstall (Homebrew)
```bash
brew uninstall --cask conductor-kit
```
This runs `conductor uninstall --force` via a cask uninstall hook to clean user-level installs.

## Observability
- `conductor status` (CLI auth and availability)
- CLI MCP bridges return raw CLI output (`gemini.prompt`, `claude.prompt`, `codex.prompt`)

## Optional MCP bundles
```bash
# Claude Code (.claude/.mcp.json)
conductor mcp-bundle --host claude --bundle gemini-cli --repo /path/to/conductor-kit --out .claude/.mcp.json
conductor mcp-bundle --host claude --bundle claude-cli --repo /path/to/conductor-kit --out .claude/.mcp.json
conductor mcp-bundle --host claude --bundle codex-cli --repo /path/to/conductor-kit --out .claude/.mcp.json

# Codex CLI (prints codex mcp add commands)
conductor mcp-bundle --host codex --bundle gemini-cli --repo /path/to/conductor-kit
conductor mcp-bundle --host codex --bundle claude-cli --repo /path/to/conductor-kit
conductor mcp-bundle --host codex --bundle codex-cli --repo /path/to/conductor-kit
```
Bundle config lives at `~/.conductor-kit/mcp-bundles.json` (installed by `conductor install`).

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
