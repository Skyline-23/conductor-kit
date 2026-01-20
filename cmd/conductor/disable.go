package main

import (
	"flag"
	"fmt"
	"io"
	"os"
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
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}

	cfg, err := loadConfig(*configPath)
	if err != nil {
		fmt.Println("Config error:", err.Error())
		return 1
	}

	if cfg.Disabled != disabled {
		cfg.Disabled = disabled
		if err := writeConfig(*configPath, cfg); err != nil {
			fmt.Println("Config write error:", err.Error())
			return 1
		}
	}

	payload := map[string]interface{}{
		"status":   state,
		"disabled": cfg.Disabled,
		"config":   *configPath,
	}
	if *jsonOut || !isTerminal(os.Stdout) {
		printJSON(payload)
		return 0
	}

	fmt.Printf("Conductor %s: %s\n", state, *configPath)
	return 0
}
