package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

func runRoles(args []string) int {
	fs := flag.NewFlagSet("roles", flag.ContinueOnError)
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

	payload := listRolesPayload(cfg, *configPath)
	if *jsonOut || !isTerminal(os.Stdout) {
		printJSON(payload)
		return 0
	}

	renderRolesPretty(payload)
	return 0
}

func renderRolesPretty(payload map[string]interface{}) {
	var sb strings.Builder

	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("99")).Render("🎛  Conductor Roles")
	sb.WriteString(title + "\n\n")

	configPath, _ := payload["config"].(string)
	sb.WriteString(labelStyle.Render("Config: ") + pathStyle.Render(configPath) + "\n")
	sb.WriteString(renderDivider(50) + "\n\n")

	roles, _ := payload["roles"].([]map[string]interface{})

	for _, role := range roles {
		name, _ := role["role"].(string)
		cli, _ := role["cli"].(string)
		model, _ := role["model"].(string)
		reasoning, _ := role["reasoning"].(string)

		roleLine := fmt.Sprintf("• %s", roleNameStyle.Render(name))

		var details []string
		if cli != "" {
			details = append(details, labelStyle.Render("cli=")+valueStyle.Render(cli))
		}
		if model != "" {
			details = append(details, labelStyle.Render("model=")+valueStyle.Render(model))
		}
		if reasoning != "" {
			details = append(details, labelStyle.Render("reasoning=")+valueStyle.Render(reasoning))
		}

		if len(details) > 0 {
			roleLine += "  " + strings.Join(details, " ")
		}

		sb.WriteString(roleLine + "\n")
	}

	fmt.Print(sb.String())
}
