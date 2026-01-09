package main

import (
	"encoding/json"
	"os"
)

type Config struct {
	Defaults Defaults              `json:"defaults"`
	Roles    map[string]RoleConfig `json:"roles"`
	Daemon   DaemonConfig          `json:"daemon"`
}

type Defaults struct {
	TimeoutMs      int  `json:"timeout_ms"`
	MaxParallel    int  `json:"max_parallel"`
	Retry          int  `json:"retry"`
	RetryBackoffMs int  `json:"retry_backoff_ms"`
	LogPrompt      bool `json:"log_prompt"`
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
	ModelFlag      string            `json:"model_flag"`
	Model          string            `json:"model"`
	Models         []ModelEntry      `json:"models"`
	ReasoningFlag  string            `json:"reasoning_flag"`
	ReasoningKey   string            `json:"reasoning_key"`
	Reasoning      string            `json:"reasoning"`
	Env            map[string]string `json:"env"`
	Cwd            string            `json:"cwd"`
	TimeoutMs      int               `json:"timeout_ms"`
	MaxParallel    int               `json:"max_parallel"`
	Retry          int               `json:"retry"`
	RetryBackoffMs int               `json:"retry_backoff_ms"`
}

type ModelEntry struct {
	Name            string
	ReasoningEffort string
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
	defaultMaxParallel    = 4
	defaultRetry          = 0
	defaultRetryBackoffMs = 500
)

func normalizeDefaults(d Defaults) Defaults {
	if d.TimeoutMs <= 0 {
		d.TimeoutMs = defaultTimeoutMs
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
