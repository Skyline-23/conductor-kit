# conductor-kit

A global skills pack for **Codex CLI**, **Claude Code**, and **Gemini CLI** with a unified MCP server for cross-CLI orchestration.

**Language**: English | [한국어](README.ko.md)

## What is this?

conductor-kit enables AI CLI tools to work together seamlessly:

- **Cross-CLI Delegation**: Let Claude delegate to Codex for reasoning, or Gemini for web search
- **Unified Skill System**: One skill works across all supported CLIs
- **Role-based Routing**: Automatically route tasks to the best CLI/model combination
- **MCP Integration**: Full Model Context Protocol support for tool interoperability

## Installation

### Option 1: npx (Easiest)
```bash
npx conductor-kit install
```

### Option 2: Homebrew (macOS)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit
conductor install
```

### Option 3: npm global
```bash
npm install -g conductor-kit
conductor install
```

### Option 4: Build from source
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install
```

### Verify Installation
```bash
conductor doctor   # Full diagnostics
conductor status   # Check CLI availability
```

---

## Tutorial: Getting Started

### Step 1: Install at least one AI CLI

conductor-kit works with these CLIs:

| CLI | Install | Auth |
|-----|---------|------|
| **Claude Code** | `npm install -g @anthropic-ai/claude-code` | `claude` (follow prompts) |
| **Codex CLI** | `npm install -g @openai/codex` | `codex --login` |
| **Gemini CLI** | `npm install -g @anthropic-ai/gemini-cli` | `gemini auth` |

### Step 2: Run the installer

```bash
conductor install
```

This will:
- Detect which CLIs are installed
- Copy skills to `~/.claude/skills/` and/or `~/.codex/skills/`
- Copy slash commands to `~/.claude/commands/` and/or `~/.codex/prompts/`
- Create config at `~/.conductor-kit/conductor.json`

### Step 3: Load the skill

Start your preferred CLI and trigger the conductor skill:

```bash
# In Claude Code
claude
> Load the conductor skill
> sym  # shorthand trigger
```

```bash
# In Codex CLI
codex
> Load conductor
```

The skill provides orchestration guidance and role-based delegation patterns.

---

## Tutorial: Cross-CLI Delegation with MCP

The real power of conductor-kit is letting one CLI call another via MCP tools.

### Step 1: Register the MCP server

**For Claude Code** - Add to `~/.claude/mcp.json`:
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

**For Codex CLI**:
```bash
codex mcp add conductor -- conductor mcp
```

### Step 2: Use cross-CLI tools in your prompts

Now you can ask Claude to delegate to other CLIs:

```
Use the codex tool to analyze this algorithm with deep reasoning
```

```
Use the gemini tool to search the web for React 19 best practices
```

```
Use the conductor tool with role "sage" to solve this complex problem
```

### Available MCP Tools

| Tool | Description | Example |
|------|-------------|---------|
| `codex` | Run Codex CLI session | Deep reasoning, complex analysis |
| `claude` | Run Claude Code session | Code generation, refactoring |
| `gemini` | Run Gemini CLI session | Web search, research |
| `conductor` | Role-based routing | Auto-select best CLI for task |
| `*-reply` | Continue a session | Multi-turn conversations |
| `status` | Check CLI availability | Diagnostics |

### Example: Multi-CLI Workflow

```
I need to implement a new authentication system.

1. Use the gemini tool to research OAuth 2.0 best practices for 2025
2. Use the codex tool to design the architecture with reasoning
3. Then implement it here in Claude
```

---

## Tutorial: Using Slash Commands

Slash commands provide quick access to common workflows.

### In Claude Code

| Command | Description |
|---------|-------------|
| `/conductor-plan` | Create an implementation plan |
| `/conductor-search` | Search codebase with delegation |
| `/conductor-implement` | Implement with verification |
| `/conductor-debug` | Debug with multi-CLI analysis |
| `/conductor-review` | Code review workflow |
| `/conductor-release` | Release preparation |
| `/conductor-symphony` | Full orchestration mode |

### In Codex CLI

Prefix commands with `/prompts:`:
```
/prompts:conductor-plan
/prompts:conductor-symphony
```

---

## Configuration

Config file: `~/.conductor-kit/conductor.json`

### Role-based Routing

Roles map task types to CLI/model combinations:

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

### Interactive Setup

```bash
conductor settings              # TUI wizard
conductor settings --list-models --cli codex  # List models
```

---

## Commands Reference

| Command | Description |
|---------|-------------|
| `conductor install` | Install skills/commands to CLIs |
| `conductor uninstall` | Remove installed files |
| `conductor status` | Check CLI auth and availability |
| `conductor doctor` | Full diagnostics |
| `conductor settings` | Configure roles and models |
| `conductor mcp` | Start unified MCP server |

---

## Troubleshooting

### "conductor: command not found"

Ensure the binary is in your PATH:
```bash
# Check install location
which conductor

# Add to PATH if needed (for npm global)
export PATH="$PATH:$(npm config get prefix)/bin"
```

### MCP tools not appearing

1. Restart your CLI after adding MCP config
2. Check MCP server is working:
   ```bash
   conductor mcp --help
   conductor status
   ```

### CLI not detected

Run diagnostics:
```bash
conductor doctor
```

This shows which CLIs are installed and authenticated.

---

## Uninstall

```bash
# Homebrew
brew uninstall --cask conductor-kit

# npm
npm uninstall -g conductor-kit

# Manual cleanup
conductor uninstall
rm -rf ~/.conductor-kit
```

---

## License

MIT
