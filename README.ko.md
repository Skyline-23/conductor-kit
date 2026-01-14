# conductor-kit

Codex CLI와 Claude Code에서 공통으로 쓸 수 있는 **스킬팩 + Go 헬퍼**이며, OpenCode 전역 설치도 지원합니다.
오케스트레이션 루프(검색 -> 계획 -> 실행 -> 검증 -> 정리)를 강제하고, CLI MCP 브릿지로 위임을 지원합니다.

**언어**: [English](README.md) | 한국어

## 빠른 설치 (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit

# 설치 실행 (Homebrew Caskroom 자동 감지, CLI 선택 프롬프트)
conductor install
```

## 수동 설치
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

대화형 설치 (CLI 선택 프롬프트):
```bash
conductor install --interactive --mode link --repo ~/.conductor-kit
```

선택적 설치 (CLI 지정):
```bash
conductor install --cli codex,claude --mode link --repo ~/.conductor-kit
```

프로젝트 로컬 설치(.claude/.codex/.opencode):
```bash
conductor install --mode link --repo ~/.conductor-kit --project
```

## 요구 사항
- 호스트 CLI: Codex CLI, Claude Code, 또는 OpenCode (스킬/커맨드는 해당 호스트 안에서 실행됨)
- 위임용 CLI를 최소 1개 PATH에 설치: `codex`, `claude`, `gemini` (config 역할과 일치)
- Go 1.23+ (소스에서 빌드할 때만 필요)
- Homebrew cask 설치는 macOS 전용입니다 (Linux는 수동 설치 사용).
- MCP 도구 등록: `conductor install`이 Codex + Claude + OpenCode에 자동 등록 (gemini-cli + claude-cli + codex-cli 번들)

## 포함 기능
- **스킬**: `conductor` (`skills/conductor/SKILL.md`)
- **커맨드**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go 헬퍼**: `conductor` 바이너리(설치/MCP 브릿지)
- **CLI MCP 브릿지**: gemini/claude/codex CLI 래퍼
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

OpenCode (슬래시 커맨드):
- `/conductor-plan`
- `/conductor-search`
- `/conductor-implement`
- `/conductor-release`
- `/conductor-ultrawork`
커맨드는 `~/.config/opencode/command` (또는 `./.opencode/command`)에 설치됩니다.
스킬은 `~/.config/opencode/skill` (또는 `./.opencode/skill`)에 설치됩니다.

### 3) CLI MCP 브릿지
Codex CLI:
```bash
codex mcp add gemini-cli -- conductor mcp-gemini
codex mcp add claude-cli -- conductor mcp-claude
codex mcp add codex-cli -- conductor mcp-codex
```

Claude Code (`~/.claude/.mcp.json`):
```json
{
  "mcpServers": {
    "gemini-cli": {
      "command": "conductor",
      "args": ["mcp-gemini"]
    },
    "claude-cli": {
      "command": "conductor",
      "args": ["mcp-claude"]
    },
    "codex-cli": {
      "command": "conductor",
      "args": ["mcp-codex"]
    }
  }
}
```

이후 도구 호출:
- `gemini.prompt` with `{ "prompt": "<task>", "model": "gemini-2.5-flash" }`
- `gemini.batch` with `{ "prompt": "<task>", "models": "gemini-2.5-flash,gemini-2.5-pro" }`
- `gemini.auth_status`
- `claude.prompt` with `{ "prompt": "<task>", "model": "claude-3-5-sonnet" }`
- `claude.batch` with `{ "prompt": "<task>", "models": "claude-3-5-sonnet,claude-3-5-haiku" }`
- `claude.auth_status`
- `codex.prompt` with `{ "prompt": "<task>", "model": "gpt-5.2-codex" }`
- `codex.batch` with `{ "prompt": "<task>", "models": "gpt-5.2-codex,gpt-4.1" }`
- `codex.auth_status`

참고: Gemini MCP는 Gemini CLI 로그인만 사용합니다 (gcloud ADC 불필요).
참고: Claude MCP는 Claude CLI 로그인을 사용하며 permission-mode 기본값은 dontAsk입니다.
참고: Codex MCP는 Codex CLI 로그인을 사용합니다 (codex exec --json).
참고: 위임 도구는 MCP 전용이며, CLI 서브커맨드는 설치/설정/진단용입니다.

## 모델 설정 (roles)
`~/.conductor-kit/conductor.json`에서 역할 -> CLI/모델 라우팅을 설정합니다 (`config/conductor.json`에서 설치).
레포의 파일이 기본 템플릿이며, `conductor install`이 이를 `~/.conductor-kit/`로 링크/복사합니다.
`model`이 비어 있으면 모델 플래그를 전달하지 않아 각 CLI의 기본 모델을 사용합니다.

핵심 필드:
- `defaults.timeout_ms` / `defaults.idle_timeout_ms` / `defaults.max_parallel` / `defaults.retry` / `defaults.retry_backoff_ms`: 런타임 기본값 (`timeout_ms=0`이면 하드 타임아웃 비활성화, `idle_timeout_ms`는 무응답 기준)
- `defaults.log_prompt`: run history에 프롬프트 저장 (기본값: false)
- `roles.<name>.cli`: 실행할 CLI (PATH에 있어야 함)
- `roles.<name>.args`: argv 템플릿; `{prompt}` 위치에 프롬프트 삽입 (codex/claude/gemini는 생략 가능)
- `roles.<name>.model_flag`: 모델 플래그 (codex/claude/gemini는 생략 가능)
- `roles.<name>.model`: 기본 모델 문자열 (선택)
- `roles.<name>.models`: 배치 fan-out 목록 (문자열 또는 `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: reasoning 설정 (codex는 `-c model_reasoning_effort`)
- `roles.<name>.env` / `roles.<name>.cwd`: env/cwd 오버라이드
- `conductor status`: CLI 인증 확인을 우선적으로 직접 호출(`auth status`/`whoami`/`status`)하고, 지원되지 않으면 로컬 저장소 검사로 폴백 (codex: `~/.codex/auth.json`, gemini: `~/.gemini/oauth_creds.json` 또는 키체인, claude: 키체인 `Claude Code-credentials`)
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
- `CONDUCTOR_CONFIG=/path/to/conductor.json`로 커스텀 설정 지정
팁: `~/.conductor-kit/conductor.json`을 직접 수정하고, 기본값으로 되돌리고 싶을 때만 `conductor install`을 재실행하세요.
스키마: `config/conductor.schema.json` (툴링용 선택 사항)

## 설정/로그인
- `conductor settings` (TUI 설정; 일반 프롬프트는 `--no-tui`)
- `conductor settings --list-models --cli codex` (모델 목록 출력)
- `conductor settings --role <role> --cli <cli> --model <model> --reasoning <effort>`
- `conductor status` (CLI 가용성/준비 상태 확인)
- `conductor uninstall` (홈 디렉토리 설치물 제거, OpenCode 포함)

## 프로젝트 로컬 오버라이드
- `./.conductor-kit/conductor.json`에 로컬 설정을 두면 글로벌 설정을 덮어씁니다.
- `conductor install --project`로 `./.claude`에 스킬/커맨드, `./.codex`에 프롬프트, `./.opencode`에 OpenCode 커맨드/스킬을 설치합니다.

## 진단
- `conductor config-validate` (`~/.conductor-kit/conductor.json` 유효성 검사)
- `conductor doctor` (설정 + CLI 가용성 + 모델명 기본 검증)

## 제거 (Homebrew)
```bash
brew uninstall --cask conductor-kit
```
cask uninstall 훅이 `conductor uninstall --force`를 실행해 사용자 설치물을 정리합니다.

## 관측/기록
- `conductor status` (CLI 인증/가용성 확인)
- CLI MCP 브릿지 출력(`gemini.prompt`, `claude.prompt`, `codex.prompt`)

## 선택적 MCP 번들
```bash
# Claude Code (.claude/.mcp.json)
conductor mcp-bundle --host claude --bundle gemini-cli --repo /path/to/conductor-kit --out .claude/.mcp.json
conductor mcp-bundle --host claude --bundle claude-cli --repo /path/to/conductor-kit --out .claude/.mcp.json
conductor mcp-bundle --host claude --bundle codex-cli --repo /path/to/conductor-kit --out .claude/.mcp.json

# Codex CLI (codex mcp add 명령 출력)
conductor mcp-bundle --host codex --bundle gemini-cli --repo /path/to/conductor-kit
conductor mcp-bundle --host codex --bundle claude-cli --repo /path/to/conductor-kit
conductor mcp-bundle --host codex --bundle codex-cli --repo /path/to/conductor-kit
```
번들 설정은 `~/.conductor-kit/mcp-bundles.json`에 설치됩니다.

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
