package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
)

func runLogin(args []string) int {
	fs := flag.NewFlagSet("login", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	configPath := fs.String("config", resolveConfigPath(""), "config path")
	role := fs.String("role", "", "role name")
	cliFlag := fs.String("cli", "", "cli name")
	deviceAuth := fs.Bool("device-auth", false, "use device auth for codex")
	gcloud := fs.Bool("gcloud", false, "use gcloud auth login for gemini")
	adc := fs.Bool("adc", false, "use gcloud application-default login for gemini")
	if err := fs.Parse(args); err != nil {
		fmt.Println(loginHelp())
		return 1
	}

	cli := *cliFlag
	if cli == "" && fs.NArg() > 0 {
		cli = fs.Arg(0)
	}
	if cli == "" && *role != "" {
		cfg, err := loadConfig(*configPath)
		if err != nil {
			fmt.Println("Config error:", err.Error())
			return 1
		}
		roleCfg, ok := cfg.Roles[*role]
		if !ok || roleCfg.CLI == "" {
			fmt.Printf("Role %s has no cli configured.\n", *role)
			return 1
		}
		cli = roleCfg.CLI
	}

	if cli == "" {
		fmt.Println("Missing CLI. Provide a CLI name or --role.")
		fmt.Println(loginHelp())
		return 1
	}

	if !isCommandAvailable(cli) && !(cli == "gcloud" && (*gcloud || *adc)) {
		fmt.Printf("Missing CLI on PATH: %s\n", cli)
		printInstallHint(cli)
		return 1
	}

	switch cli {
	case "codex":
		args := []string{"login"}
		if *deviceAuth {
			args = append(args, "--device-auth")
		}
		return runInteractive(cli, args)
	case "claude":
		fmt.Println("Claude Code login runs inside the interactive session.")
		fmt.Println("Starting claude; run /login in the prompt.")
		return runInteractive(cli, nil)
	case "gemini":
		if *gcloud || *adc {
			return runGcloudLogin(*adc)
		}
		fmt.Println("Gemini login happens on first run.")
		fmt.Println("Starting gemini; follow the auth prompts.")
		return runInteractive(cli, nil)
	default:
		return runInteractive(cli, nil)
	}
}

func loginHelp() string {
	return `conductor login

Usage:
  conductor login codex [--device-auth]
  conductor login claude
  conductor login gemini [--gcloud|--adc]
  conductor login --role <role>
`
}

func runInteractive(cmd string, args []string) int {
	c := exec.Command(cmd, args...)
	c.Stdin = os.Stdin
	c.Stdout = os.Stdout
	c.Stderr = os.Stderr
	if err := c.Run(); err != nil {
		fmt.Printf("%s login failed: %v\n", cmd, err)
		return 1
	}
	return 0
}

func runGcloudLogin(adc bool) int {
	if !isCommandAvailable("gcloud") {
		fmt.Println("Missing CLI on PATH: gcloud")
		printInstallHint("gcloud")
		return 1
	}
	args := []string{"auth", "login"}
	if adc {
		args = []string{"auth", "application-default", "login"}
	}
	return runInteractive("gcloud", args)
}

func printInstallHint(cli string) {
	switch cli {
	case "codex":
		fmt.Println("Install: npm i -g @openai/codex")
	case "claude":
		fmt.Println("Install Claude Code CLI from the Anthropic docs.")
	case "gemini":
		fmt.Println("Install Gemini CLI or gcloud, then rerun.")
	case "gcloud":
		fmt.Println("Install gcloud (Google Cloud SDK), then rerun.")
	default:
		fmt.Println("Install the CLI and rerun.")
	}
}
