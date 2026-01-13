package main

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

var (
	titleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("99")).
			MarginBottom(1)

	itemStyle = lipgloss.NewStyle().
			PaddingLeft(2)

	selectedItemStyle = lipgloss.NewStyle().
				PaddingLeft(2).
				Foreground(lipgloss.Color("170"))

	checkboxStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("212"))

	helpStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("241")).
			MarginTop(1)

	confirmStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color("82"))
)

type cliItem struct {
	name        string
	description string
	selected    bool
}

type cliSelectModel struct {
	items    []cliItem
	cursor   int
	quitting bool
	done     bool
}

func initialCLISelectModel() cliSelectModel {
	return cliSelectModel{
		items: []cliItem{
			{name: "codex", description: "OpenAI Codex CLI", selected: true},
			{name: "claude", description: "Anthropic Claude Code", selected: true},
			{name: "opencode", description: "OpenCode CLI", selected: true},
		},
	}
}

func (m cliSelectModel) Init() tea.Cmd {
	return nil
}

func (m cliSelectModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch {
		case key.Matches(msg, key.NewBinding(key.WithKeys("ctrl+c", "q"))):
			m.quitting = true
			return m, tea.Quit

		case key.Matches(msg, key.NewBinding(key.WithKeys("up", "k"))):
			if m.cursor > 0 {
				m.cursor--
			}

		case key.Matches(msg, key.NewBinding(key.WithKeys("down", "j"))):
			if m.cursor < len(m.items)-1 {
				m.cursor++
			}

		case key.Matches(msg, key.NewBinding(key.WithKeys(" ", "x"))):
			m.items[m.cursor].selected = !m.items[m.cursor].selected

		case key.Matches(msg, key.NewBinding(key.WithKeys("a"))):
			allSelected := true
			for _, item := range m.items {
				if !item.selected {
					allSelected = false
					break
				}
			}
			for i := range m.items {
				m.items[i].selected = !allSelected
			}

		case key.Matches(msg, key.NewBinding(key.WithKeys("enter"))):
			m.done = true
			return m, tea.Quit
		}
	}
	return m, nil
}

func (m cliSelectModel) View() string {
	if m.quitting {
		return ""
	}

	var b strings.Builder

	b.WriteString(titleStyle.Render("🎛  Select CLIs to install"))
	b.WriteString("\n\n")

	for i, item := range m.items {
		cursor := "  "
		if m.cursor == i {
			cursor = "▸ "
		}

		checkbox := "[ ]"
		if item.selected {
			checkbox = checkboxStyle.Render("[✓]")
		}

		line := fmt.Sprintf("%s %s %s", cursor, checkbox, item.name)
		if item.description != "" {
			line += lipgloss.NewStyle().Foreground(lipgloss.Color("241")).Render(" - " + item.description)
		}

		if m.cursor == i {
			b.WriteString(selectedItemStyle.Render(line))
		} else {
			b.WriteString(itemStyle.Render(line))
		}
		b.WriteString("\n")
	}

	selected := []string{}
	for _, item := range m.items {
		if item.selected {
			selected = append(selected, item.name)
		}
	}

	b.WriteString("\n")
	if len(selected) > 0 {
		b.WriteString(confirmStyle.Render(fmt.Sprintf("Selected: %s", strings.Join(selected, ", "))))
	} else {
		b.WriteString(lipgloss.NewStyle().Foreground(lipgloss.Color("208")).Render("No CLIs selected"))
	}

	b.WriteString("\n")
	b.WriteString(helpStyle.Render("↑/↓: navigate • space/x: toggle • a: toggle all • enter: confirm • q: quit"))

	return b.String()
}

func promptCLISelectionTUI() map[string]bool {
	m := initialCLISelectModel()
	p := tea.NewProgram(m)

	finalModel, err := p.Run()
	if err != nil {
		fmt.Printf("TUI error: %v\n", err)
		return map[string]bool{"codex": true, "claude": true, "opencode": true}
	}

	result := finalModel.(cliSelectModel)
	if result.quitting {
		return map[string]bool{}
	}

	selected := map[string]bool{}
	for _, item := range result.items {
		if item.selected {
			selected[item.name] = true
		}
	}
	return selected
}
