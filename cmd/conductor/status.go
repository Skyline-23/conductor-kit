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

	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("99")).Render("Conductor Status")
	sb.WriteString(title + "\n\n")

	configPath, _ := payload["config"].(string)
	sb.WriteString(labelStyle.Render("Config: ") + pathStyle.Render(configPath) + "\n\n")

	// CLI Auth Status section
	sb.WriteString(lipgloss.NewStyle().Bold(true).Render("CLIs") + "\n")
	sb.WriteString(renderDivider(50) + "\n")
	renderCLIAuthStatus(&sb)
	sb.WriteString("\n")

	// Roles section
	sb.WriteString(lipgloss.NewStyle().Bold(true).Render("Roles") + "\n")
	sb.WriteString(renderDivider(50) + "\n")

	roles, _ := payload["roles"].([]map[string]interface{})

	for _, role := range roles {
		name, _ := role["role"].(string)
		status, _ := role["status"].(string)
		cli, _ := role["cli"].(string)
		model, _ := role["model"].(string)
		reasoning, _ := role["reasoning"].(string)

		icon := renderStatusIcon(status)

		// Format: icon role-name (padded) cli model [reasoning]
		rolePart := fmt.Sprintf("%-24s", name)
		cliPart := fmt.Sprintf("%-8s", cli)
		modelPart := model

		line := fmt.Sprintf("%s %s %s %s", icon, roleNameStyle.Render(rolePart), valueStyle.Render(cliPart), valueStyle.Render(modelPart))

		if reasoning != "" {
			line += " " + lipgloss.NewStyle().Foreground(lipgloss.Color("208")).Render("["+reasoning+"]")
		}

		sb.WriteString(line + "\n")
	}

	sb.WriteString("\n")
	if allOK {
		summary := statusOKStyle.Render("All systems ready")
		sb.WriteString(summary + "\n")
	} else {
		summary := statusErrorStyle.Render("Some issues detected")
		sb.WriteString(summary + "\n")
	}

	fmt.Print(sb.String())
}

func renderCLIAuthStatus(sb *strings.Builder) {
	clis := []struct {
		name  string
		check func() (bool, string)
	}{
		{"codex", mcpCheckCodexAuth},
		{"claude", mcpCheckClaudeAuth},
		{"gemini", mcpCheckGeminiAuth},
	}

	for _, cli := range clis {
		authed, status := cli.check()
		var icon, statusText string
		if !isCommandAvailable(cli.name) {
			icon = lipgloss.NewStyle().Foreground(grayColor).Render("○")
			statusText = lipgloss.NewStyle().Foreground(grayColor).Render("not installed")
		} else if authed {
			icon = lipgloss.NewStyle().Foreground(greenColor).Render("●")
			statusText = lipgloss.NewStyle().Foreground(greenColor).Render(status)
		} else {
			icon = lipgloss.NewStyle().Foreground(orangeColor).Render("○")
			statusText = lipgloss.NewStyle().Foreground(orangeColor).Render(status)
		}
		line := fmt.Sprintf("%s %-10s %s", icon, cli.name, statusText)
		sb.WriteString(line + "\n")
	}
}
