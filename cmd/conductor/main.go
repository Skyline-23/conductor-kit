package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// Version is set by goreleaser at build time via -ldflags.
var Version = "dev"

func main() {
	// Handle --version / -v flag before command resolution
	if len(os.Args) > 1 {
		arg := os.Args[1]
		if arg == "--version" || arg == "-v" || arg == "version" {
			fmt.Printf("conductor %s\n", Version)
			os.Exit(0)
		}
	}

	cmd, rest := resolveCommand(os.Args[1:])
	switch cmd {
	case "install":
		os.Exit(runInstall(rest))
	case "uninstall":
		os.Exit(runUninstall(rest))
	case "disable":
		os.Exit(runDisable(rest))
	case "enable":
		os.Exit(runEnable(rest))
	case "settings":
		os.Exit(runSettings(rest))
	case "status":
		os.Exit(runStatus(rest))
	case "roles":
		os.Exit(runRoles(rest))
	case "config-validate":
		os.Exit(runConfigValidate(rest))
	case "doctor":
		os.Exit(runDoctor(rest))
	case "mcp-bundle":
		os.Exit(runMCPBundle(rest))
	case "mcp":
		os.Exit(runMCPServer(rest))

	default:
		printHelp()
		os.Exit(1)
	}
}

func resolveCommand(args []string) (string, []string) {
	subcommands := map[string]bool{
		"install":         true,
		"uninstall":       true,
		"disable":         true,
		"enable":          true,
		"settings":        true,
		"status":          true,
		"roles":           true,
		"config-validate": true,
		"doctor":          true,
		"mcp-bundle":      true,
		"mcp":             true,
	}

	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		if subcommands[args[0]] {
			return args[0], args[1:]
		}
	}

	alias := map[string]string{
		"conductor":                 "",
		"conductor-kit":             "install",
		"conductor-kit-install":     "install",
		"conductor-disable":         "disable",
		"conductor-enable":          "enable",
		"conductor-uninstall":       "uninstall",
		"conductor-settings":        "settings",
		"conductor-status":          "status",
		"conductor-config-validate": "config-validate",
		"conductor-doctor":          "doctor",
		"conductor-mcp-bundle":      "mcp-bundle",
		"conductor-mcp":             "mcp",

		"conductor-config-validate.exe": "config-validate",
		"conductor-doctor.exe":          "doctor",
		"conductor-disable.exe":         "disable",
		"conductor-enable.exe":          "enable",
		"conductor-settings.exe":        "settings",
		"conductor-uninstall.exe":       "uninstall",
		"conductor-mcp-bundle.exe":      "mcp-bundle",
		"conductor-mcp.exe":             "mcp",
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
	fmt.Printf(`conductor %s

Usage:
  conductor <command> [options]

Commands:
	  install              Install skills, commands, bins, and config
	  uninstall            Remove skills, commands, bins, and config
	  disable              Disable conductor role routing
	  enable               Re-enable conductor role routing
	  settings             Update role CLI/model settings
  status               Check CLI availability and readiness
  roles                List role -> CLI/model mappings
  config-validate      Validate conductor config JSON
  doctor               Check config and CLI availability
  mcp-bundle           Render MCP bundle templates for hosts
  mcp                  Run unified MCP server (codex/claude/gemini + conductor)
  version              Show version information

	Aliases:
	  conductor-kit, conductor-kit-install
	  conductor-disable, conductor-enable, conductor-uninstall
	  conductor-settings, conductor-status
  conductor-config-validate, conductor-doctor
  conductor-mcp-bundle, conductor-mcp
`, Version)
}
