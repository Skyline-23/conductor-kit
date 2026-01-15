package main

import (
	"testing"
)

func TestValidateConfig(t *testing.T) {
	tests := []struct {
		name       string
		cfg        Config
		wantErrors int
	}{
		{
			name:       "empty roles",
			cfg:        Config{},
			wantErrors: 1, // "roles is empty"
		},
		{
			name: "valid config",
			cfg: Config{
				Roles: map[string]RoleConfig{
					"oracle": {CLI: "codex"},
				},
			},
			wantErrors: 0,
		},
		{
			name: "negative max_parallel",
			cfg: Config{
				Defaults: Defaults{MaxParallel: -1},
				Roles: map[string]RoleConfig{
					"oracle": {CLI: "codex"},
				},
			},
			wantErrors: 1,
		},
		{
			name: "negative idle_timeout_ms",
			cfg: Config{
				Defaults: Defaults{IdleTimeoutMs: -1},
				Roles: map[string]RoleConfig{
					"oracle": {CLI: "codex"},
				},
			},
			wantErrors: 1,
		},
		{
			name: "negative retry",
			cfg: Config{
				Defaults: Defaults{Retry: -1},
				Roles: map[string]RoleConfig{
					"oracle": {CLI: "codex"},
				},
			},
			wantErrors: 1,
		},
		{
			name: "negative retry_backoff_ms",
			cfg: Config{
				Defaults: Defaults{RetryBackoffMs: -1},
				Roles: map[string]RoleConfig{
					"oracle": {CLI: "codex"},
				},
			},
			wantErrors: 1,
		},
		{
			name: "role missing cli",
			cfg: Config{
				Roles: map[string]RoleConfig{
					"oracle": {}, // missing CLI
				},
			},
			wantErrors: 1,
		},
		{
			name: "role negative max_parallel",
			cfg: Config{
				Roles: map[string]RoleConfig{
					"oracle": {CLI: "codex", MaxParallel: -1},
				},
			},
			wantErrors: 1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			errors := validateConfig(tt.cfg)
			if len(errors) != tt.wantErrors {
				t.Errorf("expected %d errors, got %d: %v", tt.wantErrors, len(errors), errors)
			}
		})
	}
}

func TestCheckModelForCLI(t *testing.T) {
	tests := []struct {
		cli      string
		model    string
		level    string
		contains string
	}{
		// Empty model
		{"codex", "", "ok", ""},

		// Valid models
		{"codex", "gpt-4", "ok", ""},
		{"codex", "gpt-5.2-codex", "ok", ""},
		{"claude", "claude-3-opus", "ok", ""},
		{"gemini", "gemini-2.5-pro", "ok", ""},
		{"gemini", "auto", "ok", ""},

		// Invalid - provider prefix
		{"codex", "openai/gpt-4", "error", "provider prefix"},
		{"claude", "anthropic/claude-3", "error", "provider prefix"},

		// Warnings - unexpected model names
		{"codex", "claude-3-opus", "warn", "unexpected for codex"},
		{"claude", "gpt-4", "warn", "unexpected for claude"},
		{"gemini", "gpt-4", "warn", "unexpected for gemini"},

		// Unknown CLI
		{"unknown", "model-123", "warn", "unknown cli"},
	}

	for _, tt := range tests {
		t.Run(tt.cli+"/"+tt.model, func(t *testing.T) {
			result := checkModelForCLI(tt.cli, tt.model)
			if result.level != tt.level {
				t.Errorf("expected level %q, got %q (message: %s)", tt.level, result.level, result.message)
			}
			if tt.contains != "" && result.message == "" {
				t.Errorf("expected message containing %q, got empty", tt.contains)
			}
		})
	}
}
