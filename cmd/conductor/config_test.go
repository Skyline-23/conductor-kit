package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadConfig(t *testing.T) {
	content := `{
  "defaults": {
    "timeout_ms": 5000,
    "idle_timeout_ms": 60000,
    "max_parallel": 2
  },
  "roles": {
    "oracle": {
      "cli": "codex",
      "model": "gpt-4"
    }
  }
}`
	path := filepath.Join(t.TempDir(), "config.json")
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	cfg, err := loadConfig(path)
	if err != nil {
		t.Fatalf("loadConfig: %v", err)
	}

	if cfg.Defaults.TimeoutMs != 5000 {
		t.Errorf("expected timeout_ms=5000, got %d", cfg.Defaults.TimeoutMs)
	}
	if cfg.Defaults.IdleTimeoutMs != 60000 {
		t.Errorf("expected idle_timeout_ms=60000, got %d", cfg.Defaults.IdleTimeoutMs)
	}
	if cfg.Defaults.MaxParallel != 2 {
		t.Errorf("expected max_parallel=2, got %d", cfg.Defaults.MaxParallel)
	}

	oracle, ok := cfg.Roles["oracle"]
	if !ok {
		t.Fatalf("expected oracle role")
	}
	if oracle.CLI != "codex" {
		t.Errorf("expected cli=codex, got %s", oracle.CLI)
	}
	if oracle.Model != "gpt-4" {
		t.Errorf("expected model=gpt-4, got %s", oracle.Model)
	}
}

func TestLoadConfigOrEmpty(t *testing.T) {
	// Non-existent file should return empty config
	cfg, err := loadConfigOrEmpty("/nonexistent/path.json")
	if err != nil {
		t.Fatalf("loadConfigOrEmpty: %v", err)
	}
	if len(cfg.Roles) != 0 {
		t.Errorf("expected empty roles, got %d", len(cfg.Roles))
	}
}

func TestNormalizeRoleConfig(t *testing.T) {
	tests := []struct {
		name    string
		role    RoleConfig
		wantErr bool
		check   func(t *testing.T, role RoleConfig)
	}{
		{
			name:    "missing cli",
			role:    RoleConfig{},
			wantErr: true,
		},
		{
			name: "codex defaults",
			role: RoleConfig{CLI: "codex"},
			check: func(t *testing.T, role RoleConfig) {
				if len(role.Args) != 2 || role.Args[0] != "exec" {
					t.Errorf("expected codex args [exec, {prompt}], got %v", role.Args)
				}
				if role.ModelFlag != "-m" {
					t.Errorf("expected model_flag=-m, got %s", role.ModelFlag)
				}
				if role.ReasoningFlag != "-c" {
					t.Errorf("expected reasoning_flag=-c, got %s", role.ReasoningFlag)
				}
			},
		},
		{
			name: "claude defaults",
			role: RoleConfig{CLI: "claude"},
			check: func(t *testing.T, role RoleConfig) {
				if len(role.Args) != 2 || role.Args[0] != "-p" {
					t.Errorf("expected claude args [-p, {prompt}], got %v", role.Args)
				}
				if role.ModelFlag != "--model" {
					t.Errorf("expected model_flag=--model, got %s", role.ModelFlag)
				}
			},
		},
		{
			name: "gemini defaults",
			role: RoleConfig{CLI: "gemini"},
			check: func(t *testing.T, role RoleConfig) {
				if len(role.Args) != 1 || role.Args[0] != "{prompt}" {
					t.Errorf("expected gemini args [{prompt}], got %v", role.Args)
				}
				if role.ModelFlag != "--model" {
					t.Errorf("expected model_flag=--model, got %s", role.ModelFlag)
				}
			},
		},
		{
			name: "custom args preserved",
			role: RoleConfig{CLI: "codex", Args: []string{"custom", "args"}},
			check: func(t *testing.T, role RoleConfig) {
				if len(role.Args) != 2 || role.Args[0] != "custom" {
					t.Errorf("expected custom args preserved, got %v", role.Args)
				}
			},
		},
		{
			name:    "unknown cli without args",
			role:    RoleConfig{CLI: "unknown"},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := normalizeRoleConfig(tt.role)
			if tt.wantErr {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if tt.check != nil {
				tt.check(t, result)
			}
		})
	}
}

func TestNormalizeDefaults(t *testing.T) {
	tests := []struct {
		name     string
		input    Defaults
		expected Defaults
	}{
		{
			name:  "all zeros get defaults",
			input: Defaults{},
			expected: Defaults{
				TimeoutMs:      0,
				IdleTimeoutMs:  120000,
				MaxParallel:    4,
				Retry:          0,
				RetryBackoffMs: 500,
			},
		},
		{
			name: "custom values preserved",
			input: Defaults{
				TimeoutMs:      10000,
				IdleTimeoutMs:  30000,
				MaxParallel:    8,
				Retry:          3,
				RetryBackoffMs: 1000,
			},
			expected: Defaults{
				TimeoutMs:      10000,
				IdleTimeoutMs:  30000,
				MaxParallel:    8,
				Retry:          3,
				RetryBackoffMs: 1000,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := normalizeDefaults(tt.input)
			if result.IdleTimeoutMs != tt.expected.IdleTimeoutMs {
				t.Errorf("idle_timeout_ms: expected %d, got %d", tt.expected.IdleTimeoutMs, result.IdleTimeoutMs)
			}
			if result.MaxParallel != tt.expected.MaxParallel {
				t.Errorf("max_parallel: expected %d, got %d", tt.expected.MaxParallel, result.MaxParallel)
			}
		})
	}
}

func TestModelEntryUnmarshalJSON(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected ModelEntry
	}{
		{
			name:     "string model",
			input:    `"gpt-4"`,
			expected: ModelEntry{Name: "gpt-4"},
		},
		{
			name:     "object with name",
			input:    `{"name": "gpt-4", "reasoning_effort": "high"}`,
			expected: ModelEntry{Name: "gpt-4", ReasoningEffort: "high"},
		},
		{
			name:     "object with reasoning alias",
			input:    `{"name": "gpt-4", "reasoning": "medium"}`,
			expected: ModelEntry{Name: "gpt-4", ReasoningEffort: "medium"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var entry ModelEntry
			if err := entry.UnmarshalJSON([]byte(tt.input)); err != nil {
				t.Fatalf("unmarshal: %v", err)
			}
			if entry.Name != tt.expected.Name {
				t.Errorf("name: expected %s, got %s", tt.expected.Name, entry.Name)
			}
			if entry.ReasoningEffort != tt.expected.ReasoningEffort {
				t.Errorf("reasoning_effort: expected %s, got %s", tt.expected.ReasoningEffort, entry.ReasoningEffort)
			}
		})
	}
}

func TestExpandModelEntries(t *testing.T) {
	cfg := RoleConfig{
		Model:     "gpt-4",
		Reasoning: "medium",
		Models: []ModelEntry{
			{Name: "gpt-4"},
			{Name: "gpt-4-turbo"},
		},
	}

	// With model override
	entries := expandModelEntries(cfg, "custom-model", "high")
	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}
	if entries[0].Name != "custom-model" || entries[0].ReasoningEffort != "high" {
		t.Errorf("unexpected entry: %+v", entries[0])
	}

	// With comma-separated models
	entries = expandModelEntries(cfg, "model1,model2", "")
	if len(entries) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(entries))
	}

	// Without override, use config models
	entries = expandModelEntries(cfg, "", "")
	if len(entries) != 2 {
		t.Fatalf("expected 2 entries from config, got %d", len(entries))
	}
}
