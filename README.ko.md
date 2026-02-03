# conductor-kit

**Codex CLI**, **Claude Code**, **Gemini CLI**를 위한 글로벌 스킬 팩과 통합 MCP 서버.

**Language**: [English](README.md) | 한국어

## 빠른 시작

1. 지원 CLI 중 하나 이상 설치
2. conductor-kit 설치
3. `conductor install` 후 `conductor status`로 확인

## 설치

npx (가장 빠름):
```bash
npx conductor-kit install
```

Homebrew (macOS):
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit
conductor install
```

npm 글로벌:
```bash
npm install -g conductor-kit
conductor install
```

소스 빌드:
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install
```

확인:
```bash
conductor doctor
conductor status
```

## 지원 CLI

| CLI | 설치 | 인증 |
|-----|------|------|
| **Claude Code** | `npm install -g @anthropic-ai/claude-code` | `claude` (안내 따라 진행) |
| **Codex CLI** | `npm install -g @openai/codex` | `codex --login` |
| **Gemini CLI** | `npm install -g @anthropic-ai/gemini-cli` | `gemini auth` |

## MCP 설정

Claude Code `~/.claude/mcp.json`:
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

Codex CLI:
```bash
codex mcp add conductor -- conductor mcp
```

OpenCode:
```bash
opencode mcp add conductor -- conductor mcp
```

설정 파일 위치:
- Codex: `~/.codex/config.toml` (또는 프로젝트 `.codex/config.toml`)
- OpenCode: `~/.config/opencode/opencode.json` (또는 프로젝트 `opencode.json`)

브리지 모드 참고:
- `conductor mcp`는 stdio 기반 통합 MCP 서버입니다.
- Codex(`codex mcp-server`)와 Claude 도구(`claude mcp serve`)를 브리지하고, Claude 프롬프트는 네이티브 CLI로 실행합니다.
- Claude 도구 승인 흐름은 MCP 클라이언트가 담당합니다.
- Codex `mcp-server`는 전역 설정 오버라이드를 상속합니다.
- 상위 MCP 서버가 없어도 경고 후 계속 동작합니다(`CONDUCTOR_BRIDGE_STRICT=1`이면 즉시 실패).

선택형 MCP 번들:
- 템플릿: `config/mcp-bundles.json`
- `~/.conductor-kit/mcp-bundles.json`에서 활성화 후 렌더링:
```bash
conductor mcp-bundle --host claude --bundle conductor
conductor mcp-bundle --host codex --bundle conductor
```

## 사용법

스킬 로드:
```bash
# Claude Code
claude
> Load the conductor skill
> sym
```

```bash
# Codex CLI
codex
> Load conductor
```

교차-CLI 프롬프트 예시:
```
Use the codex tool to analyze this algorithm with deep reasoning
```

```
Use the gemini tool to search the web for React 19 best practices
```

```
Use the conductor tool with role "sage" to solve this complex problem
```

사용 가능한 MCP 도구:

| 도구 | 설명 | 예시 |
|------|------|------|
| `codex` | Codex MCP 세션 실행(브리지) | 깊은 추론, 복잡한 분석 |
| `claude` | Claude Code 세션 실행(네이티브) | 코드 생성, 리팩토링 |
| `claude__*` | Claude Code 도구(브리지) | View/Edit/LS 등 |
| `gemini` | Gemini CLI 세션 실행 | 웹 검색, 리서치 |
| `conductor` | 역할 기반 라우팅 | 작업별 CLI 자동 선택 |
| `memory` | 공유 메모리 캐시 | 컨텍스트 저장/조회 |
| `codex-reply` / `claude-reply` / `gemini-reply` | 세션 계속 | 멀티턴 대화 |
| `status` | CLI 가용성 확인 | 진단 |

공유 메모리는 프로젝트 단위로 캐시되며(TTL + git HEAD 무효화), MCP 호출 시 자동으로 prepend 됩니다. `memory` 또는 `memory_key`/`memory_mode`를 사용하세요.

## 진단

Status 팁:
- `conductor status --skip-bridges`로 MCP 브리지 체크를 생략할 수 있습니다.
- `CONDUCTOR_BRIDGE=codex,claude|all|none`로 브리지 대상을 선택할 수 있습니다.
- `CONDUCTOR_BRIDGE_STRICT=1`로 브리지 실패 시 즉시 종료할 수 있습니다.
- `CONDUCTOR_BRIDGE_CACHE_TTL=30s`로 브리지 상태 캐시 시간을 조절할 수 있습니다.
- `CONDUCTOR_AUTH_CACHE_TTL=30s`로 CLI 인증 캐시 시간을 조절할 수 있습니다.
- `CONDUCTOR_ASYNC_LOG_MAX_BYTES=40000`로 async stdout/stderr 로그 크기를 제한할 수 있습니다.
- `CONDUCTOR_RUN_HISTORY_MAX_BYTES=10485760`로 run history 파일 크기를 제한할 수 있습니다.
- `CONDUCTOR_QUEUE_SNAPSHOT_MAX=200`으로 런타임 큐 스냅샷 크기를 제한할 수 있습니다.

`conductor status --json` 포함 항목:
- `ok`: 전체 상태
- `bridge_mode`: 활성 브리지(`codex,claude|none`)
- `bridge_targets`: 브리지 대상 목록
- `bridges_ok`: 브리지 프로브 종합 결과
- `bridges`: 브리지별 상태

`conductor doctor --json` 포함 항목:
- `ok`: 설정 + CLI/모델 체크 종합 상태
- `bridge_mode`: 활성 브리지(`codex,claude|none`)
- `bridge_targets`: 브리지 대상 목록
- `errors`: 설정 검증 에러
- `roles`: 역할별 진단

## 설정

설정 파일: `~/.conductor-kit/conductor.json` (또는 현재/상위 디렉토리의 `.conductor-kit/conductor.json`)

역할 기반 라우팅 예시:
```json
{
  "roles": {
    "sage": {
      "cli": "codex",
      "model": "gpt-5.2-codex",
      "reasoning": "medium",
      "description": "Deep reasoning for complex problems"
    },
    "scout": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "Web search and research"
    },
    "pathfinder": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "Codebase exploration and navigation"
    },
    "pixelator": {
      "cli": "gemini",
      "model": "gemini-3-pro",
      "description": "Web UI/UX design and frontend"
    }
  }
}
```

커스텀 role args 주의사항:
- Claude: `-p {prompt}`(또는 `--print {prompt}`)를 인접하게 배치
- Gemini: `-p`를 쓴다면 `-p {prompt}`로 인접하게 배치
- Claude/Gemini: `--output-format stream-json` 유지
- Codex: `--approval-policy` = `untrusted|on-request|on-failure|never`, `--sandbox` = `read-only|workspace-write|danger-full-access`

대화형 설정:
```bash
conductor settings
conductor settings --list-models --cli codex
```

## 슬래시 커맨드

Claude Code:

| 명령어 | 설명 |
|--------|------|
| `/conductor-plan` | 구현 계획 수립 |
| `/conductor-search` | 코드베이스 검색 위임 |
| `/conductor-implement` | 구현 + 검증 |
| `/conductor-debug` | 멀티-CLI 디버깅 |
| `/conductor-review` | 코드 리뷰 |
| `/conductor-release` | 릴리스 준비 |
| `/conductor-symphony` | 오케스트레이션 모드 |

Codex CLI (`/prompts:` 접두사):
```
/prompts:conductor-plan
/prompts:conductor-symphony
```

## 명령어

| 명령어 | 설명 |
|--------|------|
| `conductor install` | CLI에 스킬/커맨드 설치 |
| `conductor uninstall` | 설치된 파일 제거 |
| `conductor disable` | Conductor 비활성화 (스킬/커맨드 제거 + MCP 해제) |
| `conductor enable` | Conductor 활성화 (스킬/커맨드 복구 + MCP 등록) |
| `conductor status` | CLI 인증 및 가용성 확인 |
| `conductor roles` | role → CLI/model 매핑 목록 |
| `conductor config-validate` | config JSON 검증 |
| `conductor doctor` | 전체 진단 |
| `conductor settings` | 역할 및 모델 설정 |
| `conductor mcp-bundle` | MCP 번들 템플릿 렌더링 |
| `conductor mcp` | 통합 MCP 서버 시작 |
| `conductor help` | 명령어 도움말 |

## 문제 해결

"conductor: command not found":
```bash
which conductor
export PATH="$PATH:$(npm config get prefix)/bin"
```

MCP 도구가 나타나지 않음:
```bash
conductor status
```

CLI가 감지되지 않음:
```bash
conductor doctor
```

## 제거

```bash
brew uninstall --cask conductor-kit
npm uninstall -g conductor-kit
conductor uninstall
rm -rf ~/.conductor-kit
```

## 라이선스

MIT
