package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
)

var opencodeMCPNames = []string{"gemini-cli", "claude-cli", "codex-cli"}

func openCodeConfigPath(opencodeHome, projectRoot string) string {
	if projectRoot != "" {
		return filepath.Join(projectRoot, "opencode.json")
	}
	return filepath.Join(opencodeHome, "opencode.json")
}

func ensureOpenCodeMCP(configPath string, dryRun bool) error {
	if dryRun {
		fmt.Printf("Register OpenCode MCP -> %s\n", configPath)
		return nil
	}
	cfg, err := loadOpenCodeConfig(configPath)
	if err != nil {
		return err
	}
	changed, err := upsertOpenCodeMCP(cfg)
	if err != nil {
		return err
	}
	if !changed {
		return nil
	}
	return writeOpenCodeConfig(configPath, cfg)
}

func removeOpenCodeMCP(configPath string, dryRun bool) error {
	if dryRun {
		fmt.Printf("Remove OpenCode MCP -> %s\n", configPath)
		return nil
	}
	cfg, err := loadOpenCodeConfig(configPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	changed, err := deleteOpenCodeMCP(cfg)
	if err != nil {
		return err
	}
	if !changed {
		return nil
	}
	return writeOpenCodeConfig(configPath, cfg)
}

func loadOpenCodeConfig(path string) (map[string]interface{}, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]interface{}{}, nil
		}
		return nil, err
	}
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 {
		return map[string]interface{}{}, nil
	}
	var cfg map[string]interface{}
	if err := json.Unmarshal(stripJSONC(data), &cfg); err != nil {
		return nil, err
	}
	if cfg == nil {
		cfg = map[string]interface{}{}
	}
	return cfg, nil
}

func writeOpenCodeConfig(path string, cfg map[string]interface{}) error {
	ensureDir(filepath.Dir(path), false)
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return os.WriteFile(path, data, 0o644)
}

func upsertOpenCodeMCP(cfg map[string]interface{}) (bool, error) {
	changed := false

	// Ensure experimental.mcp_timeout
	experimental, expChanged := ensureExperimentalMap(cfg)
	if expChanged {
		changed = true
	}
	if experimental["mcp_timeout"] != float64(600000000) {
		experimental["mcp_timeout"] = 600000000
		changed = true
	}

	// Ensure mcp.conductor
	mcp, mcpChanged, err := ensureOpenCodeMCPMap(cfg)
	if err != nil {
		return false, err
	}
	if mcpChanged {
		changed = true
	}

	desired := map[string]interface{}{
		"type":    "local",
		"command": []string{"conductor", "mcp"},
		"enabled": true,
	}

	if existing, ok := mcp["conductor"]; !ok || !reflect.DeepEqual(existing, desired) {
		mcp["conductor"] = desired
		changed = true
	}

	return changed, nil
}

func ensureExperimentalMap(cfg map[string]interface{}) (map[string]interface{}, bool) {
	expVal, ok := cfg["experimental"]
	if !ok {
		exp := map[string]interface{}{}
		cfg["experimental"] = exp
		return exp, true
	}
	exp, ok := expVal.(map[string]interface{})
	if !ok {
		exp := map[string]interface{}{}
		cfg["experimental"] = exp
		return exp, true
	}
	return exp, false
}

func deleteOpenCodeMCP(cfg map[string]interface{}) (bool, error) {
	mcpVal, ok := cfg["mcp"]
	if !ok {
		return false, nil
	}
	mcp, ok := mcpVal.(map[string]interface{})
	if !ok {
		return false, fmt.Errorf("OpenCode config mcp is not an object")
	}
	changed := false
	for _, name := range opencodeMCPNames {
		if _, ok := mcp[name]; !ok {
			continue
		}
		delete(mcp, name)
		changed = true
	}
	if !changed {
		return false, nil
	}
	if len(mcp) == 0 {
		delete(cfg, "mcp")
	}
	return true, nil
}

func ensureOpenCodeMCPMap(cfg map[string]interface{}) (map[string]interface{}, bool, error) {
	mcpVal, ok := cfg["mcp"]
	if !ok {
		mcp := map[string]interface{}{}
		cfg["mcp"] = mcp
		return mcp, true, nil
	}
	mcp, ok := mcpVal.(map[string]interface{})
	if !ok {
		return nil, false, fmt.Errorf("OpenCode config mcp is not an object")
	}
	return mcp, false, nil
}

func stripJSONC(data []byte) []byte {
	out := make([]byte, 0, len(data))
	inString := false
	escape := false
	for i := 0; i < len(data); i++ {
		c := data[i]
		if inString {
			out = append(out, c)
			if escape {
				escape = false
				continue
			}
			switch c {
			case '\\':
				escape = true
			case '"':
				inString = false
			}
			continue
		}
		switch c {
		case '"':
			inString = true
			out = append(out, c)
			continue
		case '/':
			if i+1 < len(data) && data[i+1] == '/' {
				i += 2
				for i < len(data) && data[i] != '\n' {
					i++
				}
				if i < len(data) {
					out = append(out, data[i])
				}
				continue
			}
			if i+1 < len(data) && data[i+1] == '*' {
				i += 2
				for i+1 < len(data) && !(data[i] == '*' && data[i+1] == '/') {
					i++
				}
				if i+1 < len(data) {
					i++
				}
				continue
			}
		}
		out = append(out, c)
	}
	return out
}
