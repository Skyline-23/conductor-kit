package main

import (
	"flag"
	"fmt"
	"io"
	"strings"
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

func validateConfig(cfg Config) []string {
	errors := []string{}
	if len(cfg.Roles) == 0 {
		errors = append(errors, "roles is empty")
	}
	if cfg.Defaults.MaxParallel < 0 {
		errors = append(errors, "defaults.max_parallel must be >= 0")
	}
	if cfg.Defaults.TimeoutMs < 0 {
		errors = append(errors, "defaults.timeout_ms must be >= 0")
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
		if _, err := normalizeRoleConfig(role); err != nil {
			errors = append(errors, fmt.Sprintf("roles.%s.%s", name, err.Error()))
		}
		if role.MaxParallel < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.max_parallel must be >= 0", name))
		}
		if role.TimeoutMs < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.timeout_ms must be >= 0", name))
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
		if role.ReadyTimeoutMs < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.ready_timeout_ms must be >= 0", name))
		}
	}
	return errors
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
