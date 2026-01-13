package main

import (
	"fmt"
	"os"
	"os/exec"
)

var (
	mcpBundleNames = []string{"gemini-cli", "claude-cli", "codex-cli"}
	claudeMCPNames = []string{"gemini-cli", "claude-cli", "codex-cli"}
	codexMCPNames  = []string{"gemini-cli", "claude-cli", "codex-cli"}
)

func ensureClaudeMCP(bundlesPath, claudeHome string, dryRun bool) error {
	if dryRun {
		fmt.Println("Register Claude MCP -> claude mcp add -s user gemini-cli -- conductor mcp-gemini")
		fmt.Println("Register Claude MCP -> claude mcp add -s user claude-cli -- conductor mcp-claude")
		fmt.Println("Register Claude MCP -> claude mcp add -s user codex-cli -- conductor mcp-codex")
		return nil
	}
	if _, err := exec.LookPath("claude"); err != nil {
		fmt.Println("Claude CLI not found; skip MCP registration.")
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
			args := append([]string{"mcp", "add", "-s", "user", server.Name, "--"}, server.Command)
			args = append(args, server.Args...)
			cmd := exec.Command("claude", args...)
			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
			if err := cmd.Run(); err != nil {
				return err
			}
		}
	}
	return nil
}

func removeClaudeMCP(claudeHome string, dryRun bool) error {
	if dryRun {
		for _, name := range claudeMCPNames {
			fmt.Printf("Remove Claude MCP -> claude mcp remove %s\n", name)
		}
		return nil
	}
	if _, err := exec.LookPath("claude"); err != nil {
		return nil
	}
	for _, name := range claudeMCPNames {
		cmd := exec.Command("claude", "mcp", "remove", name)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			fmt.Printf("Claude MCP removal failed for %s: %v\n", name, err)
		}
	}
	return nil
}

func ensureCodexMCP(bundlesPath string, dryRun bool) error {
	if dryRun {
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
