package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type BundleConfig struct {
	Bundles map[string]Bundle `json:"bundles"`
}

type Bundle struct {
	Servers []BundleServer `json:"servers"`
}

type BundleServer struct {
	Name    string            `json:"name"`
	Command string            `json:"command"`
	Args    []string          `json:"args"`
	Env     map[string]string `json:"env,omitempty"`
	Enabled *bool             `json:"enabled,omitempty"`
	Note    string            `json:"note,omitempty"`
}

func runMCPBundle(args []string) int {
	fs := flag.NewFlagSet("mcp-bundle", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	host := fs.String("host", "claude", "claude|codex")
	bundleName := fs.String("bundle", "core", "bundle name")
	configPath := fs.String("config", "", "bundle config path (default: repo/config/mcp-bundles.json)")
	repoRoot := fs.String("repo", "", "repo root (default: cwd)")
	outPath := fs.String("out", "", "write output to file instead of stdout")
	if err := fs.Parse(args); err != nil {
		fmt.Println(mcpBundleHelp())
		return 1
	}

	if *repoRoot == "" {
		cwd, _ := os.Getwd()
		*repoRoot = cwd
	}
	if *configPath == "" {
		baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
		homePath := filepath.Join(baseDir, "mcp-bundles.json")
		if pathExists(homePath) {
			*configPath = homePath
		} else {
			*configPath = filepath.Join(*repoRoot, "config", "mcp-bundles.json")
		}
	}

	cfg, err := loadBundleConfig(*configPath)
	if err != nil {
		fmt.Println("Bundle config error:", err.Error())
		return 1
	}
	bundle, ok := cfg.Bundles[*bundleName]
	if !ok {
		fmt.Println("Unknown bundle:", *bundleName)
		return 1
	}

	output := ""
	switch strings.ToLower(*host) {
	case "claude":
		output, err = renderClaudeBundle(bundle)
	case "codex":
		output, err = renderCodexBundle(bundle)
	default:
		fmt.Println("Invalid --host. Use claude or codex.")
		return 1
	}
	if err != nil {
		fmt.Println("Render error:", err.Error())
		return 1
	}

	if *outPath != "" {
		if err := os.WriteFile(*outPath, []byte(output), 0o644); err != nil {
			fmt.Println("Write error:", err.Error())
			return 1
		}
		return 0
	}
	fmt.Print(output)
	return 0
}

func mcpBundleHelp() string {
	return `conductor mcp-bundle

Usage:
  conductor mcp-bundle --host claude|codex --bundle <name> [--out PATH]
                       [--config PATH] [--repo PATH]
`
}

func loadBundleConfig(path string) (BundleConfig, error) {
	var cfg BundleConfig
	data, err := os.ReadFile(path)
	if err != nil {
		return cfg, err
	}
	if err := json.Unmarshal(data, &cfg); err != nil {
		return cfg, err
	}
	return cfg, nil
}

func renderClaudeBundle(bundle Bundle) (string, error) {
	type server struct {
		Command string            `json:"command"`
		Args    []string          `json:"args,omitempty"`
		Env     map[string]string `json:"env,omitempty"`
	}
	out := map[string]server{}
	for _, s := range bundle.Servers {
		if !bundleEnabled(s) {
			continue
		}
		out[s.Name] = server{
			Command: s.Command,
			Args:    s.Args,
			Env:     s.Env,
		}
	}
	payload := map[string]interface{}{"mcpServers": out}
	data, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return "", err
	}
	return string(data) + "\n", nil
}

func renderCodexBundle(bundle Bundle) (string, error) {
	lines := []string{}
	for _, s := range bundle.Servers {
		if !bundleEnabled(s) {
			continue
		}
		args := append([]string{}, s.Args...)
		line := fmt.Sprintf("codex mcp add %s -- %s", s.Name, strings.Join(append([]string{s.Command}, args...), " "))
		lines = append(lines, line)
	}
	return strings.Join(lines, "\n") + "\n", nil
}

func bundleEnabled(s BundleServer) bool {
	if s.Command == "" {
		return false
	}
	if s.Enabled != nil && !*s.Enabled {
		return false
	}
	return true
}
