# conductor-kit (Global Skills Pack) — Getting Started

## 목표
conductor-kit은 Codex CLI와 Claude Code에서 공통으로 쓸 수 있는 "오케스트레이터 운영 방식"을 스킬로 제공한다.
- 멀티모델 "집계(aggregator)"가 아니라 오케스트레이션(검색 -> 계획 -> 실행 -> 검증 -> 정리)을 강제/유도한다.
- 모델 선택/라우팅은 호스트(Codex/Claude)가 제공하는 기능을 신뢰한다. 스킬은 "언제/왜 어떤 모델" 정도의 정책만 제안한다.
- 설치는 기본적으로 전역(global) 설치이며, 프로젝트별로도 선택적으로 링크 가능하다.
- Go helper CLI는 설치/백그라운드 실행/MCP 등록을 돕는 보조 수단이다. (헤드리스 런타임/데몬은 아님)

## 리포지토리 레이아웃(현재)
하나의 레포에서 `skills/`를 단일 소스 오브 트루스로 유지한다. 현재는 `conductor` 단일 스킬만 둔다.

```
conductor-kit/
  cmd/
    conductor/
      main.go
  commands/
  config/
  skills/
    conductor/
      SKILL.md
```

## 전역 설치(심링크/복사 방식)
아래는 사용자가 로컬에서 실행하는 설치 방식이다(도구 없이).

### 옵션 A) 심링크(추천: 업데이트 편함)
```
git clone https://github.com/<you>/conductor-kit ~/.conductor-kit
mkdir -p ~/.claude/skills ~/.codex/skills
ln -s ~/.conductor-kit/skills/conductor ~/.claude/skills/conductor
ln -s ~/.conductor-kit/skills/conductor ~/.codex/skills/conductor
```

### 옵션 B) 복사(보수적: 링크 싫으면)
```
git clone https://github.com/<you>/conductor-kit ~/.conductor-kit
mkdir -p ~/.claude/skills ~/.codex/skills
cp -R ~/.conductor-kit/skills/conductor ~/.claude/skills/conductor
cp -R ~/.conductor-kit/skills/conductor ~/.codex/skills/conductor
```

## 전역 설치 (Go)
Go 바이너리를 빌드한 뒤 `conductor install`로 설치한다.

예시:
```
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo /path/to/conductor-kit
```

옵션:
- `--mode link|copy` (기본: link)
- `--skills-only` / `--commands-only` (commands 제외하거나 commands만 설치)
- `--codex-home PATH` / `--claude-home PATH`
- `--no-bins` (helper 바이너리 설치 생략)
- `--bin-dir PATH` (기본: `~/.local/bin` 또는 `$CONDUCTOR_BIN`)
- `--no-config` (기본 config 설치 생략)
- `--force` (기존 대상 덮어쓰기)
- `--dry-run`
- `--repo PATH` (기본: 현재 디렉토리)

## Background Tasks (ultrawork 기본)
OpenCode의 `background_task` 스타일을 모방해 로컬 CLI를 비동기로 실행한다.
`conductor background-task` → `conductor background-output` → `conductor background-cancel`.
installer가 alias를 만들어 `conductor-background-*` 형태도 동작한다.

## MCP Tools 등록(추천)
Codex에 MCP 서버를 등록하면 `conductor.background_*` 도구로 추적/호출할 수 있다.

등록 예시:
```
codex mcp add conductor -- conductor mcp
```

제공 도구:
- `conductor.background_task`
- `conductor.background_batch`
- `conductor.background_output`
- `conductor.background_cancel`

## Roles & Models (JSON 설정)
역할별 CLI/모델 매핑은 `~/.conductor-kit/conductor.json`에서 설정한다.
`model`과 `model_flag`를 채우면 각 역할에 모델을 적용할 수 있다.
Codex 역할에는 `reasoning` 값을 넣어 추론 정도를 지정할 수 있다.
여러 모델을 병렬로 돌리고 싶다면 `models` 배열에 나열하거나 `--model`에 콤마 리스트를 넘긴다.

예시(일부):
```
{
  "roles": {
    "oracle": {
      "cli": "codex",
      "args": ["exec", "{prompt}"],
      "model_flag": "-m",
      "reasoning_flag": "-c",
      "reasoning_key": "model_reasoning_effort",
      "model": "gpt-5.2-codex",
      "reasoning": "xhigh",
      "models": [
        { "name": "gpt-5.2-codex", "reasoning_effort": "xhigh" },
        { "name": "gpt-5.2-codex-mini", "reasoning_effort": "medium" }
      ]
    },
    "librarian": { "cli": "claude", "args": ["-p", "{prompt}"], "model_flag": "--model", "model": "claude-3-5-sonnet" }
  }
}
```

## 사용법(요약)
- 스킬 트리거: "conductor", "ultrawork/ulw", "오케스트레이션" 같은 요청으로 호출
- 커맨드: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- 멀티 CLI 자동 호출(기본): background task 방식
  - `conductor background-batch --prompt "<요청>"` (설치된 CLI 자동 감지)
  - 역할 기반: `conductor background-batch --roles auto --prompt "<요청>"`
  - 모델 오버라이드(간편): `conductor background-batch --roles oracle --model gpt-5.2-codex --reasoning xhigh --prompt "<요청>"`
  - 모델 병렬(간편): `conductor background-batch --roles oracle --model gpt-5.2-codex,gpt-5.2-codex-mini --prompt "<요청>"`
  - 필요 시: `conductor background-batch --agents codex,claude,gemini --prompt "<요청>"`
  - 결과 확인: `conductor background-output --task-id <id>`
  - 취소: `conductor background-cancel --all`


## 스킬 카탈로그(확장 제안)
처음엔 `conductor/SKILL.md` 안에 섹션으로 넣고, 커지면 여러 스킬로 분리한다.
1) conductor-core
- 오케스트레이터의 기본 원칙(단계, 산출물, 실패 시 루프)
2) conductor-search-mode
- "먼저 최대 탐색": 병렬 에이전트/검색 우선, 첫 결과에서 멈추지 않기, 근거 기반 요약
3) conductor-plan-mode
- "절대 구현 금지/읽기 전용" 모드 규칙
4) conductor-implement-mode
- TDD/작은 변경/테스트 우선/롤백 전략
5) conductor-tool-policy
- 툴 콜 욕심을 담되, 안전장치(비파괴 원칙, 금지 패턴, 승인 요구 조건)
6) conductor-multi-model-routing
- "호스트 라우팅을 우선" + 선택 기준(탐색=빠른 모델, 설계=정확 모델, 검증=보수 모델)
7) conductor-output-style
- 짧고 스캔 가능한 출력 규칙(섹션/불릿/명령 표기)
8) conductor-release-mode
- 버전/체인지로그/릴리스 노트/배포 루틴(스킬팩 배포 기준 포함)
9) conductor-security
- 시크릿, 토큰, 의심 파일 커밋 금지, 파괴적 git 명령 금지
10) conductor-mcp-optional (2차)
- MCP를 붙이는 기준과 안전한 권장 구성(단, 1차 범위에서는 실행물 없음)

## Commands(기본 설치, Codex/Claude 공통)
`commands/`에 모드 전환용 커맨드를 제공한다.
- conductor-plan : Plan 모드(읽기 전용, 질문/범위 확정)
- conductor-search : Search 모드(병렬 탐색 지시)
- conductor-implement : 구현 모드(작은 변경 + 검증)
- conductor-release : 배포 체크리스트
- conductor-ultrawork : 검색->계획->실행->검증->정리 강제

설치(심링크/복사):
```
mkdir -p ~/.claude/commands ~/.codex/commands
ln -s ~/.conductor-kit/commands/conductor-plan.md ~/.claude/commands/conductor-plan.md
ln -s ~/.conductor-kit/commands/conductor-search.md ~/.claude/commands/conductor-search.md
ln -s ~/.conductor-kit/commands/conductor-implement.md ~/.claude/commands/conductor-implement.md
ln -s ~/.conductor-kit/commands/conductor-release.md ~/.claude/commands/conductor-release.md
ln -s ~/.conductor-kit/commands/conductor-ultrawork.md ~/.claude/commands/conductor-ultrawork.md
ln -s ~/.conductor-kit/commands/conductor-plan.md ~/.codex/commands/conductor-plan.md
ln -s ~/.conductor-kit/commands/conductor-search.md ~/.codex/commands/conductor-search.md
ln -s ~/.conductor-kit/commands/conductor-implement.md ~/.codex/commands/conductor-implement.md
ln -s ~/.conductor-kit/commands/conductor-release.md ~/.codex/commands/conductor-release.md
ln -s ~/.conductor-kit/commands/conductor-ultrawork.md ~/.codex/commands/conductor-ultrawork.md
```

복사 방식(선호 시):
```
mkdir -p ~/.claude/commands ~/.codex/commands
cp -R ~/.conductor-kit/commands/*.md ~/.claude/commands/
cp -R ~/.conductor-kit/commands/*.md ~/.codex/commands/
```

## Codex CLI용 범위
- 확정(추천): `~/.codex/skills/**/SKILL.md` 형태의 "스킬 주입"은 최소 공통분모로 안전하다.
- 커맨드는 Codex에서도 가능하다는 전제로 `~/.codex/commands`에 동일 파일을 둔다.

## Helper CLI vs External Runner

### Current (skills + Go helper CLI)
- 장점: 스킬 중심 유지, 전역 설치/백그라운드/MCP를 간단히 제공
- 단점: 완전한 headless 오케스트레이션(캐시/정책 강제)은 제한

### Future (external runner, 별도 레포)
- 장점: 진짜 headless 오케스트레이션, 로그/상태/재시도/정책 강제 가능
- 단점: 호스트별 연동 안정성, 설치/업데이트/호환성 부담 증가

## 로드맵
- 1차: skills + Go helper CLI 안정화
- 2차: commands/role 모델링 강화, MCP 도구 확장
- 3차: 필요 시 외부 runner를 별도 레포로 분리
