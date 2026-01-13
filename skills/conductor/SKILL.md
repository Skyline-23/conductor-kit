---
name: conductor
description: Multi-agent orchestration with MCP delegation. Say "ulw" for full automation.
---

# Conductor

**Delegate first, act later.** Host orchestrates; delegates do the work.

## Triggers

| Keyword | Mode | Action |
|---------|------|--------|
| `ulw` | Ultrawork | Full loop with staged delegation |
| review, analyze, 분석 | Search | Parallel discovery |
| plan, design, 설계 | Plan | Read-only, 3–6 step plan |
| fix, implement, 수정 | Implement | TDD, one change at a time |
| release, deploy, 배포 | Release | Version + changelog checklist |

## Roles

| Role | When to use |
|------|-------------|
| `oracle` | Architecture, security, algorithms **(mandatory for complex)** |
| `explore` | File discovery, codebase navigation |
| `librarian` | Docs, best practices |
| `multimodal-looker` | Screenshot analysis |

See `references/` for details on routing, context budget, and output formats.

## Loop

1. **Search** → 2. **Plan** → 3. **Execute** → 4. **Verify** → 5. **Cleanup**

- Search: multiple angles, collect evidence
- Plan: success criteria per step
- Execute: minimal edits, no type-safety hacks
- Verify: test → typecheck → lint
- Cleanup: summarize, prune context

## Safety

No commit/push, secrets, or destructive commands unless explicitly asked.
