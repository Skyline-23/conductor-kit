package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
)

var (
	mcpBundleNames = []string{"core", "gemini-cli", "claude-cli", "codex-cli"}
	claudeMCPNames = []string{"conductor", "gemini-cli", "claude-cli", "codex-cli", "gemini-cloud-assist"}
	codexMCPNames  = []string{"conductor", "gemini-cli", "claude-cli", "codex-cli", "gemini-cloud-assist"}
)

func claudeMCPPath(claudeHome string) string {
	return filepath.Join(claudeHome, ".mcp.json")
}

func ensureClaudeMCP(bundlesPath, claudeHome string, dryRun bool) error {
	path := claudeMCPPath(claudeHome)
	if dryRun {
		fmt.Printf("Register Claude MCP -> %s\n", path)
		return nil
	}
	cfg, err := loadClaudeMCP(path)
	if err != nil {
		return err
	}
	bundleConfig, err := loadBundleConfig(bundlesPath)
	if err != nil {
		return err
	}
	changed := false
	for _, name := range mcpBundleNames {
		bundle, ok := bundleConfig.Bundles[name]
		if !ok {
			return fmt.Errorf("missing MCP bundle: %s", name)
		}
		updated, err := upsertClaudeMCP(cfg, bundle)
		if err != nil {
			return err
		}
		if updated {
			changed = true
		}
	}
	if !changed {
		return nil
	}
	return writeClaudeMCP(path, cfg)
}

func removeClaudeMCP(claudeHome string, dryRun bool) error {
	path := claudeMCPPath(claudeHome)
	if dryRun {
		fmt.Printf("Remove Claude MCP -> %s\n", path)
		return nil
	}
	cfg, err := loadClaudeMCP(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	changed, err := deleteClaudeMCP(cfg)
	if err != nil {
		return err
	}
	if !changed {
		return nil
	}
	if len(cfg) == 0 {
		return os.Remove(path)
	}
	return writeClaudeMCP(path, cfg)
}

func ensureCodexMCP(bundlesPath string, dryRun bool) error {
	if dryRun {
		fmt.Println("Register Codex MCP -> codex mcp add conductor -- conductor mcp")
		fmt.Println("Register Codex MCP -> codex mcp add gemini-cli -- conductor mcp-gemini")
		fmt.Println("Register Codex MCP -> codex mcp add claude-cli -- conductor mcp-claude")
		fmt.Println("Register Codex MCP -> codex mcp add codex-cli -- conductor mcp-codex")
		return nil
	}
	if _, err := exec.LookPath("codex"); err != nil {
		fmt.Println("Codex CLI not found; skip MCP registration.")
		return nil
	}
	bundleConfig, err := loadBundleConfig(bundlesPath)
	if err != nil {
		return err
	}
	for _, name := range mcpBundleNames {
		bundle, ok := bundleConfig.Bundles[name]
		if !ok {
			return fmt.Errorf("missing MCP bundle: %s", name)
		}
		for _, server := range bundle.Servers {
			if !bundleEnabled(server) {
				continue
			}
			args := append([]string{}, server.Args...)
			cmdArgs := append([]string{"mcp", "add", server.Name, "--", server.Command}, args...)
			cmd := exec.Command("codex", cmdArgs...)
			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
			if err := cmd.Run(); err != nil {
				return err
			}
		}
	}
	return nil
}

func removeCodexMCP(dryRun bool) error {
	if dryRun {
		for _, name := range codexMCPNames {
			fmt.Printf("Remove Codex MCP -> codex mcp remove %s\n", name)
		}
		return nil
	}
	if _, err := exec.LookPath("codex"); err != nil {
		return nil
	}
	for _, name := range codexMCPNames {
		cmd := exec.Command("codex", "mcp", "remove", name)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			fmt.Printf("Codex MCP removal failed for %s: %v\n", name, err)
		}
	}
	return nil
}

func loadClaudeMCP(path string) (map[string]interface{}, error) {
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

func writeClaudeMCP(path string, cfg map[string]interface{}) error {
	ensureDir(filepath.Dir(path), false)
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return os.WriteFile(path, data, 0o644)
}

func upsertClaudeMCP(cfg map[string]interface{}, bundle Bundle) (bool, error) {
	mcpServers, changed, err := ensureClaudeMCPMap(cfg)
	if err != nil {
		return false, err
	}
	desired := buildClaudeServers(bundle)
	for name, server := range desired {
		if existing, ok := mcpServers[name]; ok && reflect.DeepEqual(existing, server) {
			continue
		}
		mcpServers[name] = server
		changed = true
	}
	return changed, nil
}

func deleteClaudeMCP(cfg map[string]interface{}) (bool, error) {
	mcpVal, ok := cfg["mcpServers"]
	if !ok {
		return false, nil
	}
	mcpServers, ok := mcpVal.(map[string]interface{})
	if !ok {
		return false, fmt.Errorf("Claude MCP config mcpServers is not an object")
	}
	changed := false
	for _, name := range claudeMCPNames {
		if _, ok := mcpServers[name]; !ok {
			continue
		}
		delete(mcpServers, name)
		changed = true
	}
	if !changed {
		return false, nil
	}
	if len(mcpServers) == 0 {
		delete(cfg, "mcpServers")
	}
	return true, nil
}

func ensureClaudeMCPMap(cfg map[string]interface{}) (map[string]interface{}, bool, error) {
	mcpVal, ok := cfg["mcpServers"]
	if !ok {
		mcp := map[string]interface{}{}
		cfg["mcpServers"] = mcp
		return mcp, true, nil
	}
	mcpServers, ok := mcpVal.(map[string]interface{})
	if !ok {
		return nil, false, fmt.Errorf("Claude MCP config mcpServers is not an object")
	}
	return mcpServers, false, nil
}

func buildClaudeServers(bundle Bundle) map[string]interface{} {
	out := map[string]interface{}{}
	for _, s := range bundle.Servers {
		if !bundleEnabled(s) {
			continue
		}
		server := map[string]interface{}{"command": s.Command}
		if len(s.Args) > 0 {
			server["args"] = s.Args
		}
		if len(s.Env) > 0 {
			server["env"] = s.Env
		}
		out[s.Name] = server
	}
	return out
}
