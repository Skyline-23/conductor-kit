// Package main provides the conductor CLI for managing skills, commands, and MCP bridges.
package main

import (
	"encoding/json"
	"fmt"
	"os"
)

// Config represents the root configuration structure loaded from conductor.json.
type Config struct {
	Defaults Defaults              `json:"defaults"`
	Roles    map[string]RoleConfig `json:"roles"`
	Runtime  RuntimeConfig         `json:"runtime"`
}

// Defaults contains global default settings for all roles.
type Defaults struct {
	IdleTimeoutMs  int  `json:"idle_timeout_ms"`
	MaxParallel    int  `json:"max_parallel"`
	Retry          int  `json:"retry"`
	RetryBackoffMs int  `json:"retry_backoff_ms"`
	LogPrompt      bool `json:"log_prompt"`
	SummaryOnly    bool `json:"summary_only"`
}

// RuntimeConfig controls queue and approval behavior for async runs.
type RuntimeConfig struct {
	MaxParallel int                   `json:"max_parallel"`
	Queue       RuntimeQueueConfig    `json:"queue"`
	Approval    RuntimeApprovalConfig `json:"approval"`
}

// RuntimeQueueConfig defines queue behavior settings.
type RuntimeQueueConfig struct {
	OnModeChange string `json:"on_mode_change"`
}

// RuntimeApprovalConfig defines which roles/agents require approval before execution.
type RuntimeApprovalConfig struct {
	Required bool     `json:"required"`
	Roles    []string `json:"roles"`
	Agents   []string `json:"agents"`
}

// RoleConfig defines a single role's CLI, model, and execution settings.
type RoleConfig struct {
	CLI            string            `json:"cli"`
	Args           []string          `json:"args"`
	ModelFlag      string            `json:"model_flag"`
	Model          string            `json:"model"`
	Models         []ModelEntry      `json:"models"`
	ReasoningFlag  string            `json:"reasoning_flag"`
	ReasoningKey   string            `json:"reasoning_key"`
	Reasoning      string            `json:"reasoning"`
	Description    string            `json:"description"`
	ReadyCmd       string            `json:"ready_cmd"`
	ReadyArgs      []string          `json:"ready_args"`
	ReadyTimeoutMs int               `json:"ready_timeout_ms"`
	Env            map[string]string `json:"env"`
	Cwd            string            `json:"cwd"`
	IdleTimeoutMs  int               `json:"idle_timeout_ms"`
	MaxParallel    int               `json:"max_parallel"`
	Retry          int               `json:"retry"`
	RetryBackoffMs int               `json:"retry_backoff_ms"`
}

// ModelEntry represents a model configuration with optional reasoning effort.
type ModelEntry struct {
	Name            string
	ReasoningEffort string
}

// roleDefaults holds CLI-specific default arguments and flags.
type roleDefaults struct {
	args          []string
	modelFlag     string
	reasoningFlag string
	reasoningKey  string
}

// resolveRoleDefaults returns CLI-specific defaults for codex, claude, or gemini.
func resolveRoleDefaults(cli string) (roleDefaults, bool) {
	switch cli {
	case "codex":
		return roleDefaults{
			args:          []string{"exec", "{prompt}"},
			modelFlag:     "-m",
			reasoningFlag: "-c",
			reasoningKey:  "model_reasoning_effort",
		}, true
	case "claude":
		return roleDefaults{
			args:      []string{"-p", "{prompt}"},
			modelFlag: "--model",
		}, true
	case "gemini":
		return roleDefaults{
			args:      []string{"{prompt}"},
			modelFlag: "--model",
		}, true
	default:
		return roleDefaults{}, false
	}
}

// normalizeRoleConfig applies CLI-specific defaults to a role configuration.
func normalizeRoleConfig(role RoleConfig) (RoleConfig, error) {
	if role.CLI == "" {
		return role, fmt.Errorf("Missing cli for role")
	}
	defaults, ok := resolveRoleDefaults(role.CLI)
	if len(role.Args) == 0 {
		if !ok {
			return role, fmt.Errorf("Missing args for cli: %s", role.CLI)
		}
		role.Args = append([]string{}, defaults.args...)
	}
	if role.ModelFlag == "" && ok {
		role.ModelFlag = defaults.modelFlag
	}
	if role.ReasoningFlag == "" && role.ReasoningKey == "" && ok {
		role.ReasoningFlag = defaults.reasoningFlag
		role.ReasoningKey = defaults.reasoningKey
	}
	return role, nil
}

func (m *ModelEntry) UnmarshalJSON(data []byte) error {
	if len(data) == 0 {
		return nil
	}
	if data[0] == '"' {
		var s string
		if err := json.Unmarshal(data, &s); err != nil {
			return err
		}
		m.Name = s
		return nil
	}
	var obj struct {
		Name            string `json:"name"`
		ReasoningEffort string `json:"reasoning_effort"`
		Reasoning       string `json:"reasoning"`
	}
	if err := json.Unmarshal(data, &obj); err != nil {
		return err
	}
	m.Name = obj.Name
	if obj.ReasoningEffort != "" {
		m.ReasoningEffort = obj.ReasoningEffort
	} else {
		m.ReasoningEffort = obj.Reasoning
	}
	return nil
}

// loadConfig reads and parses a conductor.json configuration file.
func loadConfig(path string) (Config, error) {
	var cfg Config
	data, err := os.ReadFile(path)
	if err != nil {
		return cfg, err
	}
	if err := json.Unmarshal(data, &cfg); err != nil {
		return cfg, err
	}
	return cfg, nil
}

// loadConfigOrEmpty loads config or returns empty Config if file doesn't exist.
func loadConfigOrEmpty(path string) (Config, error) {
	cfg, err := loadConfig(path)
	if err != nil {
		if os.IsNotExist(err) {
			return Config{}, nil
		}
		return Config{}, err
	}
	return cfg, nil
}

const (
	defaultIdleTimeoutMs  = 120000
	defaultMaxParallel    = 4
	defaultRetry          = 0
	defaultRetryBackoffMs = 500
)

// normalizeDefaults fills in missing default values with sensible defaults.
func normalizeDefaults(d Defaults) Defaults {
	if d.IdleTimeoutMs <= 0 {
		d.IdleTimeoutMs = defaultIdleTimeoutMs
	}
	if d.MaxParallel <= 0 {
		d.MaxParallel = defaultMaxParallel
	}
	if d.Retry < 0 {
		d.Retry = defaultRetry
	}
	if d.RetryBackoffMs < 0 {
		d.RetryBackoffMs = defaultRetryBackoffMs
	}
	return d
}

// effectiveInt returns roleVal if positive, otherwise defaultVal.
func effectiveInt(roleVal, defaultVal int) int {
	if roleVal > 0 {
		return roleVal
	}
	return defaultVal
}

// expandModelEntries builds a list of models from config or override values.
func expandModelEntries(cfg RoleConfig, modelOverride, reasoningOverride string) []ModelEntry {
	if modelOverride != "" {
		models := splitList(modelOverride)
		if len(models) == 0 {
			return []ModelEntry{{Name: modelOverride, ReasoningEffort: reasoningOverride}}
		}
		entries := []ModelEntry{}
		for _, model := range models {
			if model == "" {
				continue
			}
			entries = append(entries, ModelEntry{Name: model, ReasoningEffort: reasoningOverride})
		}
		if len(entries) > 0 {
			return entries
		}
		return []ModelEntry{{Name: modelOverride, ReasoningEffort: reasoningOverride}}
	}
	if len(cfg.Models) == 0 {
		return nil
	}
	entries := []ModelEntry{}
	for _, entry := range cfg.Models {
		entries = append(entries, entry)
	}
	return entries
}
