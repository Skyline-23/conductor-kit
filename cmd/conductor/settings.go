package main

import (
	"bufio"
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"golang.org/x/term"
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
	listModels := fs.Bool("list-models", false, "list models for a role or cli")
	pickModel := fs.Bool("pick-model", false, "pick model interactively")
	deleteRole := fs.Bool("delete-role", false, "delete a role")
	interactive := fs.Bool("interactive", false, "run interactive wizard")
	noTui := fs.Bool("no-tui", false, "disable TUI")
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

	interactiveRequested := *interactive || (!*list && *role == "" && *cli == "" && *model == "" && *reasoning == "" && isTerminal(os.Stdin))
	if *list && *listModels {
		fmt.Println("Choose either --list or --list-models.")
		return 1
	}

	if *listModels {
		targetCLI := *cli
		if targetCLI == "" && *role != "" {
			if roleCfg, ok := cfg.Roles[*role]; ok {
				targetCLI = roleCfg.CLI
			}
		}
		if targetCLI == "" {
			fmt.Println("Missing --cli or --role for --list-models.")
			return 1
		}
		models, source := listModelsForCLI(targetCLI, true)
		if len(models) == 0 {
			fmt.Printf("No models found for %s.\n", targetCLI)
			return 1
		}
		fmt.Printf("Models for %s (%s):\n", targetCLI, source)
		for _, item := range models {
			fmt.Println("-", item)
		}
		return 0
	}

	if *deleteRole {
		if *role == "" {
			fmt.Println("Missing --role for --delete-role.")
			return 1
		}
		if len(cfg.Roles) == 0 {
			fmt.Println("No roles to delete.")
			return 1
		}
		if _, ok := cfg.Roles[*role]; !ok {
			fmt.Printf("Role %s not found.\n", *role)
			return 1
		}
		delete(cfg.Roles, *role)
		if err := writeConfig(path, cfg); err != nil {
			fmt.Println("Config write error:", err.Error())
			return 1
		}
		fmt.Printf("Deleted role %s in %s\n", *role, path)
		return 0
	}

	if *list || (!interactiveRequested && *role == "" && *cli == "" && *model == "" && *reasoning == "" && !*pickModel) {
		printRolesSummary(cfg, path)
		return 0
	}

	if interactiveRequested {
		if *noTui {
			return runSettingsInteractive(cfg, path)
		}
		return runSettingsBubbleTea(cfg, path)
	}

	if *pickModel {
		if !isTerminal(os.Stdin) {
			fmt.Println("--pick-model requires an interactive terminal.")
			return 1
		}
		if *role == "" {
			fmt.Println("Missing --role for --pick-model.")
			return 1
		}
		if *noTui {
			return runSettingsPickModel(cfg, path, *role, *cli, false)
		}
		return runSettingsPickModel(cfg, path, *role, *cli, true)
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
  conductor settings --list-models --cli <cli>
  conductor settings --list-models --role <role>
  conductor settings --pick-model --role <role> [--cli <cli>]
  conductor settings --delete-role --role <role>
  conductor settings --interactive
  conductor settings --no-tui --interactive
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

func runSettingsInteractive(cfg Config, path string) int {
	reader := bufio.NewReader(os.Stdin)
	roleName, ok := promptRole(reader, cfg)
	if !ok {
		return 1
	}

	if cfg.Roles == nil {
		cfg.Roles = map[string]RoleConfig{}
	}
	roleCfg := cfg.Roles[roleName]

	cli := promptChoice(reader, "CLI", []string{"codex", "claude", "gemini"}, roleCfg.CLI, true)
	if cli != "" {
		roleCfg.CLI = cli
	}
	if roleCfg.CLI == "" {
		fmt.Println("CLI is required.")
		return 1
	}

	model := promptModel(reader, roleCfg.CLI, roleCfg.Model)
	if model != "" {
		roleCfg.Model = model
	}

	if roleCfg.CLI == "codex" {
		roleCfg.Reasoning = promptChoice(reader, "Reasoning effort (blank to keep)", []string{"low", "medium", "high", "xhigh"}, roleCfg.Reasoning, true)
	}

	cfg.Roles[roleName] = roleCfg
	if err := writeConfig(path, cfg); err != nil {
		fmt.Println("Config write error:", err.Error())
		return 1
	}
	clearScreen()
	fmt.Printf("Updated %s\n", path)
	return 0
}

func runSettingsPickModel(cfg Config, path, roleName, cliOverride string, useTui bool) int {
	reader := bufio.NewReader(os.Stdin)
	if cfg.Roles == nil {
		cfg.Roles = map[string]RoleConfig{}
	}
	roleCfg := cfg.Roles[roleName]
	if cliOverride != "" {
		roleCfg.CLI = cliOverride
	}
	if roleCfg.CLI == "" {
		fmt.Println("Missing cli for role; provide --cli.")
		return 1
	}
	var model string
	if useTui && isTerminal(os.Stdin) && isTerminal(os.Stdout) {
		selection, ok := tuiSelectModel(roleCfg.CLI, roleCfg.Model, false)
		if !ok {
			fmt.Println("No model selected.")
			return 1
		}
		if selection == tuiManualValue {
			model = promptLine(reader, "Model", roleCfg.Model)
		} else {
			model = selection
		}
	} else {
		model = promptModel(reader, roleCfg.CLI, roleCfg.Model)
	}
	if model == "" {
		fmt.Println("No model selected.")
		return 1
	}
	roleCfg.Model = model
	cfg.Roles[roleName] = roleCfg
	if err := writeConfig(path, cfg); err != nil {
		fmt.Println("Config write error:", err.Error())
		return 1
	}
	clearScreen()
	fmt.Printf("Updated %s\n", path)
	return 0
}

const (
	tuiManualValue = "__manual__"
	tuiNewRole     = "__new__"
	tuiDeleteRole  = "__delete__"
	tuiCancelValue = "__cancel__"
	tuiKeepValue   = "__keep__"
)

type tuiOption struct {
	Label string
	Value string
}

func runSettingsTUI(cfg Config, path string) int {
	if !isTerminal(os.Stdin) || !isTerminal(os.Stdout) {
		return runSettingsInteractive(cfg, path)
	}
	reader := bufio.NewReader(os.Stdin)
	if cfg.Roles == nil {
		cfg.Roles = map[string]RoleConfig{}
	}

	type step int
	const (
		stepRole step = iota
		stepCLI
		stepModel
		stepReasoning
		stepSave
	)

	currentStep := stepRole
	var roleName string
	var roleCfg RoleConfig

	for {
		switch currentStep {
		case stepRole:
			selected, ok := tuiSelectRole(cfg)
			if !ok {
				clearScreen()
				return 0
			}
			roleName = selected
			if roleName == tuiNewRole {
				roleName = promptLine(reader, "Role name", "")
				if roleName == "" {
					continue
				}
			}
			roleCfg = cfg.Roles[roleName]
			currentStep = stepCLI
		case stepCLI:
			cliChoice, ok := tuiSelectOptions("Select CLI", tuiGetCLIOptionsWithAuth(), roleCfg.CLI)
			if !ok {
				currentStep = stepRole
				continue
			}
			if cliChoice == tuiManualValue {
				cliChoice = promptLine(reader, "CLI", roleCfg.CLI)
			}
			if cliChoice != "" {
				roleCfg.CLI = cliChoice
			}
			if roleCfg.CLI == "" {
				continue
			}
			currentStep = stepModel
		case stepModel:
			modelChoice, ok := tuiSelectModel(roleCfg.CLI, roleCfg.Model, false)
			if !ok {
				currentStep = stepCLI
				continue
			}
			if modelChoice == tuiManualValue {
				modelChoice = promptLine(reader, "Model", roleCfg.Model)
			}
			if modelChoice != "" {
				roleCfg.Model = modelChoice
			}
			if roleCfg.CLI == "codex" {
				currentStep = stepReasoning
			} else {
				currentStep = stepSave
			}
		case stepReasoning:
			reasoningChoice, ok := tuiSelectOptions("Reasoning effort", []tuiOption{
				{Label: "Keep current", Value: tuiKeepValue},
				{Label: "low", Value: "low"},
				{Label: "medium", Value: "medium"},
				{Label: "high", Value: "high"},
				{Label: "xhigh", Value: "xhigh"},
				{Label: "Manual entry", Value: tuiManualValue},
			}, roleCfg.Reasoning)
			if !ok {
				currentStep = stepModel
				continue
			}
			switch reasoningChoice {
			case tuiKeepValue:
			case tuiManualValue:
				roleCfg.Reasoning = promptLine(reader, "Reasoning", roleCfg.Reasoning)
			default:
				if reasoningChoice != "" {
					roleCfg.Reasoning = reasoningChoice
				}
			}
			currentStep = stepSave
		case stepSave:
			cfg.Roles[roleName] = roleCfg
			if err := writeConfig(path, cfg); err != nil {
				clearScreen()
				fmt.Println("Config write error:", err.Error())
				return 1
			}
			currentStep = stepRole
		}
	}
}

func tuiSelectRole(cfg Config) (string, bool) {
	roles := make([]string, 0, len(cfg.Roles))
	for name := range cfg.Roles {
		roles = append(roles, name)
	}
	sort.Strings(roles)
	options := make([]tuiOption, 0, len(roles)+2)
	for _, name := range roles {
		roleCfg := cfg.Roles[name]
		label := name
		if roleCfg.CLI != "" {
			info := roleCfg.CLI
			if roleCfg.Model != "" {
				info += "/" + roleCfg.Model
			}
			label = fmt.Sprintf("%-20s \x1b[38;5;242m%s\x1b[0m", name, info)
		}
		options = append(options, tuiOption{Label: label, Value: name})
	}
	options = append(options, tuiOption{Label: "\x1b[38;5;86m+ New role\x1b[0m", Value: tuiNewRole})
	return tuiSelectOptions("Select role to edit", options, "")
}

// tuiGetCLIOptionsWithAuth returns CLI options with auth status indicators
func tuiGetCLIOptionsWithAuth() []tuiOption {
	clis := []struct {
		name      string
		checkAuth func() (bool, string)
	}{
		{"codex", mcpCheckCodexAuth},
		{"claude", mcpCheckClaudeAuth},
		{"gemini", mcpCheckGeminiAuth},
	}

	// ANSI color codes
	const (
		green = "\x1b[38;5;78m"
		gray  = "\x1b[38;5;242m"
		reset = "\x1b[0m"
	)

	options := make([]tuiOption, 0, len(clis)+1)
	for _, cli := range clis {
		label := cli.name
		if isCommandAvailable(cli.name) {
			auth, status := cli.checkAuth()
			if auth {
				// Truncate and format status
				if len(status) > 35 {
					status = status[:32] + "..."
				}
				label = fmt.Sprintf("%-8s %s● %s%s", cli.name, green, status, reset)
			} else {
				label = fmt.Sprintf("%-8s %s○ not authenticated%s", cli.name, gray, reset)
			}
		} else {
			label = fmt.Sprintf("%-8s %s○ not installed%s", cli.name, gray, reset)
		}
		options = append(options, tuiOption{Label: label, Value: cli.name})
	}
	options = append(options, tuiOption{Label: gray + "Manual entry" + reset, Value: tuiManualValue})
	return options
}

func tuiSelectModel(cli, current string, allowCLI bool) (string, bool) {
	models, source := listModelsForCLI(cli, allowCLI)
	options := make([]tuiOption, 0, len(models)+1)
	for _, model := range models {
		options = append(options, tuiOption{Label: model, Value: model})
	}
	options = append(options, tuiOption{Label: "Manual entry", Value: tuiManualValue})
	title := fmt.Sprintf("Select model (%s)", source)
	return tuiSelectOptions(title, options, current)
}

func tuiSelectOptions(title string, options []tuiOption, current string) (string, bool) {
	if len(options) == 0 {
		return "", false
	}
	fd := int(os.Stdin.Fd())
	state, err := term.MakeRaw(fd)
	if err != nil {
		return "", false
	}
	defer term.Restore(fd, state)

	hideCursor()
	defer showCursor()

	index := 0
	if current != "" {
		for i, opt := range options {
			if opt.Value == current {
				index = i
				break
			}
		}
	}

	reader := bufio.NewReader(os.Stdin)
	// ANSI color codes for cyan theme
	const (
		cyan     = "\x1b[38;5;86m"
		cyanBold = "\x1b[1;38;5;86m"
		gray     = "\x1b[38;5;242m"
		reset    = "\x1b[0m"
	)
	render := func() {
		lines := make([]string, 0, len(options)+4)
		lines = append(lines, cyanBold+title+reset)
		lines = append(lines, "")
		lines = append(lines, gray+"↑/↓ navigate  enter select  esc cancel"+reset)
		lines = append(lines, "")
		for i, opt := range options {
			line := opt.Label
			if i == index {
				line = fmt.Sprintf("%s▸ %s%s", cyan, line, reset)
			} else {
				line = fmt.Sprintf("  %s", line)
			}
			lines = append(lines, line)
		}
		tuiRenderLines(lines)
	}

	render()
	for {
		b, err := reader.ReadByte()
		if err != nil {
			return "", false
		}
		switch b {
		case '\r', '\n':
			return options[index].Value, true
		case 0x1b:
			next, err := reader.ReadByte()
			if err != nil {
				return "", false
			}
			if next != '[' {
				return "", false
			}
			key, err := reader.ReadByte()
			if err != nil {
				return "", false
			}
			switch key {
			case 'A':
				if index > 0 {
					index--
				}
			case 'B':
				if index < len(options)-1 {
					index++
				}
			}
		}
		render()
	}
}

func clearScreen() {
	fmt.Print("\x1b[2J\x1b[H")
}

func hideCursor() {
	fmt.Print("\x1b[?25l")
}

func showCursor() {
	fmt.Print("\x1b[?25h")
}

func tuiRenderLines(lines []string) {
	clearScreen()
	for i, line := range lines {
		fmt.Printf("\x1b[%d;1H\x1b[2K%s", i+1, line)
	}
}

func promptRole(reader *bufio.Reader, cfg Config) (string, bool) {
	roles := make([]string, 0, len(cfg.Roles))
	for name := range cfg.Roles {
		roles = append(roles, name)
	}
	sort.Strings(roles)
	fmt.Println("Select role:")
	for i, name := range roles {
		fmt.Printf("  %d) %s\n", i+1, name)
	}
	fmt.Println("  n) new role")
	fmt.Print("> ")
	line, _ := reader.ReadString('\n')
	line = strings.TrimSpace(line)
	if line == "" && len(roles) > 0 {
		return roles[0], true
	}
	if line == "n" || line == "N" {
		fmt.Print("Role name: ")
		name, _ := reader.ReadString('\n')
		name = strings.TrimSpace(name)
		return name, name != ""
	}
	if idx, err := strconv.Atoi(line); err == nil {
		if idx >= 1 && idx <= len(roles) {
			return roles[idx-1], true
		}
	}
	for _, name := range roles {
		if line == name {
			return name, true
		}
	}
	return "", false
}

func promptChoice(reader *bufio.Reader, label string, options []string, current string, allowCustom bool) string {
	if current != "" {
		fmt.Printf("%s (current: %s)\n", label, current)
	} else {
		fmt.Printf("%s\n", label)
	}
	for i, opt := range options {
		fmt.Printf("  %d) %s\n", i+1, opt)
	}
	if allowCustom {
		fmt.Println("  m) manual entry")
	}
	fmt.Print("> ")
	line, _ := reader.ReadString('\n')
	line = strings.TrimSpace(line)
	if line == "" {
		return current
	}
	if allowCustom && (line == "m" || line == "M") {
		return promptLine(reader, label, current)
	}
	if idx, err := strconv.Atoi(line); err == nil {
		if idx >= 1 && idx <= len(options) {
			return options[idx-1]
		}
	}
	for _, opt := range options {
		if line == opt {
			return opt
		}
	}
	if allowCustom {
		return line
	}
	return current
}

func promptLine(reader *bufio.Reader, label, current string) string {
	if current != "" {
		fmt.Printf("%s [%s]: ", label, current)
	} else {
		fmt.Printf("%s: ", label)
	}
	line, _ := reader.ReadString('\n')
	line = strings.TrimSpace(line)
	if line == "" {
		return current
	}
	return line
}

func promptModel(reader *bufio.Reader, cli, current string) string {
	models, source := listModelsForCLI(cli, true)
	if current != "" && !contains(models, current) {
		models = append([]string{current}, models...)
	}
	if len(models) == 0 {
		return promptLine(reader, "Model", current)
	}
	fmt.Printf("Models (%s)\n", source)
	return promptChoice(reader, "Select model", models, current, true)
}

func listModelsForCLI(cli string, allowCLI bool) ([]string, string) {
	if allowCLI && isCommandAvailable(cli) {
		if models := listModelsFromCLI(cli); len(models) > 0 {
			return models, "cli"
		}
	}
	if models := builtInModelList(cli); len(models) > 0 {
		return models, "defaults"
	}
	return nil, "none"
}

func listModelsFromCLI(cli string) []string {
	var candidates [][]string
	switch cli {
	case "codex":
		candidates = [][]string{
			{"models", "--json"},
			{"models"},
			{"model", "list"},
		}
	case "claude":
		candidates = [][]string{
			{"models", "--json"},
			{"models"},
		}
	case "gemini":
		candidates = [][]string{
			{"models", "--json"},
			{"models"},
			{"--list-models"},
		}
	default:
		return nil
	}

	for _, args := range candidates {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		cmd := exec.CommandContext(ctx, cli, args...)
		out, err := cmd.Output()
		cancel()
		if err != nil {
			continue
		}
		if models := parseModelOutput(string(out)); len(models) > 0 {
			return models
		}
	}
	return nil
}

func builtInModelList(cli string) []string {
	switch cli {
	case "codex":
		return []string{
			"gpt-5.2-codex",
			"gpt-5.1-codex-max",
			"gpt-5.1-codex-mini",
		}
	case "claude":
		return []string{
			"claude-3-5-sonnet-20241022",
			"claude-3-5-haiku-20241022",
			"claude-3-opus-20240229",
		}
	case "gemini":
		return []string{
			"gemini-3-pro-preview",
			"gemini-3-flash-preview",
			"gemini-3-flash",
			"gemini-2.5-pro",
			"gemini-2.5-flash",
		}
	default:
		return nil
	}
}

func parseModelOutput(output string) []string {
	text := strings.TrimSpace(output)
	if text == "" {
		return nil
	}
	var arr []string
	if err := json.Unmarshal([]byte(text), &arr); err == nil {
		return uniqueStrings(arr)
	}
	var obj map[string]interface{}
	if err := json.Unmarshal([]byte(text), &obj); err == nil {
		candidates := []interface{}{}
		if val, ok := obj["models"]; ok {
			candidates, _ = val.([]interface{})
		} else if val, ok := obj["data"]; ok {
			candidates, _ = val.([]interface{})
		}
		out := []string{}
		for _, item := range candidates {
			switch v := item.(type) {
			case string:
				out = append(out, v)
			case map[string]interface{}:
				if id, ok := v["id"].(string); ok {
					out = append(out, id)
				} else if name, ok := v["name"].(string); ok {
					out = append(out, name)
				}
			}
		}
		return uniqueStrings(out)
	}
	lines := strings.Split(text, "\n")
	out := []string{}
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) == 0 {
			continue
		}
		token := strings.Trim(fields[0], ",;:")
		if looksLikeModel(token) {
			out = append(out, token)
		}
	}
	return uniqueStrings(out)
}

func looksLikeModel(token string) bool {
	if len(token) < 4 {
		return false
	}
	hasDigit := false
	hasDash := false
	for _, r := range token {
		switch {
		case r >= '0' && r <= '9':
			hasDigit = true
		case r == '-':
			hasDash = true
		case (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || r == '.' || r == '_' || r == ':':
			continue
		default:
			return false
		}
	}
	return hasDigit && hasDash
}

func uniqueStrings(in []string) []string {
	seen := map[string]struct{}{}
	out := []string{}
	for _, item := range in {
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		if _, ok := seen[item]; ok {
			continue
		}
		seen[item] = struct{}{}
		out = append(out, item)
	}
	return out
}

func contains(items []string, target string) bool {
	for _, item := range items {
		if item == target {
			return true
		}
	}
	return false
}
