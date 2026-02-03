# conductor-kit

A global skills pack for **Codex CLI**, **Claude Code**, and **Gemini CLI** with a unified MCP server for cross-CLI orchestration.

**Language**: English | [한국어](README.ko.md)

## Quickstart

1. Install one or more supported CLIs.
2. Install conductor-kit.
3. Run `conductor install` and verify with `conductor status`.

## Install

npx (fastest):
```bash
npx conductor-kit install
```

Homebrew (macOS):
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit
conductor install
```

npm global:
```bash
npm install -g conductor-kit
conductor install
```

Build from source:
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install
```

Verify:
```bash
conductor doctor
conductor status
```

## Supported CLIs

| CLI | Install | Auth |
|-----|---------|------|
| **Claude Code** | `npm install -g @anthropic-ai/claude-code` | `claude` (follow prompts) |
| **Codex CLI** | `npm install -g @openai/codex` | `codex --login` |
| **Gemini CLI** | `npm install -g @anthropic-ai/gemini-cli` | `gemini auth` |

## MCP Setup

Claude Code `~/.claude/mcp.json`:
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

Codex CLI:
```bash
codex mcp add conductor -- conductor mcp
```

OpenCode:
```bash
opencode mcp add conductor -- conductor mcp
```

Config paths:
- Codex: `~/.codex/config.toml` (or project `.codex/config.toml`)
- OpenCode: `~/.config/opencode/opencode.json` (or project `opencode.json`)

Bridge mode notes:
- `conductor mcp` runs a unified MCP server over stdio for any MCP client.
- Bridges Codex (`codex mcp-server`) and Claude tools (`claude mcp serve`); Claude prompts run via native CLI.
- Claude tool approval is handled by the MCP client.
- Codex `mcp-server` inherits global config overrides (approvals/sandboxing live in Codex config).
- `conductor mcp` warns and continues if an upstream MCP server is unavailable (`CONDUCTOR_BRIDGE_STRICT=1` to fail fast).

Optional MCP bundles:
- Templates live in `config/mcp-bundles.json`.
- Enable in `~/.conductor-kit/mcp-bundles.json` and render:
```bash
conductor mcp-bundle --host claude --bundle conductor
conductor mcp-bundle --host codex --bundle conductor
```

## Usage

Load the skill:
```bash
# Claude Code
claude
> Load the conductor skill
> sym
```

```bash
# Codex CLI
codex
> Load conductor
```

Cross-CLI prompts:
```
Use the codex tool to analyze this algorithm with deep reasoning
```

```
Use the gemini tool to search the web for React 19 best practices
```

```
Use the conductor tool with role "sage" to solve this complex problem
```

Available MCP tools:

| Tool | Description | Example |
|------|-------------|---------|
| `codex` | Run Codex MCP session (bridged) | Deep reasoning, complex analysis |
| `claude` | Run Claude Code session (native CLI) | Code generation, refactoring |
| `claude__*` | Claude Code tools (bridged) | View/Edit/LS, etc. |
| `gemini` | Run Gemini CLI session | Web search, research |
| `conductor` | Role-based routing | Auto-select best CLI for task |
| `memory` | Shared memory cache | Store/retrieve shared context |
| `codex-reply` / `claude-reply` / `gemini-reply` | Continue a session | Multi-turn conversations |
| `status` | Check CLI availability | Diagnostics |

Shared memory is cached per project (TTL + git HEAD invalidation) and auto-prepended to MCP calls. Use `memory` or `memory_key`/`memory_mode`.

## Diagnostics

Status tips:
- `conductor status --skip-bridges` skips MCP bridge probes (faster).
- `CONDUCTOR_BRIDGE=codex,claude|all|none` controls which bridges are enabled.
- `CONDUCTOR_BRIDGE_STRICT=1` fails fast when a bridge is unavailable.
- `CONDUCTOR_BRIDGE_CACHE_TTL=30s` controls bridge status cache duration.
- `CONDUCTOR_AUTH_CACHE_TTL=30s` controls CLI auth cache duration.
- `CONDUCTOR_ASYNC_LOG_MAX_BYTES=40000` caps async stdout/stderr log size.
- `CONDUCTOR_RUN_HISTORY_MAX_BYTES=10485760` caps run history size.
- `CONDUCTOR_QUEUE_SNAPSHOT_MAX=200` caps runtime queue snapshot size.

`conductor status --json` includes:
- `ok`: overall health
- `bridge_mode`: enabled MCP bridges (`codex,claude|none`)
- `bridge_targets`: list of bridge targets
- `bridges_ok`: aggregate bridge probe result
- `bridges`: per-bridge status payload

`conductor doctor --json` includes:
- `ok`: overall config + CLI/model checks
- `bridge_mode`: enabled MCP bridges (`codex,claude|none`)
- `bridge_targets`: list of bridge targets
- `errors`: config validation errors
- `roles`: per-role diagnostics

## Configuration

Config path: `~/.conductor-kit/conductor.json` (or nearest `.conductor-kit/conductor.json` in current/parent directories)

Role-based routing example:
```json
{
  "roles": {
    "sage": {
      "cli": "codex",
      "model": "gpt-5.2-codex",
      "reasoning": "medium",
      "description": "Deep reasoning for complex problems"
    },
    "scout": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "Web search and research"
    },
    "pathfinder": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "Codebase exploration and navigation"
    },
    "pixelator": {
      "cli": "gemini",
      "model": "gemini-3-pro",
      "description": "Web UI/UX design and frontend"
    }
  }
}
```

Notes for custom role args:
- Claude: keep `-p {prompt}` (or `--print {prompt}`) adjacent.
- Gemini: if using `-p`, keep `-p {prompt}` adjacent.
- Claude/Gemini: keep `--output-format stream-json` so session IDs can be parsed.
- Codex: `--approval-policy` = `untrusted|on-request|on-failure|never`, `--sandbox` = `read-only|workspace-write|danger-full-access`.

Interactive setup:
```bash
conductor settings
conductor settings --list-models --cli codex
```

## Slash Commands

Claude Code:

| Command | Description |
|---------|-------------|
| `/conductor-plan` | Create an implementation plan |
| `/conductor-search` | Search codebase with delegation |
| `/conductor-implement` | Implement with verification |
| `/conductor-debug` | Debug with multi-CLI analysis |
| `/conductor-review` | Code review workflow |
| `/conductor-release` | Release preparation |
| `/conductor-symphony` | Full orchestration mode |

Codex CLI (prefix with `/prompts:`):
```
/prompts:conductor-plan
/prompts:conductor-symphony
```

## Commands

| Command | Description |
|---------|-------------|
| `conductor install` | Install skills/commands to CLIs |
| `conductor uninstall` | Remove installed files |
| `conductor disable` | Disable conductor (remove skills/commands + MCP) |
| `conductor enable` | Enable conductor (restore skills/commands + MCP) |
| `conductor status` | Check CLI auth and availability |
| `conductor roles` | List role → CLI/model mappings |
| `conductor config-validate` | Validate config JSON |
| `conductor doctor` | Full diagnostics |
| `conductor settings` | Configure roles and models |
| `conductor mcp-bundle` | Render MCP bundle templates |
| `conductor mcp` | Start unified MCP server |
| `conductor help` | Show command help |

## Troubleshooting

"conductor: command not found":
```bash
which conductor
export PATH="$PATH:$(npm config get prefix)/bin"
```

MCP tools not appearing:
```bash
conductor status
```

CLI not detected:
```bash
conductor doctor
```

## Uninstall

```bash
brew uninstall --cask conductor-kit
npm uninstall -g conductor-kit
conductor uninstall
rm -rf ~/.conductor-kit
```

## License

MIT
