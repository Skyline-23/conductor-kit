package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

func runConfigValidate(args []string) int {
	fs := flag.NewFlagSet("config-validate", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	configPath := fs.String("config", getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")), "config path")
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
	configPath := fs.String("config", getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")), "config path")
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
	if cfg.Defaults.Retry < 0 {
		errors = append(errors, "defaults.retry must be >= 0")
	}
	if cfg.Defaults.RetryBackoffMs < 0 {
		errors = append(errors, "defaults.retry_backoff_ms must be >= 0")
	}
	for name, role := range cfg.Roles {
		if role.CLI == "" {
			errors = append(errors, fmt.Sprintf("roles.%s.cli is empty", name))
		}
		if len(role.Args) == 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.args is empty", name))
		}
		if role.MaxParallel < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.max_parallel must be >= 0", name))
		}
		if role.TimeoutMs < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.timeout_ms must be >= 0", name))
		}
		if role.Retry < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.retry must be >= 0", name))
		}
		if role.RetryBackoffMs < 0 {
			errors = append(errors, fmt.Sprintf("roles.%s.retry_backoff_ms must be >= 0", name))
		}
	}
	return errors
}
