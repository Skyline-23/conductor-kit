---
name: conductor
description: |
  Use for ANY code task: explore, analyze, search, implement, review, refactor.
  Delegates to optimal AI (Codex/Gemini/Claude) via MCP. Say "ulw" for full automation.
proactive: true
triggers:
  - 파악
  - 탐색
  - 분석
  - 조사
  - 구조
  - 리뷰
  - 구현
  - 수정
  - explore
  - analyze
  - review
  - search
  - implement
  - refactor
  - investigate
  - structure
  - codebase
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

## Triggers

**This skill activates automatically when these keywords are detected:**

| Keyword | Mode | Action |
|---------|------|--------|
| `ulw`, `ultrawork` | Ultrawork | Full automation loop |
| explore, 탐색, 구조, structure, codebase | Search | MCP `explore` role |
| review, analyze, 분석, 조사, 파악, investigate | Search | MCP `explore` + `oracle` |
| plan, design, 설계, 계획 | Plan | Read-only planning |
| fix, implement, 수정, 구현, refactor | Implement | MCP-assisted implementation |
| release, deploy, 배포 | Release | Release checklist |

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
| `frontend-ui-ux-engineer` | `mcp__gemini-cli__gemini_prompt` | UI/UX review, components |
| `document-writer` | `mcp__gemini-cli__gemini_prompt` | README, docs, changelogs |
| `multimodal-looker` | `mcp__gemini-cli__gemini_prompt` | Screenshot/image analysis |

**Fallback order:** MCP tool → `mcp__claude-cli__claude_prompt` → built-in subagent → disclose

### Delegation Prompt Template
```
Goal: [one-line task]
Constraints: [limits, requirements]
Files: [relevant paths]
Output format: JSON with summary, confidence, findings, actions
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
