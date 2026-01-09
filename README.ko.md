# conductor-kit

Codex CLI와 Claude Code에서 공통으로 쓸 수 있는 **스킬팩 + Go 헬퍼**입니다.
오케스트레이션 루프(검색 -> 계획 -> 실행 -> 검증 -> 정리)를 강제하고, 필요 시 MCP 병렬 위임을 지원합니다.

**언어**: [English](README.md) | 한국어

## 빠른 설치 (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install conductor-kit

# Homebrew post_install이 Codex + Claude에 자동 링크함
# 필요 시 재실행:
conductor install --mode link --repo $(brew --prefix)/share/conductor-kit --force
```

## 수동 설치
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

## 요구 사항
- 호스트 CLI: Codex CLI 또는 Claude Code (스킬/커맨드는 해당 호스트 안에서 실행됨)
- 위임용 CLI를 최소 1개 PATH에 설치: `codex`, `claude`, `gemini` (config 역할과 일치)
- Go 1.23+ (소스에서 빌드할 때만 필요)
- MCP 도구 등록을 원하면 `codex` CLI 필요 (`codex mcp add ...`)

## 포함 기능
- **스킬**: `conductor` (`skills/conductor/SKILL.md`)
- **커맨드**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go 헬퍼**: `conductor` 바이너리(설치/MCP/위임 도구)
- **설정**: `~/.conductor-kit/conductor.json` (역할 -> CLI/모델 매핑)

## 사용법
### 1) 스킬 호출
`conductor`, `ultrawork` / `ulw`, “오케스트레이션” 등의 요청으로 트리거.

### 2) 커맨드 사용 (Codex + Claude)
- `/conductor-plan`
- `/conductor-search`
- `/conductor-implement`
- `/conductor-release`
- `/conductor-ultrawork`

### 3) 병렬 위임 (MCP 전용)
```bash
codex mcp add conductor -- conductor mcp
```

이후 도구 호출:
- `conductor.run` with `{ "role": "oracle", "prompt": "<task>" }`
- 여러 `conductor.run` 도구 호출을 병렬로 실행 (호스트가 병렬 처리)
- `conductor.run_batch` with `{ "roles": "oracle,librarian,explore", "prompt": "<task>" }`

참고: 위임 도구는 MCP 전용이며 CLI 서브커맨드는 제공하지 않습니다.

## 모델 설정 (roles)
`~/.conductor-kit/conductor.json`에서 역할 -> CLI/모델 라우팅을 설정합니다 (`config/conductor.json`에서 설치).
레포의 파일이 기본 템플릿이며, `conductor install`이 이를 `~/.conductor-kit/`로 링크/복사합니다.
`model`이 비어 있으면 모델 플래그를 전달하지 않아 각 CLI의 기본 모델을 사용합니다.

핵심 필드:
- `roles.<name>.cli`: 실행할 CLI (PATH에 있어야 함)
- `roles.<name>.args`: argv 템플릿; `{prompt}` 위치에 프롬프트 삽입
- `roles.<name>.model_flag`: 모델 플래그 (codex는 `-m`, claude/gemini는 `--model`)
- `roles.<name>.model`: 기본 모델 문자열 (선택)
- `roles.<name>.models`: `conductor.run_batch`용 fan-out 목록 (문자열 또는 `{ "name": "...", "reasoning_effort": "..." }`)
- `roles.<name>.reasoning_flag` / `reasoning_key` / `reasoning`: reasoning 설정 (codex는 `-c model_reasoning_effort`)

예시:
```json
{
  "roles": {
    "oracle": {
      "cli": "codex",
      "args": ["exec", "{prompt}"],
      "model_flag": "-m",
      "model": "gpt-5.2-codex",
      "reasoning_flag": "-c",
      "reasoning_key": "model_reasoning_effort",
      "reasoning": "high"
    }
  }
}
```

오버라이드:
- `conductor.run` with `{ "role": "<role>", "model": "<model>", "reasoning": "<level>", "prompt": "<task>" }`
- `conductor.run_batch` with `{ "roles": "<role(s)>", "model": "<model[,model]>", "reasoning": "<level>", "prompt": "<task>" }`
  (`model` 오버라이드는 `roles` 모드에서만 적용됨, `agents`는 제외)
- `conductor.run_batch` with `{ "config": "/path/to/conductor.json", "prompt": "<task>" }` 또는 `CONDUCTOR_CONFIG=/path/to/conductor.json`
팁: `~/.conductor-kit/conductor.json`을 직접 수정하고, 기본값으로 되돌리고 싶을 때만 `conductor install`을 재실행하세요.

## MCP 도구 (tool-calling UI 권장)
```bash
codex mcp add conductor -- conductor mcp
```
도구 목록:
- `conductor.run`
- `conductor.run_batch`

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
