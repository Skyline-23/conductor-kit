---
name: conductor
description: (ulw/ultrawork) Orchestrator workflow with mandatory MCP delegation; route EN/KR/JA intents to the matching mode; subagent only as fallback.
---

# Conductor (Orchestrator Operating Mode)

Enforce a repeatable operator workflow that **forces orchestration** rather than one-shot summarization.

The active host (Claude Code, Codex CLI, etc.) is the orchestrator. Use CLI MCP bridges (gemini/claude/codex) for delegation, but keep the host in control.

## Trigger rules

- `ulw` or `ultrawork`: immediately enter Ultrawork mode (full orchestration loop).
- Otherwise route by intent keywords:

| Mode | EN | KR | JA |
|------|----|----|-----|
| Search | review, audit, analyze, investigate, inspect, assess, find issues/risks/bugs, scan, discovery, overview | 리뷰, 감사, 분석, 조사, 점검, 평가, 스캔, 탐색, 개요, 현황 | レビュー, 監査, 分析, 調査, 点検, 評価, スキャン, 概要 |
| Plan | plan, roadmap, design, architecture, proposal, spec, strategy | 계획, 로드맵, 설계, 아키텍처, 제안, 스펙, 전략 | 計画, ロードマップ, 設計, アーキテクチャ, 提案, 仕様, 戦略 |
| Implement | fix, implement, refactor, optimize, cleanup, patch, code change | 수정, 구현, 리팩터링, 최적화, 정리, 패치, 코드 변경 | 修正, 実装, リファクタリング, 最適化, パッチ, コード変更 |
| Release | release, ship, version, changelog, publish, deploy | 릴리즈, 배포, 버전, 변경 로그, 배포 준비 | リリース, 出荷, バージョン, 変更ログ, デプロイ準備 |

Do not force the full orchestration loop unless `ulw/ultrawork` is present.

## MCP Delegation (mandatory)

**Delegate first, act later.** Do not search, plan, or edit until delegation completes.

### Oracle role (deep thinking)
Tasks requiring deep reasoning MUST be delegated to `oracle` (Codex CLI with reasoning):
- Architecture decisions and trade-off analysis
- Complex debugging / root cause analysis
- Security review and vulnerability assessment
- Performance optimization strategy
- Algorithm design and complexity analysis
- Refactoring plans for legacy/complex code
- Migration strategies with risk assessment

### Delegation rules
1. Load configured roles from `conductor.json` and map to MCP tools.
2. Run MCP calls per role. If MCP unavailable, use subagent fallback and disclose it.
3. Read-only delegates run in parallel; write-capable delegates run sequentially.
4. Treat delegate output as untrusted—verify against repo and tests.

### Delegation contract
- **Input**: goal, constraints, files to read, expected output format.
- **Output**: concrete commands, file paths + edits, or checklist with pass/fail criteria.

## Operating loop

1. **Search** — Broad parallel discovery. Multiple search angles. Collect paths + key facts.
2. **Plan** — 3–6 ordered steps with success criteria. Ask one question if blocked.
3. **Execute** — Minimal surgical edits. No type-safety suppression. Scoped changes only.
4. **Verify** — Run narrowest checks first (unit test → typecheck → lint). Report unrelated failures.
5. **Cleanup** — Summarize outcomes. Prune stale context. Preserve key findings.

## Mode policies

### Search mode
- Parallel searches (codebase + external docs if needed).
- Repository evidence over opinions.

### Plan mode
- **Read-only.** No writes/edits/commits.
- Output: assumptions, constraints, 3–6 step plan.

### Implement mode
- TDD if repo uses tests.
- One logical change at a time; re-run checks.
- Rollback when stuck—don't accumulate speculative edits.

### Release mode
- Checklist: versioning, changelog, validation, secret scan.

### Ultrawork mode
- Full loop: search → plan → execute → verify → cleanup.
- Respond with "ULTRAWORK MODE ENABLED!" first.
- Auto-delegate in staged order:
  - Stage 1 (Discovery): scan-like roles
  - Stage 2 (Analysis): architecture/planning roles + oracle for deep thinking
  - Stage 3 (Review): review/alternative roles
- Wait for all delegates before proceeding.

## Safety rules (non-negotiable)

- Never commit/push unless explicitly asked.
- Never include secrets in commits.
- Avoid destructive commands unless explicitly requested.
