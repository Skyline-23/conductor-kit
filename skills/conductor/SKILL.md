---
name: conductor
description: Orchestrate multi-agent workflows with MCP delegation. Say "ulw" for full automation.
---

# Conductor

Enforce orchestration over one-shot summarization. Host stays in control; delegates do the work.

## Trigger Rules

| Trigger | Mode | Behavior |
|---------|------|----------|
| `ulw`, `ultrawork` | Ultrawork | Full loop with staged delegation |
| review, analyze, 분석, 조사 | Search | Parallel discovery, collect evidence |
| plan, design, 설계, 계획 | Plan | Read-only, output 3–6 step plan |
| fix, implement, 수정, 구현 | Implement | TDD, one change at a time |
| release, deploy, 배포 | Release | Versioning checklist |

## Delegation

**Delegate first, act later.**

| Role | Use For |
|------|---------|
| `oracle` | Deep reasoning, architecture, security **(mandatory for complex tasks)** |
| `explore` | Codebase navigation, file discovery |
| `librarian` | Doc lookup, best practices |
| `frontend-ui-ux-engineer` | UI/UX review, component design |
| `document-writer` | README, docs, changelogs |
| `multimodal-looker` | Screenshot/image analysis |

Details: `references/roles.md`, `references/delegation.md`, `references/formats.md`

## Operating Loop

```
Search → Plan → Execute → Verify → Cleanup
```

1. **Search** — Parallel discovery. Multiple angles.
2. **Plan** — 3–6 steps with success criteria.
3. **Execute** — Minimal edits. No type-safety hacks.
4. **Verify** — Test → typecheck → lint.
5. **Cleanup** — Summarize. Prune context.

## Mode Shortcuts

- **Search**: Evidence over opinions. Parallel searches.
- **Plan**: Read-only. No edits.
- **Implement**: TDD. Rollback when stuck.
- **Release**: Version, changelog, secret scan.
- **Ultrawork**: `"ULTRAWORK MODE ENABLED!"` → staged delegation → full loop.

## Safety (non-negotiable)

- No commit/push unless asked.
- No secrets in commits.
- No destructive commands unless explicit.
