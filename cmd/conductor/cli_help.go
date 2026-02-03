package main

import (
	"flag"
	"fmt"
	"strings"
)

func wantsHelp(args []string) bool {
	if len(args) == 0 {
		return false
	}
	switch args[0] {
	case "-h", "--help", "help":
		return true
	default:
		return false
	}
}

func parseFlags(fs *flag.FlagSet, args []string, helpFn func() string) (bool, int) {
	if wantsHelp(args) {
		if helpFn != nil {
			fmt.Println(helpFn())
		}
		return false, 0
	}
	if err := fs.Parse(args); err != nil {
		if helpFn != nil {
			fmt.Printf("Invalid flags: %v\n\n%s", err, helpFn())
		} else {
			fmt.Printf("Invalid flags: %v\n", err)
		}
		return false, 1
	}
	return true, 0
}

func runHelp(args []string) int {
	if len(args) == 0 {
		printHelp()
		return 0
	}
	name := strings.TrimSpace(args[0])
	if name == "" {
		printHelp()
		return 0
	}
	if help, ok := commandHelp(name); ok {
		fmt.Println(help)
		return 0
	}
	fmt.Printf("Unknown command: %s\n\n", name)
	printHelp()
	return 1
}

func commandHelp(name string) (string, bool) {
	switch name {
	case "install":
		return installHelp(), true
	case "uninstall":
		return uninstallHelp(), true
	case "disable":
		return toggleHelp("disable"), true
	case "enable":
		return toggleHelp("enable"), true
	case "settings":
		return settingsHelp(), true
	case "status":
		return statusHelp(), true
	case "roles":
		return rolesHelp(), true
	case "config-validate":
		return configValidateHelp(), true
	case "doctor":
		return doctorHelp(), true
	case "mcp-bundle":
		return mcpBundleHelp(), true
	case "mcp":
		return mcpHelp(), true
	case "help":
		return `conductor help

Usage:
  conductor help [command]
`, true
	default:
		return "", false
	}
}
