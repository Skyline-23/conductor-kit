# conductor-kit

Codex CLI와 Claude Code에서 공통으로 쓸 수 있는 **스킬팩 + Go 헬퍼**입니다.
오케스트레이션 루프(검색 -> 계획 -> 실행 -> 검증 -> 정리)를 강제하고, 필요 시 백그라운드 위임을 지원합니다.

**언어**: [English](README.md) | 한국어

## 빠른 설치 (Homebrew)
```bash
brew tap Skyline-23/conductor-kit
brew install conductor-kit

# Codex + Claude에 스킬/커맨드 설치
conductor install --mode link --repo $(brew --prefix)/share/conductor-kit
```

## 수동 설치
```bash
git clone https://github.com/Skyline-23/conductor-kit ~/.conductor-kit
cd ~/.conductor-kit

go build -o ~/.local/bin/conductor ./cmd/conductor
conductor install --mode link --repo ~/.conductor-kit
```

## 포함 기능
- **스킬**: `conductor` (`skills/conductor/SKILL.md`)
- **커맨드**: `conductor-plan`, `conductor-search`, `conductor-implement`, `conductor-release`, `conductor-ultrawork`
- **Go 헬퍼**: `conductor` 바이너리(설치/백그라운드/MCP)
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

### 3) 백그라운드 위임
```bash
# 설치된 CLI 자동 감지
conductor background-batch --prompt "<task>"

# 역할 기반 (config 기준)
conductor background-batch --roles auto --prompt "<task>"

# 모델 오버라이드 (단일 또는 콤마 리스트)
conductor background-batch --roles oracle \
  --model gpt-5.2-codex,gpt-5.2-codex-mini \
  --reasoning xhigh \
  --prompt "<task>"

# 결과 확인
conductor background-output --task-id <id>

# 전체 취소
conductor background-cancel --all
```

## MCP 도구 (선택)
```bash
codex mcp add conductor -- conductor mcp
```
도구 목록:
- `conductor.background_task`
- `conductor.background_batch`
- `conductor.background_output`
- `conductor.background_cancel`

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
