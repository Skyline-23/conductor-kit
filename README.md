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

For project-local installs, use:
```bash
conductor install --project
```

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

**For OpenCode**:
```bash
opencode mcp add conductor -- conductor mcp
```

Notes:
- Codex config lives in `~/.codex/config.toml` (or project `.codex/config.toml`).
- OpenCode config lives in `~/.config/opencode/opencode.json` (or project `opencode.json`).

### Bridge mode

- `conductor mcp` runs the unified MCP server in stdio mode for any MCP client.
- `conductor mcp` bridges Codex (`codex mcp-server`) and Claude tools (`claude mcp serve`), while Claude prompts run via native CLI.
- Claude Code MCP server exposes tools like View/Edit/LS; the MCP client is responsible for any tool approval flow.
- Codex `mcp-server` inherits global config overrides, so approvals/sandboxing should be set in Codex config when needed.
- Codex `app-server` is a separate JSON-RPC protocol (not MCP).
- `conductor mcp` warns and continues if an upstream MCP server is unavailable (set `CONDUCTOR_BRIDGE_STRICT=1` to fail fast).
- OpenCode is an MCP client; connect it to local or remote servers via `opencode mcp add`.

Status tips:
- `conductor status --skip-bridges` skips MCP bridge probes (faster).
- `CONDUCTOR_BRIDGE=codex,claude|all|none` controls which bridges are enabled.
- `CONDUCTOR_BRIDGE_STRICT=1` fails fast when a bridge is unavailable.
- `CONDUCTOR_BRIDGE_CACHE_TTL=30s` controls bridge status cache duration.
- `CONDUCTOR_AUTH_CACHE_TTL=30s` controls CLI auth cache duration.
- `CONDUCTOR_ASYNC_LOG_MAX_BYTES=40000` caps async stdout/stderr log size.
- `CONDUCTOR_RUN_HISTORY_MAX_BYTES=10485760` caps run history size.
- `CONDUCTOR_QUEUE_SNAPSHOT_MAX=200` caps runtime queue snapshot size.

### MCP bundle templates (optional)

`config/mcp-bundles.json` includes optional templates. `conductor` renders a ready-to-register Conductor server, and `extended` is a scaffold for extra MCP servers.

Enable the servers you want in `~/.conductor-kit/mcp-bundles.json`, then render per host:
```bash
conductor mcp-bundle --host claude --bundle conductor
conductor mcp-bundle --host codex --bundle conductor
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
| `codex` | Run Codex MCP session (bridged) | Deep reasoning, complex analysis |
| `claude` | Run Claude Code session (native CLI) | Code generation, refactoring |
| `claude__*` | Claude Code tools (bridged) | View/Edit/LS, etc. |
| `gemini` | Run Gemini CLI session | Web search, research |
| `conductor` | Role-based routing | Auto-select best CLI for task |
| `memory` | Shared memory cache | Store/retrieve shared context |
| `codex-reply` / `claude-reply` / `gemini-reply` | Continue a session | Multi-turn conversations |
| `status` | Check CLI availability | Diagnostics |

Shared memory is cached per project (TTL + git HEAD invalidation) and auto-prepended to MCP calls. Use `memory` to update it, or `memory_key`/`memory_mode` to inject additional keys on `codex`, `claude`, `gemini`, or `conductor`.

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

Config file: `~/.conductor-kit/conductor.json` (or nearest `.conductor-kit/conductor.json` in the current/parent directories)

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

Notes for custom role args:
- Claude: keep `-p {prompt}` (or `--print {prompt}`) adjacent.
- Gemini: if using `-p`, keep `-p {prompt}` adjacent.
- Claude/Gemini: keep `--output-format stream-json` so session IDs can be parsed.
- Codex: `--approval-policy` = `untrusted|on-request|on-failure|never`, `--sandbox` = `read-only|workspace-write|danger-full-access`.

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
   conductor status
   ```

Status JSON includes:
- `ok`: overall health
- `bridge_mode`: enabled MCP bridges (`codex,claude|none`)
- `bridge_targets`: list of bridge targets
- `bridges`: per-bridge status payload

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
