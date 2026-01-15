package main

import "github.com/charmbracelet/lipgloss"

var (
	// Colors - Cyan-based theme for consistency
	colorPrimary   = lipgloss.Color("86")  // Bright cyan
	colorSecondary = lipgloss.Color("242") // Dim gray
	colorSuccess   = lipgloss.Color("78")  // Green
	colorWarning   = lipgloss.Color("214") // Orange
	colorError     = lipgloss.Color("196") // Red
	colorHighlight = lipgloss.Color("86")  // Cyan (same as primary)
	colorMuted     = lipgloss.Color("242") // Gray

	// Status/Doctor styles
	headerStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorPrimary).
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(colorPrimary).
			Padding(0, 1)

	sectionStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorPrimary).
			MarginTop(1)

	roleNameStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorPrimary)

	statusOKStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorSuccess)

	statusWarnStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(colorWarning)

	statusErrorStyle = lipgloss.NewStyle().
				Bold(true).
				Foreground(colorError)

	statusMissingStyle = lipgloss.NewStyle().
				Bold(true).
				Foreground(colorError)

	labelStyle = lipgloss.NewStyle().
			Foreground(colorMuted)

	valueStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("252"))

	pathStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("244")).
			Italic(true)

	boxStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(colorSecondary).
			Padding(0, 1).
			MarginBottom(1)

	successBoxStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(colorSuccess).
			Padding(0, 1)

	errorBoxStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(colorError).
			Padding(0, 1)

	dividerStyle = lipgloss.NewStyle().
			Foreground(colorSecondary)

	iconOK      = statusOKStyle.Render("✓")
	iconWarn    = statusWarnStyle.Render("!")
	iconError   = statusErrorStyle.Render("✗")
	iconMissing = statusMissingStyle.Render("○")
	iconArrow   = lipgloss.NewStyle().Foreground(colorHighlight).Render("→")
)

func renderStatusIcon(status string) string {
	switch status {
	case "ready", "ok":
		return iconOK
	case "warn", "warning":
		return iconWarn
	case "error", "not_ready":
		return iconError
	case "missing":
		return iconMissing
	default:
		return iconMissing
	}
}

func renderDivider(width int) string {
	line := ""
	for i := 0; i < width; i++ {
		line += "─"
	}
	return dividerStyle.Render(line)
}
