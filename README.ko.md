# conductor-kit

**Codex CLI**, **Claude Code**, **Gemini CLI**를 위한 글로벌 스킬팩과 통합 MCP 서버입니다.

**언어**: [English](README.md) | 한국어

## 이게 뭔가요?

conductor-kit은 AI CLI 도구들이 함께 원활하게 작동하도록 합니다:

- **Cross-CLI 위임**: Claude가 Codex에게 추론을, Gemini에게 웹 검색을 위임
- **통합 스킬 시스템**: 하나의 스킬이 모든 지원 CLI에서 작동
- **역할 기반 라우팅**: 작업을 최적의 CLI/모델 조합으로 자동 라우팅
- **MCP 통합**: 도구 상호운용성을 위한 완전한 Model Context Protocol 지원

## 설치

### 옵션 1: npx (가장 쉬움)
```bash
npx conductor-kit install
```

### 옵션 2: Homebrew (macOS)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit
conductor install
```

### 옵션 3: npm 글로벌
```bash
npm install -g conductor-kit
conductor install
```

### 옵션 4: 소스에서 빌드
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install
```

### 설치 확인
```bash
conductor doctor   # 전체 진단
conductor status   # CLI 가용성 확인
```

---

## 튜토리얼: 시작하기

### 1단계: AI CLI 최소 하나 설치

conductor-kit은 다음 CLI들과 작동합니다:

| CLI | 설치 | 인증 |
|-----|------|------|
| **Claude Code** | `npm install -g @anthropic-ai/claude-code` | `claude` (프롬프트 따라하기) |
| **Codex CLI** | `npm install -g @openai/codex` | `codex --login` |
| **Gemini CLI** | `npm install -g @anthropic-ai/gemini-cli` | `gemini auth` |

### 2단계: 설치 프로그램 실행

```bash
conductor install
```

이 명령은:
- 설치된 CLI 감지
- `~/.claude/skills/` 및/또는 `~/.codex/skills/`에 스킬 복사
- `~/.claude/commands/` 및/또는 `~/.codex/prompts/`에 슬래시 커맨드 복사
- `~/.conductor-kit/conductor.json`에 설정 파일 생성

프로젝트 로컬 설치를 원하면:
```bash
conductor install --project
```

### 3단계: 스킬 로드

선호하는 CLI를 시작하고 conductor 스킬을 트리거하세요:

```bash
# Claude Code에서
claude
> conductor 스킬 로드해줘
> sym  # 단축 트리거
```

```bash
# Codex CLI에서
codex
> conductor 로드
```

스킬은 오케스트레이션 가이드와 역할 기반 위임 패턴을 제공합니다.

---

## 튜토리얼: MCP로 Cross-CLI 위임

conductor-kit의 진정한 힘은 한 CLI가 MCP 도구를 통해 다른 CLI를 호출하는 것입니다.

### 1단계: MCP 서버 등록

**Claude Code용** - `~/.claude/mcp.json`에 추가:
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

**Codex CLI용**:
```bash
codex mcp add conductor -- conductor mcp
```

**OpenCode용**:
```bash
opencode mcp add conductor -- conductor mcp
```

참고:
- Codex 설정 파일: `~/.codex/config.toml` (또는 프로젝트 `.codex/config.toml`).
- OpenCode 설정 파일: `~/.config/opencode/opencode.json` (또는 프로젝트 `opencode.json`).

### 브리지 모드

- `conductor mcp`는 어떤 MCP 클라이언트에도 연결 가능한 stdio MCP 서버입니다.
- `conductor mcp`는 Codex(`codex mcp-server`)와 Claude 도구(`claude mcp serve`)를 브리지하고, Claude 프롬프트는 네이티브 CLI로 실행합니다.
- Claude Code MCP 서버는 View/Edit/LS 같은 도구를 노출하며, MCP 클라이언트가 승인 흐름을 책임집니다.
- Codex `mcp-server`는 전역 설정 오버라이드를 상속하므로, 필요 시 Codex 설정에서 승인/샌드박스를 지정하세요.
- Codex `app-server`는 MCP가 아닌 별도 JSON-RPC 프로토콜입니다.
- `conductor mcp`는 상위 MCP 서버를 시작/연결하지 못하면 즉시 실패합니다.
- OpenCode는 MCP 클라이언트이므로 `opencode mcp add`로 로컬/원격 서버에 연결하세요.

Status 팁:
- `conductor status --skip-bridges`로 MCP 브리지 체크를 생략할 수 있습니다(더 빠름).
- `CONDUCTOR_BRIDGE_CACHE_TTL=30s`로 브리지 상태 캐시 시간을 조절할 수 있습니다.
- `CONDUCTOR_AUTH_CACHE_TTL=30s`로 CLI 인증 캐시 시간을 조절할 수 있습니다.
- `CONDUCTOR_ASYNC_LOG_MAX_BYTES=40000`로 async stdout/stderr 로그 크기를 제한할 수 있습니다.
- `CONDUCTOR_RUN_HISTORY_MAX_BYTES=10485760`로 run history 파일 크기를 제한할 수 있습니다.

### MCP 번들 템플릿(선택)

`config/mcp-bundles.json`에 선택형 템플릿이 포함되어 있습니다. `conductor`는 바로 등록 가능한 Conductor 서버를 렌더링하고, `extended`는 추가 MCP 서버를 넣는 스캐폴드입니다.

`~/.conductor-kit/mcp-bundles.json`에서 원하는 서버를 활성화한 뒤, 호스트별로 렌더링하세요:
```bash
conductor mcp-bundle --host claude --bundle conductor
conductor mcp-bundle --host codex --bundle conductor
```

### 2단계: 프롬프트에서 cross-CLI 도구 사용

이제 Claude에게 다른 CLI에 위임하도록 요청할 수 있습니다:

```
codex 도구를 사용해서 이 알고리즘을 깊은 추론으로 분석해줘
```

```
gemini 도구를 사용해서 React 19 모범 사례를 웹에서 검색해줘
```

```
conductor 도구를 "sage" 역할로 사용해서 이 복잡한 문제를 해결해줘
```

### 사용 가능한 MCP 도구

| 도구 | 설명 | 예시 |
|------|------|------|
| `codex` | Codex MCP 세션 실행(브리지) | 깊은 추론, 복잡한 분석 |
| `claude` | Claude Code 세션 실행(네이티브 CLI) | 코드 생성, 리팩토링 |
| `claude__*` | Claude Code 도구(브리지) | View/Edit/LS 등 |
| `gemini` | Gemini CLI 세션 실행 | 웹 검색, 리서치 |
| `conductor` | 역할 기반 라우팅 | 작업에 최적의 CLI 자동 선택 |
| `memory` | 공유 메모리 캐시 | 공유 컨텍스트 저장/조회 |
| `codex-reply` / `claude-reply` / `gemini-reply` | 세션 계속 | 멀티턴 대화 |
| `status` | CLI 가용성 확인 | 진단 |

공유 메모리는 프로젝트 단위로 캐시됩니다(TTL + git HEAD 변경 시 무효화). MCP 호출에 자동으로 prepend 되며, `memory`로 갱신하거나 `memory_key`/`memory_mode`로 추가 키를 주입하세요.

### 예시: 멀티-CLI 워크플로우

```
새로운 인증 시스템을 구현해야 해.

1. gemini 도구로 2025년 OAuth 2.0 모범 사례 리서치
2. codex 도구로 추론과 함께 아키텍처 설계
3. 그 다음 여기 Claude에서 구현
```

---

## 튜토리얼: 슬래시 커맨드 사용

슬래시 커맨드는 일반적인 워크플로우에 빠르게 접근할 수 있게 합니다.

### Claude Code에서

| 커맨드 | 설명 |
|--------|------|
| `/conductor-plan` | 구현 계획 생성 |
| `/conductor-search` | 위임으로 코드베이스 검색 |
| `/conductor-implement` | 검증과 함께 구현 |
| `/conductor-debug` | 멀티-CLI 분석으로 디버그 |
| `/conductor-review` | 코드 리뷰 워크플로우 |
| `/conductor-release` | 릴리즈 준비 |
| `/conductor-symphony` | 풀 오케스트레이션 모드 |

### Codex CLI에서

커맨드 앞에 `/prompts:` 붙이기:
```
/prompts:conductor-plan
/prompts:conductor-symphony
```

---

## 설정

설정 파일: `~/.conductor-kit/conductor.json` (현재/상위 디렉터리의 `.conductor-kit/conductor.json` 우선)

### 역할 기반 라우팅

역할은 작업 유형을 CLI/모델 조합에 매핑합니다:

```json
{
  "roles": {
    "sage": {
      "cli": "codex",
      "model": "gpt-5.2-codex",
      "reasoning": "medium",
      "description": "복잡한 문제를 위한 깊은 추론"
    },
    "scout": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "웹 검색과 리서치"
    },
    "pathfinder": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "코드베이스 탐색과 네비게이션"
    },
    "pixelator": {
      "cli": "gemini",
      "model": "gemini-3-pro",
      "description": "웹 UI/UX 디자인과 프론트엔드"
    }
  }
}
```

커스텀 role args 주의사항:
- Claude: `-p {prompt}`(또는 `--print {prompt}`)를 붙여서 사용
- Gemini: `-p`를 쓴다면 `-p {prompt}`로 붙여서 사용
- Claude/Gemini: `--output-format stream-json` 유지 (세션 ID 파싱용)
- Codex: `--approval-policy` = `untrusted|on-request|on-failure|never`, `--sandbox` = `read-only|workspace-write|danger-full-access`.

### 대화형 설정

```bash
conductor settings              # TUI 마법사
conductor settings --list-models --cli codex  # 모델 목록
```

---

## 명령어 참조

| 명령어 | 설명 |
|--------|------|
| `conductor install` | CLI에 스킬/커맨드 설치 |
| `conductor uninstall` | 설치된 파일 제거 |
| `conductor disable` | Conductor 비활성화 (스킬/커맨드 제거 + MCP 해제) |
| `conductor enable` | Conductor 활성화 (스킬/커맨드 복구 + MCP 등록) |
| `conductor status` | CLI 인증 및 가용성 확인 |
| `conductor doctor` | 전체 진단 |
| `conductor settings` | 역할 및 모델 설정 |
| `conductor mcp` | 통합 MCP 서버 시작 |

---

## 문제 해결

### "conductor: command not found"

바이너리가 PATH에 있는지 확인:
```bash
# 설치 위치 확인
which conductor

# 필요하면 PATH에 추가 (npm 글로벌용)
export PATH="$PATH:$(npm config get prefix)/bin"
```

### MCP 도구가 나타나지 않음

1. MCP 설정 추가 후 CLI 재시작
2. MCP 서버 작동 확인:
   ```bash
   conductor status
   ```

### CLI가 감지되지 않음

진단 실행:
```bash
conductor doctor
```

어떤 CLI가 설치되고 인증되었는지 표시됩니다.

---

## 제거

```bash
# Homebrew
brew uninstall --cask conductor-kit

# npm
npm uninstall -g conductor-kit

# 수동 정리
conductor uninstall
rm -rf ~/.conductor-kit
```

---

## 라이선스

MIT
