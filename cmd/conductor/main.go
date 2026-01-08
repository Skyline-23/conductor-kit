package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	cmd, rest := resolveCommand(os.Args[1:])
	switch cmd {
	case "install":
		os.Exit(runInstall(rest))
	case "background-task":
		os.Exit(runBackgroundTask(rest))
	case "background-output":
		os.Exit(runBackgroundOutput(rest))
	case "background-cancel":
		os.Exit(runBackgroundCancel(rest))
	case "background-batch":
		os.Exit(runBackgroundBatch(rest))
	case "mcp":
		os.Exit(runMCP(rest))
	default:
		printHelp()
		os.Exit(1)
	}
}

func resolveCommand(args []string) (string, []string) {
	subcommands := map[string]bool{
		"install":           true,
		"background-task":   true,
		"background-output": true,
		"background-cancel": true,
		"background-batch":  true,
		"mcp":               true,
	}

	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		if subcommands[args[0]] {
			return args[0], args[1:]
		}
	}

	alias := map[string]string{
		"conductor":                       "",
		"conductor-kit":                   "install",
		"conductor-kit-install":           "install",
		"conductor-background-task":       "background-task",
		"conductor-background-output":     "background-output",
		"conductor-background-cancel":     "background-cancel",
		"conductor-background-batch":      "background-batch",
		"conductor-mcp":                   "mcp",
		"conductor-background-task.exe":   "background-task",
		"conductor-background-output.exe": "background-output",
		"conductor-background-cancel.exe": "background-cancel",
		"conductor-background-batch.exe":  "background-batch",
		"conductor-mcp.exe":               "mcp",
	}

	exe := filepath.Base(os.Args[0])
	if mapped, ok := alias[exe]; ok {
		if mapped == "" {
			return "", args
		}
		return mapped, args
	}

	return "", args
}

func printHelp() {
	fmt.Println(`conductor

Usage:
  conductor <command> [options]

Commands:
  install              Install skills, commands, bins, and config
  background-task      Run a CLI in background for a role or agent
  background-output    Fetch output for a background task
  background-cancel    Cancel background tasks
  background-batch     Start background tasks for roles/agents in parallel
  mcp                  Run MCP server (stdio)

Aliases:
  conductor-kit, conductor-kit-install
  conductor-background-task, conductor-background-output, conductor-background-cancel, conductor-background-batch
  conductor-mcp
`)
}
