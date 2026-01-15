package main

import (
	"fmt"
	"strings"
	"sync"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// Cyan-based color scheme for consistent TUI appearance
var (
	// Primary cyan color for headers and highlights
	cyanColor   = lipgloss.Color("86")  // Bright cyan
	cyanDim     = lipgloss.Color("37")  // Dim cyan
	greenColor  = lipgloss.Color("78")  // Success green
	grayColor   = lipgloss.Color("242") // Muted gray
	orangeColor = lipgloss.Color("208") // Warning orange

	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(cyanColor).
			MarginBottom(1)

	itemStyle = lipgloss.NewStyle().
			PaddingLeft(2)

	selectedItemStyle = lipgloss.NewStyle().
				PaddingLeft(2).
				Foreground(cyanColor).
				Bold(true)

	checkboxStyle = lipgloss.NewStyle().
			Foreground(cyanColor)

	helpStyle = lipgloss.NewStyle().
			Foreground(grayColor).
			MarginTop(1)

	confirmStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(greenColor)

	stepStyle = lipgloss.NewStyle().
			Foreground(grayColor).
			Render
)

type selectableItem struct {
	name        string
	description string
	selected    bool
	authStatus  string
	available   bool
}

type installStep int

const (
	stepSelectCLIs installStep = iota
	stepConfirmInstall
)

type installSelectModel struct {
	step     installStep
	cliItems []selectableItem
	cursor   int
	quitting bool
	done     bool
	width    int // terminal width
}

type cliStatus struct {
	available  bool
	authStatus string
}

func loadCLIStatuses(names []string) map[string]cliStatus {
	statuses := make(map[string]cliStatus, len(names))
	seen := map[string]bool{}
	var mu sync.Mutex
	var wg sync.WaitGroup
	for _, name := range names {
		name := strings.TrimSpace(name)
		if name == "" || seen[name] {
			continue
		}
		seen[name] = true
		wg.Add(1)
		go func(cliName string) {
			defer wg.Done()
			available := isCommandAvailable(cliName)
			authStatus := ""
			if available {
				// Get actual auth status (no truncation here - done in render)
				switch cliName {
				case "codex":
					auth, msg := mcpCheckCodexAuth()
					if auth {
						authStatus = msg
					} else {
						authStatus = "not authenticated"
					}
				case "claude":
					auth, msg := mcpCheckClaudeAuth()
					if auth {
						authStatus = msg
					} else {
						authStatus = "not authenticated"
					}
				case "gemini":
					auth, msg := mcpCheckGeminiAuth()
					if auth {
						authStatus = msg
					} else {
						authStatus = "not authenticated"
					}
				default:
					authStatus = "available"
				}
			}
			mu.Lock()
			statuses[cliName] = cliStatus{available: available, authStatus: authStatus}
			mu.Unlock()
		}(name)
	}
	wg.Wait()
	return statuses
}

func truncateStatus(s string, maxLen int) string {
	// Handle Unicode properly by using runes
	runes := []rune(s)
	if len(runes) <= maxLen {
		return s
	}
	return string(runes[:maxLen-3]) + "..."
}

func initialInstallSelectModel() installSelectModel {
	cliItems := []selectableItem{
		{name: "codex", description: "OpenAI Codex CLI"},
		{name: "claude", description: "Anthropic Claude Code"},
		{name: "opencode", description: "OpenCode CLI"},
	}

	names := []string{}
	for _, item := range cliItems {
		names = append(names, item.name)
	}
	statuses := loadCLIStatuses(names)

	for i := range cliItems {
		status, ok := statuses[cliItems[i].name]
		if !ok {
			status = cliStatus{available: false, authStatus: "missing"}
		}
		cliItems[i].available = status.available
		cliItems[i].authStatus = status.authStatus
		cliItems[i].selected = status.available
	}

	return installSelectModel{
		step:     stepSelectCLIs,
		cliItems: cliItems,
	}
}

func (m installSelectModel) Init() tea.Cmd {
	return tea.EnterAltScreen
}

func (m installSelectModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		return m, nil
	case tea.KeyMsg:
		switch {
		case key.Matches(msg, key.NewBinding(key.WithKeys("ctrl+c", "q"))):
			m.quitting = true
			return m, tea.Quit

		case key.Matches(msg, key.NewBinding(key.WithKeys("esc"))):
			if m.step == stepConfirmInstall {
				m.step = stepSelectCLIs
				m.cursor = 0
			} else {
				m.quitting = true
				return m, tea.Quit
			}

		case key.Matches(msg, key.NewBinding(key.WithKeys("up", "k"))):
			if m.cursor > 0 {
				m.cursor--
			}

		case key.Matches(msg, key.NewBinding(key.WithKeys("down", "j"))):
			items := m.currentItems()
			if m.cursor < len(items)-1 {
				m.cursor++
			}

		case key.Matches(msg, key.NewBinding(key.WithKeys(" ", "x"))):
			m.toggleCurrent()

		case key.Matches(msg, key.NewBinding(key.WithKeys("a"))):
			m.toggleAll()

		case key.Matches(msg, key.NewBinding(key.WithKeys("enter"))):
			return m.advance()
		}
	}
	return m, nil
}

func (m *installSelectModel) toggleCurrent() {
	if m.step == stepSelectCLIs {
		m.cliItems[m.cursor].selected = !m.cliItems[m.cursor].selected
	}
}

func (m *installSelectModel) toggleAll() {
	if m.step != stepSelectCLIs {
		return
	}
	allSelected := true
	for _, item := range m.cliItems {
		if !item.selected {
			allSelected = false
			break
		}
	}
	for i := range m.cliItems {
		m.cliItems[i].selected = !allSelected
	}
}

func (m installSelectModel) currentItems() []selectableItem {
	if m.step == stepSelectCLIs {
		return m.cliItems
	}
	return nil
}

func (m installSelectModel) advance() (tea.Model, tea.Cmd) {
	switch m.step {
	case stepSelectCLIs:
		m.step = stepConfirmInstall
		m.cursor = 0
	case stepConfirmInstall:
		m.done = true
		return m, tea.Quit
	}
	return m, nil
}

func (m installSelectModel) View() string {
	if m.quitting {
		return ""
	}

	var b strings.Builder

	b.WriteString(titleStyle.Render("Conductor Install"))
	b.WriteString("\n\n")
	b.WriteString(m.renderStepIndicator())
	b.WriteString("\n\n")

	switch m.step {
	case stepSelectCLIs:
		b.WriteString(m.renderCLIStep())
	case stepConfirmInstall:
		b.WriteString(m.renderConfirmStep())
	}

	return b.String()
}

func (m installSelectModel) renderStepIndicator() string {
	steps := []string{"CLI Targets", "Confirm"}
	var parts []string
	for i, s := range steps {
		if i == int(m.step) {
			parts = append(parts, lipgloss.NewStyle().Bold(true).Foreground(cyanColor).Render("● "+s))
		} else if i < int(m.step) {
			parts = append(parts, lipgloss.NewStyle().Foreground(greenColor).Render("✓ "+s))
		} else {
			parts = append(parts, lipgloss.NewStyle().Foreground(grayColor).Render("○ "+s))
		}
	}
	return strings.Join(parts, "  ")
}

func (m installSelectModel) renderCLIStep() string {
	var b strings.Builder
	b.WriteString(lipgloss.NewStyle().Bold(true).Render("Select CLI targets for skills/commands:"))
	b.WriteString("\n\n")

	for i, item := range m.cliItems {
		b.WriteString(m.renderItem(i, item, false)) // CLI: no description
		b.WriteString("\n")
	}

	b.WriteString("\n")
	b.WriteString(m.renderSelected(m.cliItems))
	b.WriteString("\n")
	b.WriteString(helpStyle.Render("↑/↓: navigate • space/x: toggle • a: all • enter: next • esc/q: quit"))

	return b.String()
}

func (m installSelectModel) renderConfirmStep() string {
	var b strings.Builder
	b.WriteString(lipgloss.NewStyle().Bold(true).Render("Review installation:"))
	b.WriteString("\n\n")

	selectedCLIs := m.getSelectedNames(m.cliItems)

	b.WriteString(labelStyle.Render("  CLI Targets: "))
	if len(selectedCLIs) > 0 {
		b.WriteString(valueStyle.Render(strings.Join(selectedCLIs, ", ")))
	} else {
		b.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("241")).Render("none"))
	}
	b.WriteString("\n")

	b.WriteString(labelStyle.Render("  MCP Bridge:  "))
	b.WriteString(valueStyle.Render("conductor (always installed)"))
	b.WriteString("\n\n")

	b.WriteString(confirmStyle.Render("Press enter to install, esc to go back, q to quit"))

	return b.String()
}

func (m installSelectModel) renderItem(idx int, item selectableItem, showDescription bool) string {
	cursor := "  "
	if m.cursor == idx {
		cursor = "▸ "
	}

	checkbox := "[ ]"
	if item.selected {
		checkbox = checkboxStyle.Render("[✓]")
	}

	// Simple status: installed or not
	status := renderSimpleStatus(item.available)

	// First line: cursor + checkbox + name + status
	firstLine := fmt.Sprintf("%s %s %-10s %s", cursor, checkbox, item.name, status)

	var result string
	if showDescription && item.description != "" {
		// Second line: indented description (for MCP page)
		descStyle := lipgloss.NewStyle().Foreground(grayColor)
		secondLine := fmt.Sprintf("         %s", descStyle.Render(item.description))
		if m.cursor == idx {
			result = selectedItemStyle.Render(firstLine) + "\n" + secondLine
		} else {
			result = itemStyle.Render(firstLine) + "\n" + secondLine
		}
	} else {
		if m.cursor == idx {
			result = selectedItemStyle.Render(firstLine)
		} else {
			result = itemStyle.Render(firstLine)
		}
	}
	return result
}

func renderSimpleStatus(available bool) string {
	if available {
		return lipgloss.NewStyle().Foreground(greenColor).Render("● installed")
	}
	return lipgloss.NewStyle().Foreground(grayColor).Render("○ not installed")
}

func (m installSelectModel) renderSelected(items []selectableItem) string {
	names := m.getSelectedNames(items)
	if len(names) > 0 {
		return confirmStyle.Render(fmt.Sprintf("Selected: %s", strings.Join(names, ", ")))
	}
	return lipgloss.NewStyle().Foreground(lipgloss.Color("208")).Render("None selected")
}

func (m installSelectModel) getSelectedNames(items []selectableItem) []string {
	var names []string
	for _, item := range items {
		if item.selected {
			names = append(names, item.name)
		}
	}
	return names
}

func renderCLIStatusBadge(available bool, authStatus string, maxWidth int) string {
	if !available {
		return lipgloss.NewStyle().Foreground(grayColor).Render("○ not installed")
	}
	if authStatus != "" && authStatus != "unknown" && authStatus != "missing" {
		// Truncate if needed (account for "● " prefix = 2 chars)
		status := authStatus
		if maxWidth > 4 && len([]rune(status)) > maxWidth-2 {
			status = truncateStatus(status, maxWidth-2)
		}
		return lipgloss.NewStyle().Foreground(greenColor).Render("● " + status)
	}
	return lipgloss.NewStyle().Foreground(greenColor).Render("● available")
}

type installSelections struct {
	clis map[string]bool
	mcps map[string]bool
}

func promptInstallSelectionsTUI() installSelections {
	m := initialInstallSelectModel()
	p := tea.NewProgram(m)

	finalModel, err := p.Run()
	if err != nil {
		fmt.Printf("TUI error: %v\n", err)
		return installSelections{
			clis: map[string]bool{"codex": true, "claude": true, "opencode": true},
			mcps: map[string]bool{"mcp": true},
		}
	}

	result := finalModel.(installSelectModel)
	if result.quitting {
		return installSelections{clis: map[string]bool{}, mcps: map[string]bool{}}
	}

	clis := map[string]bool{}
	for _, item := range result.cliItems {
		if item.selected {
			clis[item.name] = true
		}
	}

	// MCP is always installed
	mcps := map[string]bool{"mcp": true}

	return installSelections{clis: clis, mcps: mcps}
}

func promptCLISelection() map[string]bool {
	selections := promptInstallSelectionsTUI()
	return selections.clis
}

func promptCLISelectionTUI() map[string]bool {
	return promptCLISelection()
}
