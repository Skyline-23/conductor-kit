package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

func runDisable(args []string) int {
	return runToggleDisabled(args, true)
}

func runEnable(args []string) int {
	return runToggleDisabled(args, false)
}

func runToggleDisabled(args []string, disabled bool) int {
	name := "disable"
	state := "disabled"
	if !disabled {
		name = "enable"
		state = "enabled"
	}

	fs := flag.NewFlagSet(name, flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	configPath := fs.String("config", resolveConfigPath(""), "config path")
	jsonOut := fs.Bool("json", false, "output JSON")
	project := fs.Bool("project", false, "use project-local .claude/.codex/.opencode")
	codexHome := fs.String("codex-home", getenv("CODEX_HOME", filepath.Join(os.Getenv("HOME"), ".codex")), "codex home")
	claudeHome := fs.String("claude-home", filepath.Join(os.Getenv("HOME"), ".claude"), "claude home")
	opencodeHome := fs.String("opencode-home", defaultOpenCodeHome(), "opencode home")
	dryRun := fs.Bool("dry-run", false, "print actions only")
	cliFlag := fs.String("cli", "", "comma-separated CLIs to toggle (codex,claude,opencode)")
	mcpFlag := fs.String("mcp", "", "toggle MCP registration (mcp)")
	mode := fs.String("mode", "link", "link|copy (enable only)")
	repoRoot := fs.String("repo", "", "repo root (enable only)")
	force := fs.Bool("force", false, "overwrite existing files (enable only)")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}
	if !disabled && *mode != "link" && *mode != "copy" {
		fmt.Println("Invalid --mode (expected link or copy).")
		return 1
	}

	*configPath = expandPath(*configPath)
	*codexHome = expandPath(*codexHome)
	*claudeHome = expandPath(*claudeHome)
	*opencodeHome = expandPath(*opencodeHome)
	*repoRoot = expandPath(*repoRoot)

	cfg, err := loadConfig(*configPath)
	if err != nil {
		fmt.Println("Config error:", err.Error())
		return 1
	}

	projectRoot := ""
	if *project {
		cwd, _ := os.Getwd()
		projectRoot = cwd
	} else {
		projectRoot = projectRootFromConfigPath(*configPath)
	}
	if projectRoot != "" {
		*codexHome = filepath.Join(projectRoot, ".codex")
		*claudeHome = filepath.Join(projectRoot, ".claude")
		*opencodeHome = filepath.Join(projectRoot, ".opencode")
	}

	selectedCLIs := map[string]bool{
		"codex":    true,
		"claude":   true,
		"opencode": true,
	}
	if *cliFlag != "" {
		selectedCLIs = parseCLIFlag(*cliFlag)
	}
	selectedMCPs := map[string]bool{
		"mcp": true,
	}
	if *mcpFlag != "" {
		selectedMCPs = parseMCPFlag(*mcpFlag)
	}

	targets := resolveToggleTargets(selectedCLIs, *codexHome, *claudeHome, *opencodeHome)

	if disabled {
		if cfg.Disabled != disabled {
			cfg.Disabled = disabled
			if !*dryRun {
				if err := writeConfig(*configPath, cfg); err != nil {
					fmt.Println("Config write error:", err.Error())
					return 1
				}
			}
		}
		for _, t := range targets {
			skillsDir := filepath.Join(t.home, t.skillsDir, "conductor")
			removePath(skillsDir, *dryRun)
			commandsDir := filepath.Join(t.home, t.commandsDir)
			removeConductorCommands(commandsDir, *dryRun)
		}
		if selectedMCPs["mcp"] {
			configPath := openCodeConfigPath(*opencodeHome, projectRoot)
			if err := removeOpenCodeMCP(configPath, *dryRun); err != nil {
				fmt.Println("OpenCode MCP removal failed:", err.Error())
				return 1
			}
			if err := removeClaudeMCP(*claudeHome, *dryRun); err != nil {
				fmt.Println("Claude MCP removal failed:", err.Error())
				return 1
			}
			if err := removeCodexMCP(*dryRun); err != nil {
				fmt.Println("Codex MCP removal failed:", err.Error())
				return 1
			}
		}
	} else {
		if err := enableConductor(targets, *configPath, projectRoot, *mode, *repoRoot, *force, *dryRun, selectedMCPs, *codexHome, *claudeHome, *opencodeHome); err != nil {
			fmt.Println(err.Error())
			return 1
		}
		if cfg.Disabled != disabled {
			cfg.Disabled = disabled
			if !*dryRun {
				if err := writeConfig(*configPath, cfg); err != nil {
					fmt.Println("Config write error:", err.Error())
					return 1
				}
			}
		}
	}

	payload := map[string]interface{}{
		"status":   state,
		"disabled": cfg.Disabled,
		"config":   *configPath,
	}
	if *dryRun {
		payload["dry_run"] = true
	}
	if *jsonOut || !isTerminal(os.Stdout) {
		printJSON(payload)
		return 0
	}

	prefix := ""
	if *dryRun {
		prefix = "Dry run: "
	}
	if disabled {
		fmt.Printf("%sConductor %s (skills removed, MCP unregistered): %s\n", prefix, state, *configPath)
		return 0
	}
	fmt.Printf("%sConductor %s (skills restored, MCP registered): %s\n", prefix, state, *configPath)
	return 0
}

type toggleTarget struct {
	name        string
	home        string
	skillsDir   string
	commandsDir string
}

func resolveToggleTargets(selected map[string]bool, codexHome, claudeHome, opencodeHome string) []toggleTarget {
	all := []toggleTarget{
		{name: "codex", home: codexHome, skillsDir: "skills", commandsDir: "prompts"},
		{name: "claude", home: claudeHome, skillsDir: "skills", commandsDir: "commands"},
		{name: "opencode", home: opencodeHome, skillsDir: "skill", commandsDir: "command"},
	}
	filtered := []toggleTarget{}
	for _, t := range all {
		if selected[t.name] {
			filtered = append(filtered, t)
		}
	}
	return filtered
}

func projectRootFromConfigPath(configPath string) string {
	if configPath == "" {
		return ""
	}
	configPath = filepath.Clean(configPath)
	if filepath.Base(configPath) != "conductor.json" {
		return ""
	}
	configDir := filepath.Dir(configPath)
	if filepath.Base(configDir) != ".conductor-kit" {
		return ""
	}
	if home := os.Getenv("HOME"); home != "" {
		userConfigDir := filepath.Clean(filepath.Join(home, ".conductor-kit"))
		if filepath.Clean(configDir) == userConfigDir {
			return ""
		}
	}
	return filepath.Dir(configDir)
}

func enableConductor(targets []toggleTarget, configPath, projectRoot, mode, repoRoot string, force, dryRun bool, selectedMCPs map[string]bool, codexHome, claudeHome, opencodeHome string) error {
	root := repoRoot
	if root == "" {
		root = detectRepoRoot()
	}

	skillsSource := filepath.Join(root, "skills", "conductor")
	commandsSource := filepath.Join(root, "commands")
	configSource := filepath.Join(root, "config", "conductor.json")
	bundlesSource := filepath.Join(root, "config", "mcp-bundles.json")

	if len(targets) > 0 {
		if !pathExists(skillsSource) {
			return fmt.Errorf("Missing skills source: %s", skillsSource)
		}
		if !pathExists(commandsSource) {
			return fmt.Errorf("Missing commands source: %s", commandsSource)
		}
	}

	configDir := filepath.Dir(configPath)
	bundlesDest := filepath.Join(configDir, "mcp-bundles.json")

	if !pathExists(configPath) || force {
		if !pathExists(configSource) {
			return fmt.Errorf("Missing config source: %s", configSource)
		}
		if err := ensureDir(configDir, dryRun); err != nil {
			return err
		}
		_ = doLinkOrCopy(configSource, configPath, "copy", force, dryRun)
	}
	if pathExists(bundlesSource) && (!pathExists(bundlesDest) || force) {
		_ = doLinkOrCopy(bundlesSource, bundlesDest, "copy", force, dryRun)
	}
	if selectedMCPs["mcp"] && !pathExists(bundlesDest) && !pathExists(bundlesSource) {
		return fmt.Errorf("Missing MCP bundle: %s", bundlesSource)
	}

	commandFiles := []string{}
	if len(targets) > 0 {
		entries, err := os.ReadDir(commandsSource)
		if err != nil {
			return err
		}
		for _, entry := range entries {
			if entry.IsDir() {
				continue
			}
			name := entry.Name()
			if filepath.Ext(name) == ".md" {
				commandFiles = append(commandFiles, name)
			}
		}
	}

	for _, t := range targets {
		skillsDir := filepath.Join(t.home, t.skillsDir)
		if err := ensureDir(skillsDir, dryRun); err != nil {
			return err
		}
		_ = doLinkOrCopy(skillsSource, filepath.Join(skillsDir, "conductor"), mode, force, dryRun)

		commandsDir := filepath.Join(t.home, t.commandsDir)
		if err := ensureDir(commandsDir, dryRun); err != nil {
			return err
		}
		for _, name := range commandFiles {
			_ = doLinkOrCopy(filepath.Join(commandsSource, name), filepath.Join(commandsDir, name), mode, force, dryRun)
		}
	}

	if selectedMCPs["mcp"] {
		configPath := openCodeConfigPath(opencodeHome, projectRoot)
		if err := ensureOpenCodeMCP(configPath, dryRun); err != nil {
			return err
		}
		if err := ensureClaudeMCP(bundlesDest, claudeHome, dryRun); err != nil {
			return err
		}
		if err := ensureCodexMCP(bundlesDest, dryRun); err != nil {
			return err
		}
	}

	return nil
}
