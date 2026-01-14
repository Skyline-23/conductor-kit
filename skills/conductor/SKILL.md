---
name: conductor
description: |
  STOP. Load this skill FIRST before doing ANYTHING with code.

  This skill MUST be loaded for ANY code-related task. No exceptions.
  Using built-in tools (Task, Explore, grep agents) without loading this skill is a violation.

  TRIGGERS: ulw, ultrawork, code, debug, fix, refactor, implement, search, explore, plan, review, release, build, test, deploy

  REPLACES: Task(Explore), Task(Plan), built-in search/grep agents.

  Handles: reading code, project structure, debugging, planning, implementing, refactoring, reviewing, testing, deploying, documentation - ALL code tasks.
---

# Conductor

Host orchestrates; delegates do the work.

---

## ⚠️ Core Rules (non-negotiable)

### 1. DELEGATE FIRST — NO EXCEPTIONS
**Do NOT use built-in tools (Explore, Grep, Search) when MCP is available.**

- Check MCP tools availability FIRST (`mcp__*` tools)
- ALWAYS prefer MCP delegation over built-in/native tools:
  - ❌ `Task(subagent_type=Explore)` → ✅ MCP `explore` role
  - ❌ Built-in search/grep → ✅ MCP `librarian` or `explore` role
  - ❌ Direct analysis → ✅ MCP `oracle` role for complex reasoning
- Run all delegate calls before any action
- If MCP unavailable → use subagent fallback → **disclose to user**

### 2. ORACLE FOR COMPLEX TASKS
The following MUST be delegated to `oracle` (Codex + reasoning):

- Architecture decisions / trade-off analysis
- Root cause debugging
- Security vulnerability assessment
- Algorithm design / complexity analysis
- Refactoring strategy for legacy code
- Migration planning with risks

**Do not attempt deep analysis yourself. Oracle first.**

### 3. VERIFY BEFORE TRUST
Treat all delegate output as untrusted. Verify against:
- Actual repo code
- Test results
- Type checker output

---

## Activation

**This skill activates automatically for all code-related tasks.**

Conductor assesses the task and chooses the appropriate mode:

| Mode | When | Action |
|------|------|--------|
| **Ultrawork** | `ulw` or `ultrawork` command | Full automation: Search → Plan → Execute → Verify → Cleanup |
| **Search** | Explore, analyze, investigate, understand | Delegate to `explore` + `oracle` via MCP |
| **Plan** | Design, architect, plan | Read-only planning, no edits |
| **Implement** | Fix, build, refactor, migrate | MCP-assisted implementation |
| **Release** | Deploy, publish, release | Release checklist + validation |

**Decision flow:**
1. Skill loads → Conductor activates
2. Assess task complexity
3. Simple task → execute directly
4. Complex/specialized → delegate via MCP

---

## Ultrawork Mode

When triggered, respond **immediately** with:

```
ULTRAWORK MODE ENABLED!
```

Then execute staged delegation:

**Stage 1 — Discovery**
- `explore`: file structure, entrypoints, patterns

**Stage 2 — Analysis**
- `oracle`: deep reasoning on findings (MANDATORY)
- `librarian`: verify against docs/best practices

**Stage 3 — Review**
- Additional roles as needed

**Then:** Search → Plan → Execute → Verify → Cleanup

Do NOT proceed until all delegates complete.

---

## Roles → MCP Tools

**Always use MCP tools for delegation. Map roles to tools:**

| Role | MCP Tool | When to use |
|------|----------|-------------|
| `oracle` | `mcp__codex-cli__codex_prompt` | Complex reasoning, architecture, security |
| `explore` | `mcp__gemini-cli__gemini_prompt` | File discovery, codebase navigation, **project structure** |
| `librarian` | `mcp__gemini-cli__gemini_prompt` | Doc lookup, best practices |
| `frontend-ui-ux-engineer` | `mcp__gemini-cli__gemini_prompt` | **Web** UI/UX, React/Vue/CSS, responsive design |
| `document-writer` | `mcp__gemini-cli__gemini_prompt` | README, docs, changelogs |
| `multimodal-looker` | `mcp__gemini-cli__gemini_prompt` | Screenshot/image analysis |

**Fallback order:** MCP tool → `mcp__claude-cli__claude_prompt` → built-in subagent → disclose

### Model Configuration

**CRITICAL: Read `~/.conductor-kit/conductor.json` BEFORE calling MCP tools.**

Each role has a configured `model`. You MUST pass this model to the MCP tool:

```json
// Example: conductor.json
{
  "roles": {
    "oracle": { "cli": "codex", "model": "gpt-4.1" },
    "explore": { "cli": "gemini", "model": "gemini-2.5-flash" }
  }
}
```

When delegating to `oracle`, call:
```json
mcp__codex-cli__codex_prompt({ "prompt": "...", "model": "gpt-4.1" })
```

When delegating to `explore`, call:
```json
mcp__gemini-cli__gemini_prompt({ "prompt": "...", "model": "gemini-2.5-flash" })
```

**Do NOT omit the model parameter. Do NOT invent model names.**

### Delegation Prompt Template
```
Goal: [one-line task]
Role: [role name]
Constraints: [limits, requirements]
Files: [relevant paths]
Output format: markdown with ## Summary, ## Confidence, ## Findings, ## Suggested Actions
```

---

## Operating Loop

```
Search → Plan → Execute → Verify → Cleanup
```

### Search
- Run parallel searches (multiple angles)
- Collect file paths + key facts
- Evidence over opinions

### Plan
- **READ-ONLY** — no edits allowed
- Output 3–6 steps with success criteria
- Ask ONE question if blocked, otherwise proceed

### Execute
- Minimal surgical edits
- No type-safety hacks (`as any`, `@ts-ignore`)
- One logical change at a time

### Verify
- Run checks: test → typecheck → lint
- If unrelated failure, report but don't fix

### Cleanup
- Summarize outcomes
- Prune stale context
- List next actions if any

---

## Mode-Specific Behavior

### Search Mode
- **Use MCP `explore` role** for codebase discovery (NOT built-in Explore agent)
- Parallel codebase + external doc searches via MCP delegation
- Output: findings with file references

### Plan Mode
- **No writes/edits/commits**
- Output: assumptions, constraints, ordered steps

### Implement Mode
- TDD if repo has tests
- Rollback when stuck (don't accumulate bad edits)

### Release Mode
- Checklist: version bump, changelog, validation, secret scan

---

## Safety (non-negotiable)

- **No commit/push** unless explicitly asked
- **No secrets** in commits (check for .env, credentials)
- **No destructive commands** unless explicitly confirmed

---

## References

For detailed specifications:
- `references/roles.md` — Role routing and combinations
- `references/delegation.md` — Context budget, failure handling
- `references/formats.md` — JSON output schemas
