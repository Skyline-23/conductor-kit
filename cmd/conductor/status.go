package main

import (
	"flag"
	"fmt"
	"io"
	"os"
)

func runStatus(args []string) int {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
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

	payload, ok := statusPayload(cfg, *configPath)
	if *jsonOut || !isTerminal(os.Stdout) {
		printJSON(payload)
		if ok {
			return 0
		}
		return 1
	}

	roles, _ := payload["roles"].([]map[string]interface{})
	fmt.Printf("Config: %s\n", payload["config"])
	for _, role := range roles {
		name, _ := role["role"].(string)
		status, _ := role["status"].(string)
		line := fmt.Sprintf("- %s: %s", name, status)
		if cli, ok := role["cli"].(string); ok && cli != "" {
			line += fmt.Sprintf(" (cli=%s)", cli)
		}
		if model, ok := role["model"].(string); ok && model != "" {
			line += fmt.Sprintf(" model=%s", model)
		}
		if errMsg, ok := role["error"].(string); ok && errMsg != "" {
			line += fmt.Sprintf(" :: %s", errMsg)
		}
		fmt.Println(line)
	}
	if ok {
		return 0
	}
	return 1
}
