package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

var conductorCommandFiles = []string{
	"conductor-plan.md",
	"conductor-search.md",
	"conductor-implement.md",
	"conductor-release.md",
	"conductor-ultrawork.md",
}

func runUninstall(args []string) int {
	fs := flag.NewFlagSet("uninstall", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	skillsOnly := fs.Bool("skills-only", false, "remove only skills")
	commandsOnly := fs.Bool("commands-only", false, "remove only commands")
	project := fs.Bool("project", false, "uninstall from local project .claude/.codex/.opencode")
	codexHome := fs.String("codex-home", getenv("CODEX_HOME", filepath.Join(os.Getenv("HOME"), ".codex")), "codex home")
	claudeHome := fs.String("claude-home", filepath.Join(os.Getenv("HOME"), ".claude"), "claude home")
	opencodeHome := fs.String("opencode-home", defaultOpenCodeHome(), "opencode home")
	binDir := fs.String("bin-dir", getenv("CONDUCTOR_BIN", filepath.Join(os.Getenv("HOME"), ".local", "bin")), "bin dir")
	noBins := fs.Bool("no-bins", false, "skip bin removal")
	noConfig := fs.Bool("no-config", false, "skip config removal")
	force := fs.Bool("force", false, "remove bin copies as well as symlinks")
	dryRun := fs.Bool("dry-run", false, "print actions only")

	if err := fs.Parse(args); err != nil {
		fmt.Println(uninstallHelp())
		return 1
	}
	if *skillsOnly && *commandsOnly {
		fmt.Println("--skills-only and --commands-only are mutually exclusive.")
		return 1
	}

	configDest := filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")
	bundlesDest := filepath.Join(os.Getenv("HOME"), ".conductor-kit", "mcp-bundles.json")

	if *project {
		cwd, _ := os.Getwd()
		*codexHome = filepath.Join(cwd, ".codex")
		*claudeHome = filepath.Join(cwd, ".claude")
		*opencodeHome = filepath.Join(cwd, ".opencode")
		configDest = filepath.Join(cwd, ".conductor-kit", "conductor.json")
		bundlesDest = filepath.Join(cwd, ".conductor-kit", "mcp-bundles.json")
	}

	removeCommands := *commandsOnly || !*skillsOnly
	removeSkills := !*commandsOnly

	targets := []struct {
		name        string
		home        string
		skillsDir   string
		commandsDir string
	}{
		{"codex", *codexHome, "skills", "prompts"},
		{"claude", *claudeHome, "skills", "commands"},
		{"opencode", *opencodeHome, "skill", "command"},
	}

	for _, t := range targets {
		if removeSkills {
			skillsDir := filepath.Join(t.home, t.skillsDir, "conductor")
			removePath(skillsDir, *dryRun)
		}
		if removeCommands {
			commandsDir := filepath.Join(t.home, t.commandsDir)
			for _, name := range conductorCommandFiles {
				removePath(filepath.Join(commandsDir, name), *dryRun)
			}
		}
	}

	if !*noBins {
		for _, name := range conductorBinAliases() {
			removeBin(filepath.Join(*binDir, name), *force, *dryRun)
		}
	}

	if !*noConfig {
		removePath(configDest, *dryRun)
		removePath(bundlesDest, *dryRun)
		removeIfEmpty(filepath.Dir(configDest), *dryRun)
	}

	if !*noConfig {
		projectRoot := ""
		if *project {
			cwd, _ := os.Getwd()
			projectRoot = cwd
		}
		configPath := openCodeConfigPath(*opencodeHome, projectRoot)
		if err := removeOpenCodeMCP(configPath, *dryRun); err != nil {
			fmt.Printf("OpenCode MCP removal failed: %v\n", err)
			return 1
		}
		if err := removeClaudeMCP(*claudeHome, *dryRun); err != nil {
			fmt.Printf("Claude MCP removal failed: %v\n", err)
			return 1
		}
		if err := removeCodexMCP(*dryRun); err != nil {
			fmt.Printf("Codex MCP removal failed: %v\n", err)
			return 1
		}
	}

	fmt.Println("Done.")
	return 0
}

func uninstallHelp() string {
	return `conductor uninstall

Usage:
  conductor uninstall [--skills-only|--commands-only]
                      [--project] [--codex-home PATH] [--claude-home PATH] [--opencode-home PATH]
                      [--no-bins] [--bin-dir PATH] [--no-config]
                      [--force] [--dry-run]
`
}

func removePath(path string, dryRun bool) {
	if !pathExists(path) {
		return
	}
	if dryRun {
		fmt.Printf("Remove: %s\n", path)
		return
	}
	_ = os.RemoveAll(path)
}

func removeBin(path string, force, dryRun bool) {
	info, err := os.Lstat(path)
	if err != nil {
		return
	}
	if info.Mode()&os.ModeSymlink == 0 && !force {
		return
	}
	if dryRun {
		fmt.Printf("Remove: %s\n", path)
		return
	}
	_ = os.Remove(path)
}

func removeIfEmpty(dir string, dryRun bool) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	if len(entries) > 0 {
		return
	}
	if dryRun {
		fmt.Printf("Remove: %s\n", dir)
		return
	}
	_ = os.Remove(dir)
}
