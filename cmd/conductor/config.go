package main

import (
	"encoding/json"
	"os"
)

type Config struct {
	Roles map[string]RoleConfig `json:"roles"`
}

type RoleConfig struct {
	CLI           string       `json:"cli"`
	Args          []string     `json:"args"`
	ModelFlag     string       `json:"model_flag"`
	Model         string       `json:"model"`
	Models        []ModelEntry `json:"models"`
	ReasoningFlag string       `json:"reasoning_flag"`
	ReasoningKey  string       `json:"reasoning_key"`
	Reasoning     string       `json:"reasoning"`
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
