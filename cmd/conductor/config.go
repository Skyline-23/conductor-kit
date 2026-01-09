package main

import (
	"encoding/json"
	"fmt"
	"os"
)

type Config struct {
	Defaults Defaults              `json:"defaults"`
	Routing  RoutingConfig         `json:"routing"`
	Roles    map[string]RoleConfig `json:"roles"`
	Daemon   DaemonConfig          `json:"daemon"`
}

type Defaults struct {
	TimeoutMs      int  `json:"timeout_ms"`
	IdleTimeoutMs  int  `json:"idle_timeout_ms"`
	MaxParallel    int  `json:"max_parallel"`
	Retry          int  `json:"retry"`
	RetryBackoffMs int  `json:"retry_backoff_ms"`
	LogPrompt      bool `json:"log_prompt"`
	SummaryOnly    bool `json:"summary_only"`
}

type RoutingConfig struct {
	RouterRole string   `json:"router_role"`
	Always     []string `json:"always"`
}

type DaemonConfig struct {
	Host        string         `json:"host"`
	Port        int            `json:"port"`
	MaxParallel int            `json:"max_parallel"`
	Queue       QueueConfig    `json:"queue"`
	Approval    ApprovalConfig `json:"approval"`
}

type QueueConfig struct {
	OnModeChange string `json:"on_mode_change"`
}

type ApprovalConfig struct {
	Required bool     `json:"required"`
	Roles    []string `json:"roles"`
	Agents   []string `json:"agents"`
}

type RoleConfig struct {
	CLI            string            `json:"cli"`
	Args           []string          `json:"args"`
	ReadyCmd       string            `json:"ready_cmd"`
	ReadyArgs      []string          `json:"ready_args"`
	ReadyTimeoutMs int               `json:"ready_timeout_ms"`
	ModelFlag      string            `json:"model_flag"`
	Model          string            `json:"model"`
	Models         []ModelEntry      `json:"models"`
	ReasoningFlag  string            `json:"reasoning_flag"`
	ReasoningKey   string            `json:"reasoning_key"`
	Reasoning      string            `json:"reasoning"`
	Env            map[string]string `json:"env"`
	Cwd            string            `json:"cwd"`
	TimeoutMs      int               `json:"timeout_ms"`
	IdleTimeoutMs  int               `json:"idle_timeout_ms"`
	MaxParallel    int               `json:"max_parallel"`
	Retry          int               `json:"retry"`
	RetryBackoffMs int               `json:"retry_backoff_ms"`
}

type ModelEntry struct {
	Name            string
	ReasoningEffort string
}

type roleDefaults struct {
	args          []string
	modelFlag     string
	reasoningFlag string
	reasoningKey  string
}

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
	defaultTimeoutMs      = 120000
	defaultIdleTimeoutMs  = 0
	defaultMaxParallel    = 4
	defaultRetry          = 0
	defaultRetryBackoffMs = 500
)

func normalizeDefaults(d Defaults) Defaults {
	if d.TimeoutMs <= 0 {
		d.TimeoutMs = defaultTimeoutMs
	}
	if d.IdleTimeoutMs < 0 {
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

func effectiveInt(roleVal, defaultVal int) int {
	if roleVal > 0 {
		return roleVal
	}
	return defaultVal
}

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
