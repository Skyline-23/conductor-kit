# conductor-kit

A global skills pack and lightweight Go helper for **Codex CLI** and **Claude Code**.
It enforces a consistent orchestration loop (search -> plan -> execute -> verify -> cleanup) and supports optional background delegation.

**Language**: English | [한국어](README.ko.md)

## Quick start (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install conductor-kit

# Install skills/commands into Codex + Claude
conductor install --mode link --repo $(brew --prefix)/share/conductor-kit
```

## Manual install
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

## What you get
- **Skill**: `conductor` (`skills/conductor/SKILL.md`)
- **Commands**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go helper**: `conductor` binary for install + background tasks + MCP server
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

### 3) Background delegation
```bash
# auto-detect installed CLIs
conductor background-batch --prompt "<task>"

# role-based (from ~/.conductor-kit/conductor.json)
conductor background-batch --roles auto --prompt "<task>"

# model override (single or comma list)
conductor background-batch --roles oracle \
  --model gpt-5.2-codex,gpt-5.2-codex-mini \
  --reasoning xhigh \
  --prompt "<task>"

# read output
conductor background-output --task-id <id>

# cancel all
conductor background-cancel --all
```

## MCP tools (optional)
```bash
codex mcp add conductor -- conductor mcp
```
Tools:
- `conductor.background_task`
- `conductor.background_batch`
- `conductor.background_output`
- `conductor.background_cancel`

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
