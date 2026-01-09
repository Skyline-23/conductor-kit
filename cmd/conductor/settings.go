package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
)

func runSettings(args []string) int {
	fs := flag.NewFlagSet("settings", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	configPath := fs.String("config", resolveConfigPath(""), "config path")
	role := fs.String("role", "", "role name")
	cli := fs.String("cli", "", "cli name")
	model := fs.String("model", "", "default model")
	reasoning := fs.String("reasoning", "", "reasoning effort")
	list := fs.Bool("list", false, "list roles")
	if err := fs.Parse(args); err != nil {
		fmt.Println(settingsHelp())
		return 1
	}

	path := resolveConfigPath(*configPath)
	cfg, err := loadConfig(path)
	if err != nil {
		fmt.Println("Config error:", err.Error())
		return 1
	}

	if *list || (*role == "" && *cli == "" && *model == "" && *reasoning == "") {
		printRolesSummary(cfg, path)
		return 0
	}

	if *role == "" {
		fmt.Println("Missing --role.")
		fmt.Println(settingsHelp())
		return 1
	}

	if cfg.Roles == nil {
		cfg.Roles = map[string]RoleConfig{}
	}
	roleCfg := cfg.Roles[*role]
	if *cli != "" {
		roleCfg.CLI = *cli
	}
	if *model != "" {
		roleCfg.Model = *model
	}
	if *reasoning != "" {
		roleCfg.Reasoning = *reasoning
	}
	cfg.Roles[*role] = roleCfg

	if err := writeConfig(path, cfg); err != nil {
		fmt.Println("Config write error:", err.Error())
		return 1
	}

	if roleCfg.CLI != "" && roleCfg.Model != "" {
		check := checkModelForCLI(roleCfg.CLI, roleCfg.Model)
		if check.level != "ok" {
			fmt.Printf("Warning: model %s (%s)\n", roleCfg.Model, check.message)
		}
	}

	fmt.Printf("Updated %s\n", path)
	return 0
}

func settingsHelp() string {
	return `conductor settings

Usage:
  conductor settings --list
  conductor settings --role <name> [--cli <cli>] [--model <model>] [--reasoning <effort>]
  conductor settings --config /path/to/conductor.json --role <name> --model <model>
`
}

func printRolesSummary(cfg Config, path string) {
	fmt.Printf("Config: %s\n", path)
	if len(cfg.Roles) == 0 {
		fmt.Println("Roles: (none)")
		return
	}
	roles := make([]string, 0, len(cfg.Roles))
	for name := range cfg.Roles {
		roles = append(roles, name)
	}
	sort.Strings(roles)
	for _, name := range roles {
		role := cfg.Roles[name]
		fmt.Printf("- %s: cli=%s model=%s reasoning=%s\n", name, role.CLI, role.Model, role.Reasoning)
	}
}

func writeConfig(path string, cfg Config) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	out, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, out, 0o644)
}
