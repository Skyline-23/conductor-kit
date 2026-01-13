---
name: conductor
description: (ulw/ultrawork) Orchestrator workflow with mandatory MCP delegation; route EN/KR/JA intents to the matching mode; subagent only as fallback.
---

# Conductor (Orchestrator Operating Mode)

Enforce a repeatable operator workflow that **forces orchestration** rather than one-shot summarization.

The active host (Claude Code, Codex CLI, etc.) is the orchestrator. Use CLI MCP bridges for delegation, keeping host in control.

## Trigger Rules

- `ulw` or `ultrawork`: immediately enter Ultrawork mode (full loop).
- Otherwise route by intent keywords:

| Mode | EN | KR | JA |
|------|----|----|-----|
| Search | review, audit, analyze, investigate, scan, overview | 리뷰, 분석, 조사, 점검, 탐색, 현황 | レビュー, 分析, 調査, 点検, 概要 |
| Plan | plan, roadmap, design, architecture, strategy | 계획, 로드맵, 설계, 아키텍처, 전략 | 計画, 設計, アーキテクチャ, 戦略 |
| Implement | fix, implement, refactor, optimize, patch | 수정, 구현, 리팩터링, 최적화, 패치 | 修正, 実装, リファクタリング, 最適化 |
| Release | release, ship, version, changelog, deploy | 릴리즈, 배포, 버전, 변경 로그 | リリース, バージョン, デプロイ |

## MCP Delegation (mandatory)

**Delegate first, act later.** Do not search, plan, or edit until delegation completes.

See: `references/delegation.md` for full rules, context budget, and failure handling.

### Role Routing
Route to appropriate role based on task type. See: `references/roles.md`

| Role | Use For |
|------|---------|
| `oracle` | Deep reasoning, architecture, security, algorithms (MANDATORY for complex tasks) |
| `librarian` | Doc lookup, API reference, best practices |
| `explore` | Codebase navigation, file discovery |
| `multimodal-looker` | Screenshot/image analysis |

### Output Format
All delegates must return structured JSON. See: `references/formats.md`

```json
{
  "summary": "one-line conclusion",
  "confidence": "high|medium|low",
  "findings": [...],
  "suggested_actions": [...]
}
```

## Operating Loop

1. **Search** — Broad parallel discovery. Multiple angles. Collect paths + facts.
2. **Plan** — 3–6 steps with success criteria. One question if blocked.
3. **Execute** — Minimal surgical edits. No type-safety suppression.
4. **Verify** — Narrowest checks first (test → typecheck → lint).
5. **Cleanup** — Summarize outcomes. Prune stale context.

## Mode Policies

### Search mode
- Parallel searches. Repository evidence over opinions.

### Plan mode
- **Read-only.** No writes/edits/commits.
- Output: assumptions, constraints, 3–6 step plan.

### Implement mode
- TDD if repo uses tests. One change at a time.
- Rollback when stuck.

### Release mode
- Checklist: versioning, changelog, validation, secret scan.

### Ultrawork mode
- Full loop: search → plan → execute → verify → cleanup.
- Respond with "ULTRAWORK MODE ENABLED!" first.
- Staged delegation:
  - Stage 1: `explore` (discovery)
  - Stage 2: `oracle` (analysis) + `librarian` (verification)
  - Stage 3: additional review roles
- Wait for all delegates before proceeding.

## Safety Rules (non-negotiable)

- Never commit/push unless explicitly asked.
- Never include secrets in commits.
- Avoid destructive commands unless explicitly requested.

## Reference Files

- `references/roles.md` — Role routing guide and task mapping
- `references/delegation.md` — Context budget, failure handling, delegation sequence
- `references/formats.md` — Output format standards for all delegate types
