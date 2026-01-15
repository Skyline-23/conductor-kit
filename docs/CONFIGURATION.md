# Configuration Guide

conductor-kit uses a JSON configuration file to control role-based CLI routing, timeouts, and other runtime behavior.

## Config File Location

| Priority | Location | Description |
|----------|----------|-------------|
| 1 | `CONDUCTOR_CONFIG` env var | Custom path override |
| 2 | `./.conductor-kit/conductor.json` | Project-local config |
| 3 | `~/.conductor-kit/conductor.json` | Global user config |

## Full Configuration Schema

```json
{
  "defaults": {
    "timeout_ms": 0,
    "idle_timeout_ms": 120000,
    "summary_only": false,
    "max_parallel": 4,
    "retry": 0,
    "retry_backoff_ms": 500,
    "log_prompt": false
  },
  "roles": {
    "role-name": {
      "cli": "codex|claude|gemini",
      "model": "model-name",
      "model_flag": "--model",
      "args": ["-p", "{prompt}"],
      "reasoning": "low|medium|high",
      "reasoning_flag": "-c model_reasoning_effort",
      "reasoning_key": "model_reasoning_effort",
      "models": ["model1", "model2"],
      "env": { "KEY": "value" },
      "cwd": "/path/to/dir",
      "timeout_ms": 0,
      "idle_timeout_ms": 120000,
      "max_parallel": 4,
      "retry": 0,
      "retry_backoff_ms": 500
    }
  }
}
```

## Defaults Section

Global defaults applied to all roles unless overridden.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `timeout_ms` | int | `0` | Hard timeout in milliseconds. `0` = no limit |
| `idle_timeout_ms` | int | `120000` | Inactivity timeout (2 minutes) |
| `summary_only` | bool | `false` | Return summary instead of full output |
| `max_parallel` | int | `4` | Max concurrent CLI executions |
| `retry` | int | `0` | Number of retries on failure |
| `retry_backoff_ms` | int | `500` | Backoff between retries |
| `log_prompt` | bool | `false` | Store prompt text in run history |

## Roles Section

Each role defines how to route prompts to a specific CLI.

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `cli` | string | CLI executable: `codex`, `claude`, or `gemini` |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `model` | string | Model to use (CLI-native name) |
| `model_flag` | string | Flag to pass model. Default varies by CLI |
| `args` | array | Custom argv template. Use `{prompt}` placeholder |
| `reasoning` | string | Reasoning effort: `low`, `medium`, `high` (Codex only) |
| `reasoning_flag` | string | Flag for reasoning config |
| `reasoning_key` | string | Config key for reasoning |
| `models` | array | List of models for batch fan-out |
| `env` | object | Environment variable overrides |
| `cwd` | string | Working directory override |

### Per-Role Overrides

Each role can override any default:

| Field | Description |
|-------|-------------|
| `timeout_ms` | Role-specific hard timeout |
| `idle_timeout_ms` | Role-specific idle timeout |
| `max_parallel` | Role-specific parallelism |
| `retry` | Role-specific retry count |
| `retry_backoff_ms` | Role-specific backoff |

## CLI Defaults

When `args` and `model_flag` are omitted, these defaults are used:

| CLI | Args | Model Flag | Reasoning Flag |
|-----|------|------------|----------------|
| `codex` | `["exec", "{prompt}"]` | `-m` | `-c model_reasoning_effort` |
| `claude` | `["-p", "{prompt}"]` | `--model` | - |
| `gemini` | `["{prompt}"]` | `--model` | - |

## Examples

### Minimal Config

```json
{
  "roles": {
    "oracle": {
      "cli": "codex"
    }
  }
}
```

### Default Config (installed by conductor)

```json
{
  "defaults": {
    "timeout_ms": 0,
    "idle_timeout_ms": 120000,
    "max_parallel": 4,
    "retry": 0,
    "retry_backoff_ms": 500,
    "log_prompt": false
  },
  "roles": {
    "oracle": {
      "cli": "codex",
      "model": "gpt-5.2-codex",
      "reasoning": "medium"
    },
    "librarian": {
      "cli": "gemini",
      "model": "gemini-3-flash-preview"
    },
    "explore": {
      "cli": "gemini",
      "model": "gemini-3-flash-preview"
    },
    "frontend-ui-ux-engineer": {
      "cli": "gemini",
      "model": "gemini-3-pro-preview"
    },
    "document-writer": {
      "cli": "gemini",
      "model": "gemini-3-flash-preview"
    },
    "multimodal-looker": {
      "cli": "gemini",
      "model": "gemini-3-flash-preview"
    }
  }
}
```

### Multi-Model Batch Config

```json
{
  "roles": {
    "oracle": {
      "cli": "codex",
      "models": [
        { "name": "gpt-5.2-codex", "reasoning_effort": "high" },
        { "name": "o3", "reasoning_effort": "medium" }
      ]
    }
  }
}
```

### Custom CLI Args

```json
{
  "roles": {
    "custom-claude": {
      "cli": "claude",
      "args": ["-p", "{prompt}", "--max-turns", "5", "--permission-mode", "bypassPermissions"],
      "model": "sonnet"
    }
  }
}
```

### Environment and Working Directory

```json
{
  "roles": {
    "secure-runner": {
      "cli": "codex",
      "env": {
        "CODEX_SANDBOX": "docker"
      },
      "cwd": "/secure/workspace"
    }
  }
}
```

## Validating Config

```bash
# Validate config syntax
conductor config-validate

# Full diagnostics (config + CLI availability + model names)
conductor doctor
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `CONDUCTOR_CONFIG` | Override config file path |

## Schema

For IDE autocompletion and validation, use the JSON schema:
`config/conductor.schema.json`

## Tips

1. **Start minimal**: Only specify what you need. Defaults work well.
2. **Use `conductor settings`**: Interactive TUI for easy editing.
3. **Check with `conductor doctor`**: Validates config and CLI availability.
4. **Project-local overrides**: Put `.conductor-kit/conductor.json` in your repo root.
