# 설정 가이드

conductor-kit은 JSON 설정 파일을 사용하여 역할 기반 CLI 라우팅, 타임아웃 및 기타 런타임 동작을 제어합니다.

## 설정 파일 위치

| 우선순위 | 위치 | 설명 |
|----------|------|------|
| 1 | `CONDUCTOR_CONFIG` 환경변수 | 사용자 지정 경로 |
| 2 | `./.conductor-kit/conductor.json` | 프로젝트 로컬 설정 |
| 3 | `~/.conductor-kit/conductor.json` | 전역 사용자 설정 |

## 기본 역할 (Roles)

| 역할 | CLI | 설명 |
|------|-----|------|
| `sage` | codex | 복잡한 문제에 대한 심층 추론 |
| `scout` | gemini | 웹 검색 및 리서치 |
| `pathfinder` | gemini | 코드베이스 탐색 및 네비게이션 |
| `pixelator` | gemini | 웹 UI/UX 디자인 및 프론트엔드 |
| `author` | gemini | 문서화 및 기술 문서 작성 |
| `vision` | gemini | 이미지 및 스크린샷 분석 |

## 전체 설정 스키마

```json
{
  "defaults": {
    "idle_timeout_ms": 120000,
    "summary_only": false,
    "max_parallel": 4,
    "retry": 0,
    "retry_backoff_ms": 500,
    "log_prompt": false
  },
  "roles": {
    "role-name": {
      "cli": "codex|claude|gemini",
      "model": "model-name",
      "description": "역할 설명",
      "model_flag": "--model",
      "args": ["-p", "{prompt}"],
      "reasoning": "low|medium|high",
      "reasoning_flag": "-c model_reasoning_effort",
      "reasoning_key": "model_reasoning_effort",
      "models": ["model1", "model2"],
      "env": { "KEY": "value" },
      "cwd": "/path/to/dir",
      "idle_timeout_ms": 120000,
      "max_parallel": 4,
      "retry": 0,
      "retry_backoff_ms": 500
    }
  }
}
```

## Defaults 섹션

모든 역할에 적용되는 전역 기본값입니다.

| 필드 | 타입 | 기본값 | 설명 |
|------|------|--------|------|
| `idle_timeout_ms` | int | `120000` | 비활성 타임아웃 (2분) |
| `summary_only` | bool | `false` | 전체 출력 대신 요약 반환 |
| `max_parallel` | int | `4` | 최대 동시 CLI 실행 수 |
| `retry` | int | `0` | 실패 시 재시도 횟수 |
| `retry_backoff_ms` | int | `500` | 재시도 간 대기 시간 |
| `log_prompt` | bool | `false` | 실행 기록에 프롬프트 저장 |

## Roles 섹션

각 역할은 프롬프트를 특정 CLI로 라우팅하는 방법을 정의합니다.

### 필수 필드

| 필드 | 타입 | 설명 |
|------|------|------|
| `cli` | string | CLI 실행 파일: `codex`, `claude`, 또는 `gemini` |

### 선택 필드

| 필드 | 타입 | 설명 |
|------|------|------|
| `model` | string | 사용할 모델 (CLI 네이티브 이름) |
| `description` | string | 역할 설명 (TUI에 표시됨) |
| `model_flag` | string | 모델 전달 플래그. CLI별 기본값 있음 |
| `args` | array | 커스텀 argv 템플릿. `{prompt}` 플레이스홀더 사용 |
| `reasoning` | string | 추론 수준: `low`, `medium`, `high` (Codex만) |
| `reasoning_flag` | string | 추론 설정 플래그 |
| `reasoning_key` | string | 추론 설정 키 |
| `models` | array | 배치 팬아웃용 모델 목록 |
| `env` | object | 환경 변수 오버라이드 |
| `cwd` | string | 작업 디렉토리 오버라이드 |

## CLI 기본값

`args`와 `model_flag`가 생략되면 다음 기본값이 사용됩니다:

| CLI | Args | Model Flag | Reasoning Flag |
|-----|------|------------|----------------|
| `codex` | `["exec", "{prompt}"]` | `-m` | `-c model_reasoning_effort` |
| `claude` | `["-p", "{prompt}"]` | `--model` | - |
| `gemini` | `["{prompt}"]` | `--model` | - |

## 예제

### 최소 설정

```json
{
  "roles": {
    "sage": {
      "cli": "codex"
    }
  }
}
```

### 기본 설정 (conductor 설치 시)

```json
{
  "defaults": {
    "idle_timeout_ms": 120000,
    "max_parallel": 4,
    "retry": 0,
    "retry_backoff_ms": 500,
    "log_prompt": false
  },
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
    },
    "author": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "Documentation and technical writing"
    },
    "vision": {
      "cli": "gemini",
      "model": "gemini-3-flash",
      "description": "Image and screenshot analysis"
    }
  }
}
```

### 멀티 모델 배치 설정

```json
{
  "roles": {
    "sage": {
      "cli": "codex",
      "models": [
        { "name": "gpt-5.2-codex", "reasoning_effort": "high" },
        { "name": "o3", "reasoning_effort": "medium" }
      ]
    }
  }
}
```

### 커스텀 CLI Args

```json
{
  "roles": {
    "custom-claude": {
      "cli": "claude",
      "args": ["-p", "{prompt}", "--max-turns", "5", "--permission-mode", "bypassPermissions"],
      "model": "sonnet"
    }
  }
}
```

### 환경 변수 및 작업 디렉토리

```json
{
  "roles": {
    "secure-runner": {
      "cli": "codex",
      "env": {
        "CODEX_SANDBOX": "docker"
      },
      "cwd": "/secure/workspace"
    }
  }
}
```

## 설정 검증

```bash
# 설정 문법 검증
conductor config-validate

# 전체 진단 (설정 + CLI 가용성 + 모델 이름)
conductor doctor
```

## 환경 변수

| 변수 | 설명 |
|------|------|
| `CONDUCTOR_CONFIG` | 설정 파일 경로 오버라이드 |

## 팁

1. **최소한으로 시작**: 필요한 것만 지정하세요. 기본값이 잘 작동합니다.
2. **`conductor settings` 사용**: 쉬운 편집을 위한 대화형 TUI.
3. **`conductor doctor`로 확인**: 설정과 CLI 가용성을 검증합니다.
4. **프로젝트 로컬 오버라이드**: 레포 루트에 `.conductor-kit/conductor.json`을 넣으세요.
