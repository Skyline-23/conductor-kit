# conductor-kit

Codex CLI와 Claude Code에서 공통으로 쓸 수 있는 **스킬팩 + Go 헬퍼**입니다.
오케스트레이션 루프(검색 -> 계획 -> 실행 -> 검증 -> 정리)를 강제하고, 필요 시 MCP 병렬 위임을 지원합니다.

**언어**: [English](README.md) | 한국어

## 빠른 설치 (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit

# Homebrew post_install이 Codex + Claude에 자동 링크함
# 필요 시 재실행:
conductor install --mode link --repo "$(brew --prefix)/Caskroom/conductor-kit/$(brew list --cask --versions conductor-kit | awk '{print $2}')" --force
```

## 수동 설치
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

프로젝트 로컬 설치:
```bash
conductor install --mode link --repo ~/.conductor-kit --project
```

## 요구 사항
- 호스트 CLI: Codex CLI 또는 Claude Code (스킬/커맨드는 해당 호스트 안에서 실행됨)
- 위임용 CLI를 최소 1개 PATH에 설치: `codex`, `claude`, `gemini` (config 역할과 일치)
- Go 1.23+ (소스에서 빌드할 때만 필요)
- Homebrew cask 설치는 macOS 전용입니다 (Linux는 수동 설치 사용).
- MCP 도구 등록:
  - Codex CLI: `codex mcp add ...`
  - Claude Code: `~/.claude/.mcp.json` (아래 참고)

## 포함 기능
- **스킬**: `conductor` (`skills/conductor/SKILL.md`)
- **커맨드**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go 헬퍼**: `conductor` 바이너리(설치/MCP/위임 도구)
- **옵션 런타임**: 로컬 데몬(큐/승인 기반 비동기 실행)
- **설정**: `~/.conductor-kit/conductor.json` (역할 -> CLI/모델 매핑)

## 사용법
### 1) 스킬 호출
`conductor`, `ultrawork` / `ulw`, “오케스트레이션” 등의 요청으로 트리거.

### 2) 커맨드 사용
Claude Code (슬래시 커맨드):
- `/conductor-plan`
- `/conductor-search`
- `/conductor-implement`
- `/conductor-release`
- `/conductor-ultrawork`

Codex CLI (커스텀 프롬프트):
- `/prompts:conductor-plan`
- `/prompts:conductor-search`
- `/prompts:conductor-implement`
- `/prompts:conductor-release`
- `/prompts:conductor-ultrawork`
프롬프트는 `~/.codex/prompts` (또는 `$CODEX_HOME/prompts`)에 설치됩니다.

### 3) 병렬 위임 (MCP 전용)
Codex CLI:
```bash
codex mcp add conductor -- conductor mcp
```

Claude Code (`~/.claude/.mcp.json`):
```json
{
  "mcpServers": {
    "conductor": {
      "command": "conductor",
      "args": ["mcp"]
    }
  }
}
```

이후 도구 호출:
- `conductor.run` with `{ "role": "oracle", "prompt": "<task>" }`
- 여러 `conductor.run` 도구 호출을 병렬로 실행 (호스트가 병렬 처리)
- `conductor.run_batch` with `{ "roles": "oracle,librarian,explore", "prompt": "<task>" }`

참고: 위임 도구는 MCP 전용이며, CLI 서브커맨드는 데몬 설정용입니다.

### 4) 비동기 위임 (선택)
- 비동기 시작: `conductor.run_async` with `{ "role": "oracle", "prompt": "<task>" }`
- 배치 비동기: `conductor.run_batch_async` with `{ "roles": "oracle,librarian", "prompt": "<task>" }`
- 상태 확인: `conductor.run_status` with `{ "run_id": "<id>" }`
- 완료 대기: `conductor.run_wait` with `{ "run_id": "<id>", "timeout_ms": 120000 }`
- 취소: `conductor.run_cancel` with `{ "run_id": "<id>", "force": false }`

### 5) 로컬 데몬 (큐 + 승인, 선택)
로컬 데몬을 실행하면 비동기 작업을 큐잉하고 승인/목록 조회가 가능합니다.

```bash
conductor daemon --mode start --detach
conductor daemon --mode status
conductor daemon --mode stop
```

데몬이 실행 중이면 비동기 MCP 도구가 자동으로 데몬을 통해 동작합니다 (`no_daemon: true`로 우회 가능).
원격 데몬을 쓰려면 `CONDUCTOR_DAEMON_URL`을 설정하세요.
추가 도구:
- `conductor.queue_list` with `{ "status": "queued|running|awaiting_approval", "limit": 50 }`
- `conductor.approval_list`
- `conductor.approval_approve` with `{ "run_id": "<id>" }`
- `conductor.approval_reject` with `{ "run_id": "<id>" }`
- `conductor.daemon_status`

비동기 옵션:
- `require_approval: true` (강제 승인)
- `mode: "string"` (모드 해시 지정)
- `no_daemon: true` (데몬 우회)

## 모델 설정 (roles)
`~/.conductor-kit/conductor.json`에서 역할 -> CLI/모델 라우팅을 설정합니다 (`config/conductor.json`에서 설치).
레포의 파일이 기본 템플릿이며, `conductor install`이 이를 `~/.conductor-kit/`로 링크/복사합니다.
`model`이 비어 있으면 모델 플래그를 전달하지 않아 각 CLI의 기본 모델을 사용합니다.

핵심 필드:
- `defaults.timeout_ms` / `defaults.idle_timeout_ms` / `defaults.max_parallel` / `defaults.retry` / `defaults.retry_backoff_ms`: 런타임 기본값
- `defaults.log_prompt`: run history에 프롬프트 저장 (기본값: false)
- `routing.router_role`: `strategy=oracle`에서 사용할 라우팅 역할
- `routing.always`: 자동 라우팅 시 항상 포함할 역할 (예: `["oracle"]`)
- `daemon.host` / `daemon.port`: 로컬 데몬 바인딩 주소
- `daemon.max_parallel`: 데몬 동시 실행 제한 (기본값: `defaults.max_parallel`)
- `daemon.queue.on_mode_change`: `none` | `cancel_pending` | `cancel_running`
- `daemon.approval.required`: 전체 승인 강제
- `daemon.approval.roles` / `daemon.approval.agents`: 특정 역할/CLI 에이전트 승인 강제
- `roles.<name>.cli`: 실행할 CLI (PATH에 있어야 함)
- `roles.<name>.args`: argv 템플릿; `{prompt}` 위치에 프롬프트 삽입 (codex/claude/gemini는 생략 가능)
- `roles.<name>.model_flag`: 모델 플래그 (codex/claude/gemini는 생략 가능)
- `roles.<name>.model`: 기본 모델 문자열 (선택)
- `roles.<name>.models`: `conductor.run_batch`용 fan-out 목록 (문자열 또는 `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: reasoning 설정 (codex는 `-c model_reasoning_effort`)
- `roles.<name>.env` / `roles.<name>.cwd`: env/cwd 오버라이드
- `roles.<name>.timeout_ms` / `roles.<name>.idle_timeout_ms` / `roles.<name>.max_parallel` / `roles.<name>.retry` / `roles.<name>.retry_backoff_ms`: role 오버라이드

기본값(생략 시):
- `codex`: args `["exec","{prompt}"]`, model flag `-m`, reasoning flag `-c model_reasoning_effort`
- `claude`: args `["-p","{prompt}"]`, model flag `--model`
- `gemini`: args `["{prompt}"]`, model flag `--model`

템플릿 기본 모델 (CLI 네이티브 이름):
- `oracle`: codex `gpt-5.2-codex` + `reasoning: "medium"`
- `librarian`: gemini `gemini-3-flash-preview`
- `explore`: gemini `gemini-3-flash-preview`
- `frontend-ui-ux-engineer`: gemini `gemini-3-pro-preview`
- `document-writer`: gemini `gemini-3-flash-preview`
- `multimodal-looker`: gemini `gemini-3-flash-preview`

최소 예시:
```json
{
  "roles": {
    "oracle": {
      "cli": "codex"
    }
  }
}
```

오버라이드:
- `conductor.run` with `{ "role": "<role>", "model": "<model>", "reasoning": "<level>", "timeout_ms": 120000, "idle_timeout_ms": 30000, "prompt": "<task>" }`
- `conductor.run_batch` with `{ "roles": "<role(s)>", "model": "<model[,model]>", "reasoning": "<level>", "timeout_ms": 120000, "idle_timeout_ms": 30000, "prompt": "<task>" }`
- `conductor.run_batch` with `{ "config": "/path/to/conductor.json", "prompt": "<task>" }` 또는 `CONDUCTOR_CONFIG=/path/to/conductor.json`
팁: `~/.conductor-kit/conductor.json`을 직접 수정하고, 기본값으로 되돌리고 싶을 때만 `conductor install`을 재실행하세요.
스키마: `config/conductor.schema.json` (툴링용 선택 사항)

## 설정/로그인
- `conductor settings` (TUI 설정; 일반 프롬프트는 `--no-tui`)
- `conductor settings --list-models --cli codex` (모델 목록 출력)
- `conductor settings --role <role> --cli <cli> --model <model> --reasoning <effort>`
- `conductor login codex|claude|gemini` (CLI 로그인 실행)
- `conductor uninstall` (홈 디렉토리 설치물 제거)

## 프로젝트 로컬 오버라이드
- `./.conductor-kit/conductor.json`에 로컬 설정을 두면 글로벌 설정을 덮어씁니다.
- `conductor install --project`로 `./.claude`에 스킬/커맨드, `./.codex`에 프롬프트를 설치합니다.

## 진단
- `conductor config-validate` (`~/.conductor-kit/conductor.json` 유효성 검사)
- `conductor doctor` (설정 + CLI 가용성 + 모델명 기본 검증)

## 제거 (Homebrew)
```bash
brew uninstall --cask conductor-kit
```
cask uninstall 훅이 `conductor uninstall --force`를 실행해 사용자 설치물을 정리합니다.

## 관측/기록
- `conductor.run_history` with `{ "limit": 20 }`
- `conductor.run_info` with `{ "run_id": "<id>" }`
- `conductor.queue_list` with `{ "status": "queued|running|awaiting_approval" }`
- `conductor.approval_list` (승인 대기 목록)

## 선택적 MCP 번들
```bash
# Claude Code (.claude/.mcp.json)
conductor mcp-bundle --host claude --bundle core --repo /path/to/conductor-kit --out .claude/.mcp.json

# Codex CLI (codex mcp add 명령 출력)
conductor mcp-bundle --host codex --bundle core --repo /path/to/conductor-kit
```
번들 설정은 `~/.conductor-kit/mcp-bundles.json`에 설치됩니다.

## MCP 도구 (tool-calling UI 권장)
```bash
codex mcp add conductor -- conductor mcp
```
도구 목록:
- `conductor.run`
- `conductor.run_batch`
- `conductor.run_async`
- `conductor.run_batch_async`
- `conductor.run_status`
- `conductor.run_wait`
- `conductor.run_cancel`
- `conductor.run_history`
- `conductor.run_info`
- `conductor.queue_list` (daemon)
- `conductor.approval_list` (daemon)
- `conductor.approval_approve` (daemon)
- `conductor.approval_reject` (daemon)
- `conductor.daemon_status` (daemon)

## 레포 구조
```
conductor-kit/
  cmd/conductor/         # Go 헬퍼 CLI
  commands/              # Codex + Claude commands
  config/                # 기본 role/model 설정
  skills/conductor/      # 메인 스킬
```

## License
MIT
