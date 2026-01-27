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
	project := fs.Bool("project", false, "install into local project .claude/.codex/.opencode")
	codexHome := fs.String("codex-home", getenv("CODEX_HOME", filepath.Join(os.Getenv("HOME"), ".codex")), "codex home")
	claudeHome := fs.String("claude-home", filepath.Join(os.Getenv("HOME"), ".claude"), "claude home")
	opencodeHome := fs.String("opencode-home", defaultOpenCodeHome(), "opencode home")
	binDir := fs.String("bin-dir", getenv("CONDUCTOR_BIN", filepath.Join(os.Getenv("HOME"), ".local", "bin")), "bin dir")
	noBins := fs.Bool("no-bins", false, "skip bin install")
	noConfig := fs.Bool("no-config", false, "skip config install")
	repoRoot := fs.String("repo", "", "repo root (default: cwd)")
	force := fs.Bool("force", false, "overwrite existing")
	dryRun := fs.Bool("dry-run", false, "print actions only")
	interactive := fs.Bool("interactive", false, "prompt for CLI/MCP selection (default when TTY)")
	cliFlag := fs.String("cli", "", "comma-separated CLIs to install (codex,claude,opencode)")
	mcpFlag := fs.String("mcp", "", "register conductor MCP server")

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

	*codexHome = expandPath(*codexHome)
	*claudeHome = expandPath(*claudeHome)
	*opencodeHome = expandPath(*opencodeHome)
	*binDir = expandPath(*binDir)
	*repoRoot = expandPath(*repoRoot)

	root := *repoRoot
	if root == "" {
		root = detectRepoRoot()
	}

	skillsSource := filepath.Join(root, "skills", "conductor")
	commandsSource := filepath.Join(root, "commands")
	configSource := filepath.Join(root, "config", "conductor.json")
	bundlesSource := filepath.Join(root, "config", "mcp-bundles.json")
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

	allTargets := []struct {
		name          string
		home          string
		skillsDir     string
		commandsDir   string
		commandsLabel string
	}{
		{"codex", *codexHome, "skills", "prompts", "prompts"},
		{"claude", *claudeHome, "skills", "commands", "commands"},
		{"opencode", *opencodeHome, "skill", "command", "command"},
	}

	selectedCLIs := map[string]bool{"codex": true, "claude": true, "opencode": true}
	selectedMCPs := map[string]bool{"mcp": true}
	if *cliFlag != "" {
		selectedCLIs = parseCLIFlag(*cliFlag)
	}
	if *mcpFlag != "" {
		selectedMCPs = parseMCPFlag(*mcpFlag)
	}
	if *cliFlag == "" && *mcpFlag == "" && (*interactive || (!*dryRun && isTerminal(os.Stdout))) {
		selections := promptInstallSelectionsTUI()
		selectedCLIs = selections.clis
		selectedMCPs = selections.mcps
		if len(selectedCLIs) == 0 && len(selectedMCPs) == 0 {
			fmt.Println("Installation cancelled.")
			return 0
		}
	}

	var targets []struct {
		name          string
		home          string
		skillsDir     string
		commandsDir   string
		commandsLabel string
	}
	for _, t := range allTargets {
		if selectedCLIs[t.name] {
			targets = append(targets, t)
		}
	}

	if len(targets) == 0 {
		fmt.Println("No CLIs selected. Nothing to install.")
		return 0
	}

	for _, t := range targets {
		if installSkills {
			fmt.Printf("Install skills -> %s: %s\n", t.name, t.home)
			skillsDir := filepath.Join(t.home, t.skillsDir)
			ensureDir(skillsDir, *dryRun)
			dest := filepath.Join(skillsDir, "conductor")
			doLinkOrCopy(skillsSource, dest, *mode, *force, *dryRun)
		}
		if installCommands {
			commandsDir := filepath.Join(t.home, t.commandsDir)
			fmt.Printf("Install %s -> %s: %s\n", t.commandsLabel, t.name, t.home)
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
		for _, name := range conductorBinAliases() {
			dest := filepath.Join(*binDir, name)
			doLinkOrCopy(exe, dest, *mode, *force, *dryRun)
		}
	}

	if installConfig {
		ensureDir(filepath.Dir(configDest), *dryRun)
		// Only install config if it doesn't exist (preserve user settings)
		// Always use "copy" mode for config to prevent loss on brew upgrade
		// (symlinks break when Caskroom version folder is replaced)
		if !pathExists(configDest) || *force {
			fmt.Printf("Install config -> %s\n", configDest)
			doLinkOrCopy(configSource, configDest, "copy", *force, *dryRun)
		} else {
			fmt.Printf("Config exists, skipping: %s (use --force to overwrite)\n", configDest)
		}
		if pathExists(bundlesSource) && (!pathExists(bundlesDest) || *force) {
			doLinkOrCopy(bundlesSource, bundlesDest, "copy", *force, *dryRun)
		}
	}

	if installConfig {
		projectRoot := ""
		if *project {
			cwd, _ := os.Getwd()
			projectRoot = cwd
		}
		configPath := openCodeConfigPath(*opencodeHome, projectRoot)
		if selectedMCPs["mcp"] {
			if err := ensureOpenCodeMCP(configPath, *dryRun); err != nil {
				fmt.Printf("OpenCode MCP registration failed: %v\n", err)
				return 1
			}
			if err := ensureClaudeMCP(bundlesDest, *claudeHome, *dryRun); err != nil {
				fmt.Printf("Claude MCP registration failed: %v\n", err)
				return 1
			}
			if err := ensureCodexMCP(bundlesDest, *dryRun); err != nil {
				fmt.Printf("Codex MCP registration failed: %v\n", err)
				return 1
			}
		}
	}

	fmt.Println("Done.")
	return 0
}

func installHelp() string {
	return `conductor install

Usage:
  conductor install [--mode link|copy] [--skills-only|--commands-only]
                    [--project] [--codex-home PATH] [--claude-home PATH] [--opencode-home PATH]
                    [--force] [--dry-run]
                    [--no-bins] [--bin-dir PATH] [--no-config] [--repo PATH]
`
}

func ensureDir(dir string, dryRun bool) error {
	if dryRun {
		return nil
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		fmt.Printf("Warning: failed to create directory %s: %v\n", dir, err)
		return err
	}
	return nil
}

func doLinkOrCopy(src, dest, mode string, force, dryRun bool) error {
	if pathExists(dest) {
		if mode == "link" && !force {
			if linkMatches(dest, src) {
				fmt.Printf("Skip (linked): %s\n", dest)
				return nil
			}
		} else if !force {
			fmt.Printf("Skip (exists): %s\n", dest)
			return nil
		}
		if dryRun {
			fmt.Printf("Remove: %s\n", dest)
			return nil
		}
		if err := os.RemoveAll(dest); err != nil {
			fmt.Printf("Warning: failed to remove %s: %v\n", dest, err)
			return err
		}
	}
	if dryRun {
		fmt.Printf("%s: %s -> %s\n", strings.Title(mode), src, dest)
		return nil
	}
	if mode == "link" {
		if err := os.Symlink(src, dest); err != nil {
			fmt.Printf("Warning: failed to symlink %s -> %s: %v\n", src, dest, err)
			return err
		}
		return nil
	}
	info, err := os.Stat(src)
	if err != nil {
		fmt.Printf("Warning: failed to stat %s: %v\n", src, err)
		return err
	}
	if info.IsDir() {
		if err := copyDir(src, dest); err != nil {
			fmt.Printf("Warning: failed to copy directory %s -> %s: %v\n", src, dest, err)
			return err
		}
		return nil
	}
	if err := copyFile(src, dest); err != nil {
		fmt.Printf("Warning: failed to copy file %s -> %s: %v\n", src, dest, err)
		return err
	}
	return nil
}

func detectRepoRoot() string {
	candidates := []string{
		filepath.Join(os.Getenv("HOME"), ".conductor-kit"),
		"/opt/homebrew/Caskroom/conductor-kit",
		"/usr/local/Caskroom/conductor-kit",
	}

	for _, base := range candidates {
		if base == "" {
			continue
		}
		if strings.Contains(base, "Caskroom") {
			versions, _ := os.ReadDir(base)
			for i := len(versions) - 1; i >= 0; i-- {
				ver := versions[i]
				if ver.IsDir() {
					candidate := filepath.Join(base, ver.Name())
					if pathExists(filepath.Join(candidate, "skills", "conductor")) {
						return candidate
					}
				}
			}
		} else if pathExists(filepath.Join(base, "skills", "conductor")) {
			return base
		}
	}

	cwd, _ := os.Getwd()
	return cwd
}

func linkMatches(dest, src string) bool {
	info, err := os.Lstat(dest)
	if err != nil {
		return false
	}
	if info.Mode()&os.ModeSymlink == 0 {
		return false
	}
	target, err := os.Readlink(dest)
	if err != nil {
		return false
	}
	if !filepath.IsAbs(target) {
		target = filepath.Join(filepath.Dir(dest), target)
	}
	target = filepath.Clean(target)
	srcAbs, err := filepath.Abs(src)
	if err != nil {
		return false
	}
	return filepath.Clean(srcAbs) == target
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

func parseCLIFlag(flag string) map[string]bool {
	result := map[string]bool{}
	for _, cli := range strings.Split(flag, ",") {
		cli = strings.TrimSpace(strings.ToLower(cli))
		if cli == "codex" || cli == "claude" || cli == "opencode" {
			result[cli] = true
		}
	}
	return result
}

func parseMCPFlag(flag string) map[string]bool {
	result := map[string]bool{}
	for _, m := range strings.Split(flag, ",") {
		m = strings.TrimSpace(strings.ToLower(m))
		if m == "mcp" || m == "conductor" || m == "true" {
			result["mcp"] = true
		}
	}
	return result
}
