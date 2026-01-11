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
	case "settings":
		os.Exit(runSettings(rest))
	case "status":
		os.Exit(runStatus(rest))
	case "config-validate":
		os.Exit(runConfigValidate(rest))
	case "doctor":
		os.Exit(runDoctor(rest))
	case "mcp-bundle":
		os.Exit(runMCPBundle(rest))
	case "mcp":
		os.Exit(runMCP(rest))
	case "mcp-gemini":
		os.Exit(runGeminiMCP(rest))
	case "mcp-claude":
		os.Exit(runClaudeMCP(rest))
	default:
		printHelp()
		os.Exit(1)
	}
}

func resolveCommand(args []string) (string, []string) {
	subcommands := map[string]bool{
		"install":         true,
		"uninstall":       true,
		"settings":        true,
		"status":          true,
		"config-validate": true,
		"doctor":          true,
		"mcp-bundle":      true,
		"mcp":             true,
		"mcp-gemini":      true,
		"mcp-claude":      true,
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
		"conductor-uninstall":           "uninstall",
		"conductor-settings":            "settings",
		"conductor-status":              "status",
		"conductor-config-validate":     "config-validate",
		"conductor-doctor":              "doctor",
		"conductor-mcp-bundle":          "mcp-bundle",
		"conductor-mcp":                 "mcp",
		"conductor-mcp-gemini":          "mcp-gemini",
		"conductor-mcp-claude":          "mcp-claude",
		"conductor-mcp.exe":             "mcp",
		"conductor-config-validate.exe": "config-validate",
		"conductor-doctor.exe":          "doctor",
		"conductor-settings.exe":        "settings",
		"conductor-uninstall.exe":       "uninstall",
		"conductor-mcp-bundle.exe":      "mcp-bundle",
		"conductor-mcp-gemini.exe":      "mcp-gemini",
		"conductor-mcp-claude.exe":      "mcp-claude",
		"conductor-status.exe":          "status",
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
  uninstall            Remove skills, commands, bins, and config
  settings             Update role CLI/model settings
  status               Check CLI availability and readiness
  config-validate      Validate conductor config JSON
  doctor               Check config and CLI availability
  mcp-bundle           Render MCP bundle templates for hosts
  mcp                  Run MCP server (stdio)
  mcp-gemini           Run Gemini CLI MCP server (stdio)
  mcp-claude           Run Claude CLI MCP server (stdio)

Aliases:
  conductor-kit, conductor-kit-install
  conductor-uninstall, conductor-settings, conductor-status
  conductor-config-validate, conductor-doctor
  conductor-mcp-bundle
  conductor-mcp
  conductor-mcp-gemini
  conductor-mcp-claude
`)
}
