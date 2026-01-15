# conductor-kit

A global skills pack for **Codex CLI**, **Claude Code**, and **Gemini CLI** with a unified MCP server for cross-CLI orchestration.

**Language**: English | [한국어](README.ko.md)

## What is this?

conductor-kit helps you:
- Use a consistent orchestration workflow (search → plan → execute → verify)
- Delegate tasks between different AI CLIs (Codex, Claude, Gemini)
- Load specialized skills and commands into your preferred CLI

## Quick Start

### Install (macOS)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit
conductor install
```

### Install (Manual)
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install
```

### Verify Installation
```bash
conductor status   # Check CLI availability and auth status
conductor doctor   # Full diagnostics
```

## Usage

### 1. Use the Skill
In Claude Code or Codex CLI, trigger by saying:
- `conductor` or `ultrawork` or `ulw`

### 2. Use Slash Commands
| Claude Code | Codex CLI |
|-------------|-----------|
| `/conductor-plan` | `/prompts:conductor-plan` |
| `/conductor-search` | `/prompts:conductor-search` |
| `/conductor-implement` | `/prompts:conductor-implement` |
| `/conductor-release` | `/prompts:conductor-release` |
| `/conductor-ultrawork` | `/prompts:conductor-ultrawork` |

### 3. Use MCP Tools (Cross-CLI Delegation)

Register the unified MCP server:

**Claude Code** (`~/.claude/mcp.json`):
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

**Codex CLI**:
```bash
codex mcp add conductor -- conductor mcp
```

Available MCP tools:
| Tool | Description |
|------|-------------|
| `codex` | Run Codex CLI session |
| `codex-reply` | Continue Codex session |
| `claude` | Run Claude Code session |
| `claude-reply` | Continue Claude session |
| `gemini` | Run Gemini CLI session |
| `gemini-reply` | Continue Gemini session |
| `conductor` | Role-based routing (uses config) |
| `conductor-reply` | Continue conductor session |
| `status` | Check CLI availability and auth |

Example usage in your prompt:
```
Use the gemini tool to search the codebase for authentication logic
```

## Configuration

Config file: `~/.conductor-kit/conductor.json`

### Role-based Routing (Default)
```json
{
  "roles": {
    "sage": { "cli": "codex", "model": "gpt-5.2-codex", "reasoning": "medium", "description": "Deep reasoning for complex problems" },
    "scout": { "cli": "gemini", "model": "gemini-3-flash", "description": "Web search and research" },
    "pathfinder": { "cli": "gemini", "model": "gemini-3-flash", "description": "Codebase exploration and navigation" },
    "pixel": { "cli": "gemini", "model": "gemini-3-pro", "description": "Web UI/UX design and frontend" }
  }
}
```

### Setup Wizard
```bash
conductor settings              # Interactive TUI wizard
conductor settings --list-models --cli codex  # List available models
```

For detailed configuration options, see [CONFIGURATION.md](docs/CONFIGURATION.md).

## Requirements

- **Host CLI**: At least one of Codex CLI, Claude Code, or Gemini CLI
- **Go 1.24+**: Only if building from source
- **macOS**: For Homebrew cask install (Linux: use manual install)

## Commands Reference

| Command | Description |
|---------|-------------|
| `conductor install` | Install skills/commands to CLIs |
| `conductor uninstall` | Remove installed files |
| `conductor status` | Check CLI auth and availability |
| `conductor doctor` | Full diagnostics |
| `conductor settings` | Configure roles and models |
| `conductor mcp` | Start unified MCP server |

## Uninstall

```bash
# Homebrew
brew uninstall --cask conductor-kit

# Manual
conductor uninstall
```

## License

MIT
