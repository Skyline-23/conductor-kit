package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func runInstall(args []string) int {
	fs := flag.NewFlagSet("install", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	mode := fs.String("mode", "link", "link|copy")
	skillsOnly := fs.Bool("skills-only", false, "install only skills")
	commandsOnly := fs.Bool("commands-only", false, "install only commands")
	codexHome := fs.String("codex-home", getenv("CODEX_HOME", filepath.Join(os.Getenv("HOME"), ".codex")), "codex home")
	claudeHome := fs.String("claude-home", filepath.Join(os.Getenv("HOME"), ".claude"), "claude home")
	binDir := fs.String("bin-dir", getenv("CONDUCTOR_BIN", filepath.Join(os.Getenv("HOME"), ".local", "bin")), "bin dir")
	noBins := fs.Bool("no-bins", false, "skip bin install")
	noConfig := fs.Bool("no-config", false, "skip config install")
	repoRoot := fs.String("repo", "", "repo root (default: cwd)")
	force := fs.Bool("force", false, "overwrite existing")
	dryRun := fs.Bool("dry-run", false, "print actions only")

	if err := fs.Parse(args); err != nil {
		fmt.Println(installHelp())
		return 1
	}

	if *mode != "link" && *mode != "copy" {
		fmt.Println("Invalid --mode. Use link or copy.")
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

	skillsSource := filepath.Join(root, "skills", "conductor")
	commandsSource := filepath.Join(root, "commands")
	configSource := filepath.Join(root, "config", "conductor.json")
	configDest := filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")

	installCommands := *commandsOnly || !*skillsOnly
	installSkills := !*commandsOnly
	installBins := !*noBins
	installConfig := !*noConfig

	if installSkills && !pathExists(skillsSource) {
		fmt.Printf("Missing skills source: %s\n", skillsSource)
		return 1
	}
	if installCommands && !pathExists(commandsSource) {
		fmt.Printf("Missing commands source: %s\n", commandsSource)
		return 1
	}
	if installConfig && !pathExists(configSource) {
		fmt.Printf("Missing config source: %s\n", configSource)
		return 1
	}

	targets := []struct {
		name string
		home string
	}{{"codex", *codexHome}, {"claude", *claudeHome}}

	for _, t := range targets {
		if installSkills {
			fmt.Printf("Install skills -> %s: %s\n", t.name, t.home)
			skillsDir := filepath.Join(t.home, "skills")
			ensureDir(skillsDir, *dryRun)
			dest := filepath.Join(skillsDir, "conductor")
			doLinkOrCopy(skillsSource, dest, *mode, *force, *dryRun)
		}
		if installCommands {
			fmt.Printf("Install commands -> %s: %s\n", t.name, t.home)
			commandsDir := filepath.Join(t.home, "commands")
			ensureDir(commandsDir, *dryRun)
			entries, _ := os.ReadDir(commandsSource)
			for _, entry := range entries {
				if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
					continue
				}
				src := filepath.Join(commandsSource, entry.Name())
				dest := filepath.Join(commandsDir, entry.Name())
				doLinkOrCopy(src, dest, *mode, *force, *dryRun)
			}
		}
	}

	if installBins {
		fmt.Printf("Install bins -> %s\n", *binDir)
		ensureDir(*binDir, *dryRun)
		exe, _ := os.Executable()
		aliases := []string{
			"conductor",
			"conductor-kit",
			"conductor-kit-install",
			"conductor-background-task",
			"conductor-background-output",
			"conductor-background-cancel",
			"conductor-background-batch",
			"conductor-mcp",
		}
		for _, name := range aliases {
			dest := filepath.Join(*binDir, name)
			doLinkOrCopy(exe, dest, *mode, *force, *dryRun)
		}
	}

	if installConfig {
		fmt.Printf("Install config -> %s\n", configDest)
		ensureDir(filepath.Dir(configDest), *dryRun)
		doLinkOrCopy(configSource, configDest, *mode, *force, *dryRun)
	}

	fmt.Println("Done.")
	return 0
}

func installHelp() string {
	return `conductor install

Usage:
  conductor install [--mode link|copy] [--skills-only|--commands-only]
                    [--codex-home PATH] [--claude-home PATH] [--force] [--dry-run]
                    [--no-bins] [--bin-dir PATH] [--no-config] [--repo PATH]
`
}

func ensureDir(dir string, dryRun bool) {
	if dryRun {
		return
	}
	_ = os.MkdirAll(dir, 0o755)
}

func doLinkOrCopy(src, dest, mode string, force, dryRun bool) {
	if pathExists(dest) {
		if !force {
			fmt.Printf("Skip (exists): %s\n", dest)
			return
		}
		if dryRun {
			fmt.Printf("Remove: %s\n", dest)
			return
		}
		_ = os.RemoveAll(dest)
	}
	if dryRun {
		fmt.Printf("%s: %s -> %s\n", strings.Title(mode), src, dest)
		return
	}
	if mode == "link" {
		_ = os.Symlink(src, dest)
		return
	}
	info, err := os.Stat(src)
	if err != nil {
		return
	}
	if info.IsDir() {
		_ = copyDir(src, dest)
		return
	}
	_ = copyFile(src, dest)
}

func copyFile(src, dest string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, in)
	if err != nil {
		return err
	}
	return out.Close()
}

func copyDir(src, dest string) error {
	entries, err := os.ReadDir(src)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(dest, 0o755); err != nil {
		return err
	}
	for _, entry := range entries {
		srcPath := filepath.Join(src, entry.Name())
		destPath := filepath.Join(dest, entry.Name())
		if entry.IsDir() {
			if err := copyDir(srcPath, destPath); err != nil {
				return err
			}
			continue
		}
		if err := copyFile(srcPath, destPath); err != nil {
			return err
		}
	}
	return nil
}
