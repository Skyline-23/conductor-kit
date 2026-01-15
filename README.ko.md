# conductor-kit

**Codex CLI**, **Claude Code**, **Gemini CLI**를 위한 글로벌 스킬팩과 통합 MCP 서버입니다.

**언어**: [English](README.md) | 한국어

## 이게 뭔가요?

conductor-kit은 다음을 도와줍니다:
- 일관된 오케스트레이션 워크플로우 사용 (검색 → 계획 → 실행 → 검증)
- 여러 AI CLI 간의 작업 위임 (Codex, Claude, Gemini)
- 원하는 CLI에 전문 스킬과 커맨드 로드

## 빠른 시작

### 설치 (macOS)
```bash
brew tap Skyline-23/conductor-kit
brew install --cask conductor-kit
conductor install
```

### 수동 설치
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit
go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install
```

### 설치 확인
```bash
conductor status   # CLI 가용성 및 인증 상태 확인
conductor doctor   # 전체 진단
```

## 사용법

### 1. 스킬 사용
Claude Code 또는 Codex CLI에서 다음으로 트리거:
- `conductor` 또는 `ultrawork` 또는 `ulw`

### 2. 슬래시 커맨드
| Claude Code | Codex CLI |
|-------------|-----------|
| `/conductor-plan` | `/prompts:conductor-plan` |
| `/conductor-search` | `/prompts:conductor-search` |
| `/conductor-implement` | `/prompts:conductor-implement` |
| `/conductor-release` | `/prompts:conductor-release` |
| `/conductor-ultrawork` | `/prompts:conductor-ultrawork` |

### 3. MCP 도구 사용 (CLI 간 위임)

통합 MCP 서버 등록:

**Claude Code** (`~/.claude/mcp.json`):
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

**Codex CLI**:
```bash
codex mcp add conductor -- conductor mcp
```

사용 가능한 MCP 도구:
| 도구 | 설명 |
|------|------|
| `codex` | Codex CLI 세션 실행 |
| `codex-reply` | Codex 세션 계속 |
| `claude` | Claude Code 세션 실행 |
| `claude-reply` | Claude 세션 계속 |
| `gemini` | Gemini CLI 세션 실행 |
| `gemini-reply` | Gemini 세션 계속 |
| `conductor` | 역할 기반 라우팅 (config 사용) |
| `conductor-reply` | conductor 세션 계속 |
| `status` | CLI 가용성 및 인증 확인 |

프롬프트에서 사용 예시:
```
gemini 도구를 사용해서 인증 로직을 코드베이스에서 검색해줘
```

## 설정

설정 파일: `~/.conductor-kit/conductor.json`

### 역할 기반 라우팅 (기본값)
```json
{
  "roles": {
    "oracle": { "cli": "codex", "model": "gpt-5.2-codex", "reasoning": "medium" },
    "librarian": { "cli": "gemini", "model": "gemini-3-flash" },
    "explore": { "cli": "gemini", "model": "gemini-3-flash" },
    "frontend-ui-ux-engineer": { "cli": "gemini", "model": "gemini-3-pro" }
  }
}
```

### 설정 마법사
```bash
conductor settings              # 대화형 TUI 마법사
conductor settings --list-models --cli codex  # 사용 가능한 모델 목록
```

자세한 설정 옵션은 [CONFIGURATION.md](docs/CONFIGURATION.md)를 참조하세요.

## 요구 사항

- **호스트 CLI**: Codex CLI, Claude Code, Gemini CLI 중 최소 하나
- **Go 1.24+**: 소스에서 빌드할 때만 필요
- **macOS**: Homebrew cask 설치용 (Linux: 수동 설치 사용)

## 명령어 참조

| 명령어 | 설명 |
|--------|------|
| `conductor install` | CLI에 스킬/커맨드 설치 |
| `conductor uninstall` | 설치된 파일 제거 |
| `conductor status` | CLI 인증 및 가용성 확인 |
| `conductor doctor` | 전체 진단 |
| `conductor settings` | 역할 및 모델 설정 |
| `conductor mcp` | 통합 MCP 서버 시작 |

## 제거

```bash
# Homebrew
brew uninstall --cask conductor-kit

# 수동
conductor uninstall
```

## 라이선스

MIT
