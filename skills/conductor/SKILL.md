---
name: conductor
description: Multi-agent orchestration with MCP delegation. Say "ulw" for full automation.
---

# Conductor

Host orchestrates; delegates do the work.

---

## ⚠️ Core Rules (non-negotiable)

### 1. DELEGATE FIRST
**Do NOT search, plan, or edit until delegation completes.**

- Check MCP tools availability first
- Run all delegate calls before any action
- If MCP unavailable → use subagent fallback → disclose to user

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

| Keyword | Mode |
|---------|------|
| `ulw`, `ultrawork` | Ultrawork |
| review, analyze, 분석, 조사, 파악 | Search |
| plan, design, 설계, 계획 | Plan |
| fix, implement, 수정, 구현 | Implement |
| release, deploy, 배포 | Release |

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

## Roles

| Role | When to use |
|------|-------------|
| `oracle` | Complex reasoning, architecture, security |
| `explore` | File discovery, codebase navigation |
| `librarian` | Doc lookup, best practices |
| `frontend-ui-ux-engineer` | UI/UX review, components |
| `document-writer` | README, docs, changelogs |
| `multimodal-looker` | Screenshot/image analysis |

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
- Parallel codebase + external doc searches
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
