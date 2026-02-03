package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"sort"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

func runConfigValidate(args []string) int {
	fs := flag.NewFlagSet("config-validate", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	configPath := fs.String("config", resolveConfigPath(""), "config path")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}

	cfg, err := loadConfig(*configPath)
	if err != nil {
		fmt.Println("Config error:", err.Error())
		return 1
	}
	errors := validateConfig(cfg)
	if len(errors) > 0 {
		for _, msg := range errors {
			fmt.Println("Error:", msg)
		}
		return 1
	}
	fmt.Println("OK")
	return 0
}

func runDoctor(args []string) int {
	fs := flag.NewFlagSet("doctor", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	configPath := fs.String("config", resolveConfigPath(""), "config path")
	jsonOut := fs.Bool("json", false, "output JSON")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}

	cfg, err := loadConfig(*configPath)
	if err != nil {
		fmt.Println("Config error:", err.Error())
		return 1
	}
	errors := validateConfig(cfg)

	if *jsonOut || !isTerminal(os.Stdout) {
		return runDoctorPlain(cfg, errors)
	}

	return runDoctorPretty(cfg, *configPath, errors)
}

func runDoctorPlain(cfg Config, errors []string) int {
	if len(errors) > 0 {
		for _, msg := range errors {
			fmt.Println("Error:", msg)
		}
		return 1
	}

	fmt.Println("Config: OK")
	missing := false
	for name, role := range cfg.Roles {
		if role.CLI == "" {
			fmt.Printf("Role %s: missing cli\n", name)
			missing = true
			continue
		}
		if isCommandAvailable(role.CLI) {
			fmt.Printf("Role %s: %s (ok)\n", name, role.CLI)
		} else {
			fmt.Printf("Role %s: %s (missing)\n", name, role.CLI)
			missing = true
		}
		modelIssues := append([]ModelEntry{}, role.Models...)
		if role.Model != "" {
			modelIssues = append(modelIssues, ModelEntry{Name: role.Model, ReasoningEffort: role.Reasoning})
		}
		for _, entry := range modelIssues {
			if entry.Name == "" {
				continue
			}
			check := checkModelForCLI(role.CLI, entry.Name)
			switch check.level {
			case "ok":
				fmt.Printf("Role %s: model %s (ok)\n", name, entry.Name)
			case "warn":
				fmt.Printf("Role %s: model %s (%s)\n", name, entry.Name, check.message)
			case "error":
				fmt.Printf("Role %s: model %s (%s)\n", name, entry.Name, check.message)
				missing = true
			}
		}
	}
	if missing {
		return 1
	}
	return 0
}

func runDoctorPretty(cfg Config, configPath string, errors []string) int {
	var sb strings.Builder

	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("99")).Render("🩺 Conductor Doctor")
	sb.WriteString(title + "\n\n")

	sb.WriteString(labelStyle.Render("Config: ") + pathStyle.Render(configPath) + "\n")
	sb.WriteString(renderDivider(50) + "\n\n")

	if len(errors) > 0 {
		sb.WriteString(sectionStyle.Render("Configuration Errors") + "\n")
		for _, msg := range errors {
			sb.WriteString("  " + iconError + " " + statusErrorStyle.Render(msg) + "\n")
		}
		sb.WriteString("\n")
		fmt.Print(sb.String())
		return 1
	}

	sb.WriteString(iconOK + " " + statusOKStyle.Render("Config validated") + "\n\n")

	sb.WriteString(sectionStyle.Render("Role Diagnostics") + "\n\n")

	roleNames := make([]string, 0, len(cfg.Roles))
	for name := range cfg.Roles {
		roleNames = append(roleNames, name)
	}
	sort.Strings(roleNames)

	hasIssues := false
	for _, name := range roleNames {
		role := cfg.Roles[name]

		roleHeader := roleNameStyle.Render(name)
		sb.WriteString("  " + roleHeader + "\n")

		if role.CLI == "" {
			sb.WriteString("    " + iconError + " " + labelStyle.Render("cli: ") + statusErrorStyle.Render("missing") + "\n")
			hasIssues = true
			continue
		}

		cliAvailable := isCommandAvailable(role.CLI)
		if cliAvailable {
			sb.WriteString("    " + iconOK + " " + labelStyle.Render("cli: ") + valueStyle.Render(role.CLI) + "\n")
		} else {
			sb.WriteString("    " + iconError + " " + labelStyle.Render("cli: ") + valueStyle.Render(role.CLI) + " " + statusErrorStyle.Render("(not found)") + "\n")
			hasIssues = true
		}

		modelEntries := append([]ModelEntry{}, role.Models...)
		if role.Model != "" {
			modelEntries = append(modelEntries, ModelEntry{Name: role.Model, ReasoningEffort: role.Reasoning})
		}

		for _, entry := range modelEntries {
			if entry.Name == "" {
				continue
			}
			check := checkModelForCLI(role.CLI, entry.Name)
			switch check.level {
			case "ok":
				sb.WriteString("    " + iconOK + " " + labelStyle.Render("model: ") + valueStyle.Render(entry.Name) + "\n")
			case "warn":
				sb.WriteString("    " + iconWarn + " " + labelStyle.Render("model: ") + valueStyle.Render(entry.Name) + " " + statusWarnStyle.Render("("+check.message+")") + "\n")
			case "error":
				sb.WriteString("    " + iconError + " " + labelStyle.Render("model: ") + valueStyle.Render(entry.Name) + " " + statusErrorStyle.Render("("+check.message+")") + "\n")
				hasIssues = true
			}
		}
		sb.WriteString("\n")
	}

	if hasIssues {
		summary := statusWarnStyle.Render("Some issues need attention")
		sb.WriteString(summary + "\n")
		fmt.Print(sb.String())
		return 1
	}

	summary := statusOKStyle.Render("✓ All checks passed")
	sb.WriteString(summary + "\n")
	fmt.Print(sb.String())
	return 0
}

func validateConfig(cfg Config) []string {
	errors := []string{}
	if len(cfg.Roles) == 0 {
		errors = append(errors, "roles is empty")
	}
	if cfg.Runtime.MaxParallel < 0 {
		errors = append(errors, "runtime.max_parallel must be >= 0")
	}
	switch cfg.Runtime.Queue.OnModeChange {
	case "", "none", "cancel_pending", "cancel_running":
		// ok
	default:
		errors = append(errors, "runtime.queue.on_mode_change must be one of: none, cancel_pending, cancel_running")
	}
	for _, role := range cfg.Runtime.Approval.Roles {
		if role == "" {
			continue
		}
		if _, ok := cfg.Roles[role]; !ok {
			errors = append(errors, fmt.Sprintf("runtime.approval.roles unknown: %s", role))
		}
	}
	for _, agent := range cfg.Runtime.Approval.Agents {
		if agent == "" {
			continue
		}
		switch agent {
		case "codex", "claude", "gemini":
			// ok
		default:
			errors = append(errors, fmt.Sprintf("runtime.approval.agents unknown: %s", agent))
		}
	}
	if cfg.Defaults.MaxParallel < 0 {
		errors = append(errors, "defaults.max_parallel must be >= 0")
	}
	if cfg.Defaults.IdleTimeoutMs < 0 {
		errors = append(errors, "defaults.idle_timeout_ms must be >= 0")
	}
	if cfg.Defaults.Retry < 0 {
		errors = append(errors, "defaults.retry must be >= 0")
	}
	if cfg.Defaults.RetryBackoffMs < 0 {
		errors = append(errors, "defaults.retry_backoff_ms must be >= 0")
	}
	for name, role := range cfg.Roles {
		normalized, err := normalizeRoleConfig(role)
		if err != nil {
			errors = append(errors, fmt.Sprintf("roles.%s.%s", name, err.Error()))
		}
		if normalized.CLI == "claude" {
			if !hasArgExact(normalized.Args, "-p") && !hasArgExact(normalized.Args, "--print") {
				errors = append(errors, fmt.Sprintf("roles.%s.args must include -p or --print", name))
			}
			if idx := indexOf(normalized.Args, "-p"); idx >= 0 {
				if idx+1 >= len(normalized.Args) || normalized.Args[idx+1] != "{prompt}" {
					errors = append(errors, fmt.Sprintf("roles.%s.args must place {prompt} immediately after -p", name))
				}
			}
			if idx := indexOf(normalized.Args, "--print"); idx >= 0 {
				if idx+1 >= len(normalized.Args) || normalized.Args[idx+1] != "{prompt}" {
					errors = append(errors, fmt.Sprintf("roles.%s.args must place {prompt} immediately after --print", name))
				}
			}
		}
		if normalized.CLI == "gemini" {
			if idx := indexOf(normalized.Args, "-p"); idx >= 0 {
				if idx+1 >= len(normalized.Args) || normalized.Args[idx+1] != "{prompt}" {
					errors = append(errors, fmt.Sprintf("roles.%s.args must place {prompt} immediately after -p", name))
				}
			}
		}
		if normalized.CLI == "claude" || normalized.CLI == "gemini" {
			if hasArgExact(normalized.Args, "--output-format") {
				val, ok := readFlagValue(normalized.Args, "--output-format")
				if !ok || strings.TrimSpace(val) == "" {
					errors = append(errors, fmt.Sprintf("roles.%s.args missing value for --output-format", name))
				} else if val != "stream-json" {
					errors = append(errors, fmt.Sprintf("roles.%s.args --output-format must be stream-json", name))
				}
			}
		}
		if normalized.CLI == "codex" {
			if val, ok := readFlagValue(normalized.Args, "--approval-policy"); ok {
				if !isAllowedApprovalPolicy(val) {
					errors = append(errors, fmt.Sprintf("roles.%s.args invalid --approval-policy: %s", name, val))
				}
			}
			if val, ok := readFlagValue(normalized.Args, "--sandbox"); ok {
				if !isAllowedSandbox(val) {
					errors = append(errors, fmt.Sprintf("roles.%s.args invalid --sandbox: %s", name, val))
				}
			}
		}
		if role.MaxParallel < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.max_parallel must be >= 0", name))
		}
		if role.IdleTimeoutMs < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.idle_timeout_ms must be >= 0", name))
		}
		if role.Retry < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.retry must be >= 0", name))
		}
		if role.RetryBackoffMs < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.retry_backoff_ms must be >= 0", name))
		}
		for key := range role.Env {
			if strings.TrimSpace(key) == "" {
				errors = append(errors, fmt.Sprintf("roles.%s.env contains empty key", name))
				break
			}
			if strings.Contains(key, "=") {
				errors = append(errors, fmt.Sprintf("roles.%s.env contains invalid key: %s", name, key))
				break
			}
		}
	}
	return errors
}

func isAllowedApprovalPolicy(val string) bool {
	switch val {
	case "untrusted", "on-request", "on-failure", "never":
		return true
	default:
		return false
	}
}

func isAllowedSandbox(val string) bool {
	switch val {
	case "read-only", "workspace-write", "danger-full-access":
		return true
	default:
		return false
	}
}

func hasArgExact(args []string, target string) bool {
	for _, arg := range args {
		if arg == target {
			return true
		}
	}
	return false
}

type modelCheck struct {
	level   string
	message string
}

func checkModelForCLI(cli, model string) modelCheck {
	if model == "" {
		return modelCheck{level: "ok", message: "uses cli default"}
	}
	if strings.Contains(model, "/") {
		return modelCheck{
			level:   "error",
			message: "invalid (use CLI-native model names without provider prefix)",
		}
	}
	switch cli {
	case "codex":
		if strings.HasPrefix(model, "gpt-") {
			return modelCheck{level: "ok", message: "ok"}
		}
		return modelCheck{level: "warn", message: "unexpected for codex (expected gpt-*)"}
	case "claude":
		if strings.HasPrefix(model, "claude-") {
			return modelCheck{level: "ok", message: "ok"}
		}
		return modelCheck{level: "warn", message: "unexpected for claude (expected claude-*)"}
	case "gemini":
		if strings.HasPrefix(model, "gemini-") || model == "auto" {
			return modelCheck{level: "ok", message: "ok"}
		}
		return modelCheck{level: "warn", message: "unexpected for gemini (expected gemini-*)"}
	default:
		return modelCheck{level: "warn", message: "unknown cli (skipping model validation)"}
	}
}
