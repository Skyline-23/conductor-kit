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
	case "uninstall":
		os.Exit(runUninstall(rest))
	case "config-validate":
		os.Exit(runConfigValidate(rest))
	case "doctor":
		os.Exit(runDoctor(rest))
	case "mcp":
		os.Exit(runMCP(rest))
	default:
		printHelp()
		os.Exit(1)
	}
}

func resolveCommand(args []string) (string, []string) {
	subcommands := map[string]bool{
		"install":         true,
		"uninstall":       true,
		"config-validate": true,
		"doctor":          true,
		"mcp":             true,
	}

	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		if subcommands[args[0]] {
			return args[0], args[1:]
		}
	}

	alias := map[string]string{
		"conductor":                     "",
		"conductor-kit":                 "install",
		"conductor-kit-install":         "install",
		"conductor-kit-uninstall":       "uninstall",
		"conductor-config-validate":     "config-validate",
		"conductor-doctor":              "doctor",
		"conductor-mcp":                 "mcp",
		"conductor-mcp.exe":             "mcp",
		"conductor-kit-uninstall.exe":   "uninstall",
		"conductor-config-validate.exe": "config-validate",
		"conductor-doctor.exe":          "doctor",
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
	fmt.Print(`conductor

Usage:
  conductor <command> [options]

Commands:
  install              Install skills, commands, bins, and config
  uninstall            Remove skills, commands/prompts, bins, and config
  config-validate      Validate conductor config JSON
  doctor               Check config and CLI availability
  mcp                  Run MCP server (stdio)

Aliases:
  conductor-kit, conductor-kit-install
  conductor-kit-uninstall
  conductor-config-validate, conductor-doctor
  conductor-mcp
`)
}
