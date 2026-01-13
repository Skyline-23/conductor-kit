package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
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

	renderStatusPretty(payload, ok)
	if ok {
		return 0
	}
	return 1
}

func renderStatusPretty(payload map[string]interface{}, allOK bool) {
	var sb strings.Builder

	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("99")).Render("🎛  Conductor Status")
	sb.WriteString(title + "\n\n")

	configPath, _ := payload["config"].(string)
	sb.WriteString(labelStyle.Render("Config: ") + pathStyle.Render(configPath) + "\n")
	sb.WriteString(renderDivider(50) + "\n\n")

	roles, _ := payload["roles"].([]map[string]interface{})

	for _, role := range roles {
		name, _ := role["role"].(string)
		status, _ := role["status"].(string)
		cli, _ := role["cli"].(string)
		model, _ := role["model"].(string)
		errMsg, _ := role["error"].(string)

		icon := renderStatusIcon(status)
		roleLine := fmt.Sprintf("%s %s", icon, roleNameStyle.Render(name))

		var details []string
		if cli != "" {
			details = append(details, labelStyle.Render("cli=")+valueStyle.Render(cli))
		}
		if model != "" {
			details = append(details, labelStyle.Render("model=")+valueStyle.Render(model))
		}

		if len(details) > 0 {
			roleLine += "  " + strings.Join(details, " ")
		}

		sb.WriteString(roleLine + "\n")

		if errMsg != "" {
			errLine := "    " + statusErrorStyle.Render("└─ "+errMsg)
			sb.WriteString(errLine + "\n")
		}
	}

	sb.WriteString("\n")
	if allOK {
		summary := statusOKStyle.Render("✓ All systems ready")
		sb.WriteString(summary + "\n")
	} else {
		summary := statusErrorStyle.Render("✗ Some issues detected")
		sb.WriteString(summary + "\n")
	}

	fmt.Print(sb.String())
}
