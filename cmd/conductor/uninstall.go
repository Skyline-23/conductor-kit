package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func runUninstall(args []string) int {
	fs := flag.NewFlagSet("uninstall", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	skillsOnly := fs.Bool("skills-only", false, "remove only skills")
	commandsOnly := fs.Bool("commands-only", false, "remove only commands/prompts")
	codexHome := fs.String("codex-home", getenv("CODEX_HOME", filepath.Join(os.Getenv("HOME"), ".codex")), "codex home")
	claudeHome := fs.String("claude-home", filepath.Join(os.Getenv("HOME"), ".claude"), "claude home")
	binDir := fs.String("bin-dir", getenv("CONDUCTOR_BIN", filepath.Join(os.Getenv("HOME"), ".local", "bin")), "bin dir")
	noBins := fs.Bool("no-bins", false, "skip bin removal")
	keepHome := fs.Bool("keep-home", false, "keep ~/.conductor-kit")
	repoRoot := fs.String("repo", "", "repo root (default: cwd)")
	dryRun := fs.Bool("dry-run", false, "print actions only")

	if err := fs.Parse(args); err != nil {
		fmt.Println(uninstallHelp())
		return 1
	}
	if *skillsOnly && *commandsOnly {
		fmt.Println("--skills-only and --commands-only are mutually exclusive.")
		return 1
	}

	root := *repoRoot
	if root == "" {
		cwd, _ := os.Getwd()
		root = cwd
	}

	commandsSource := filepath.Join(root, "commands")
	entries := commandEntries(commandsSource)

	removeCommands := *commandsOnly || !*skillsOnly
	removeSkills := !*commandsOnly
	removeBins := !*noBins

	targets := []struct {
		name string
		home string
	}{{"codex", *codexHome}, {"claude", *claudeHome}}

	for _, t := range targets {
		if removeSkills {
			skillsDir := filepath.Join(t.home, "skills")
			dest := filepath.Join(skillsDir, "conductor")
			removePath(dest, *dryRun)
		}
		if removeCommands {
			commandsDir := filepath.Join(t.home, "commands")
			if t.name == "codex" {
				commandsDir = filepath.Join(t.home, "prompts")
			}
			removeCommandFiles(commandsDir, entries, *dryRun)
		}
	}

	if removeBins {
		aliases := []string{
			"conductor",
			"conductor-kit",
			"conductor-kit-install",
			"conductor-config-validate",
			"conductor-doctor",
			"conductor-mcp",
		}
		for _, name := range aliases {
			removePath(filepath.Join(*binDir, name), *dryRun)
		}
	}

	if !*keepHome {
		conductorHome := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
		removePath(conductorHome, *dryRun)
	}

	fmt.Println("Done.")
	return 0
}

func uninstallHelp() string {
	return `conductor uninstall

Usage:
  conductor uninstall [--skills-only|--commands-only]
                      [--codex-home PATH] [--claude-home PATH] [--dry-run]
                      [--no-bins] [--bin-dir PATH] [--keep-home] [--repo PATH]
`
}

func commandEntries(commandsSource string) []string {
	entries := []string{}
	if !pathExists(commandsSource) {
		return entries
	}
	dirEntries, err := os.ReadDir(commandsSource)
	if err != nil {
		return entries
	}
	for _, entry := range dirEntries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}
		entries = append(entries, entry.Name())
	}
	return entries
}

func removeCommandFiles(dir string, entries []string, dryRun bool) {
	if len(entries) == 0 {
		prefix := "conductor-"
		dirEntries, err := os.ReadDir(dir)
		if err != nil {
			return
		}
		for _, entry := range dirEntries {
			if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
				continue
			}
			if strings.HasPrefix(entry.Name(), prefix) {
				removePath(filepath.Join(dir, entry.Name()), dryRun)
			}
		}
		return
	}
	for _, name := range entries {
		removePath(filepath.Join(dir, name), dryRun)
	}
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
