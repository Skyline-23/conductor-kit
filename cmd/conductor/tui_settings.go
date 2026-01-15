package main

import (
	"fmt"
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

type settingsStep int

const (
	stepSelectRole settingsStep = iota
	stepDeleteRole
	stepSelectCLI
	stepSelectModel
	stepSelectReasoning
	stepInputRole
	stepInputModel
)

type settingsModel struct {
	cfg        Config
	configPath string

	step         settingsStep
	cursor       int
	options      []tuiOption
	roleName     string
	roleCfg      RoleConfig
	modelCLI     string
	modelOptions []tuiOption

	textInput textinput.Model
	quitting  bool
	saved     bool
	message   string
}

func newSettingsModel(cfg Config, configPath string) settingsModel {
	ti := textinput.New()
	ti.Focus()
	ti.CharLimit = 64
	ti.Width = 40

	return settingsModel{
		cfg:        cfg,
		configPath: configPath,
		step:       stepSelectRole,
		textInput:  ti,
	}
}

func (m settingsModel) Init() tea.Cmd {
	return textinput.Blink
}

func (m settingsModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	if m.step == stepSelectModel {
		m = m.ensureModelOptions()
	}

	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch m.step {
		case stepInputRole, stepInputModel:
			return m.handleTextInput(msg)
		default:
			return m.handleSelection(msg)
		}
	}

	if m.step == stepInputRole || m.step == stepInputModel {
		var cmd tea.Cmd
		m.textInput, cmd = m.textInput.Update(msg)
		return m, cmd
	}

	return m, nil
}

func (m settingsModel) handleTextInput(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch {
	case key.Matches(msg, key.NewBinding(key.WithKeys("ctrl+c"))):
		m.quitting = true
		return m, tea.Quit

	case key.Matches(msg, key.NewBinding(key.WithKeys("esc"))):
		if m.step == stepInputRole {
			m.step = stepSelectRole
		} else {
			m.step = stepSelectModel
		}
		return m, nil

	case key.Matches(msg, key.NewBinding(key.WithKeys("enter"))):
		value := strings.TrimSpace(m.textInput.Value())
		if value == "" {
			return m, nil
		}
		if m.step == stepInputRole {
			m.roleName = value
			m.roleCfg = m.cfg.Roles[m.roleName]
			m.step = stepSelectCLI
		} else {
			m.roleCfg.Model = value
			if m.roleCfg.CLI == "codex" {
				m.step = stepSelectReasoning
			} else {
				(&m).saveAndReset()
			}
		}
		m.textInput.SetValue("")
		return m, nil
	}

	var cmd tea.Cmd
	m.textInput, cmd = m.textInput.Update(msg)
	return m, cmd
}

func (m settingsModel) handleSelection(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch {
	case key.Matches(msg, key.NewBinding(key.WithKeys("ctrl+c", "q"))):
		m.quitting = true
		return m, tea.Quit

	case key.Matches(msg, key.NewBinding(key.WithKeys("esc"))):
		switch m.step {
		case stepSelectRole:
			m.quitting = true
			return m, tea.Quit
		case stepSelectCLI:
			m.step = stepSelectRole
		case stepSelectModel:
			m.step = stepSelectCLI
		case stepSelectReasoning:
			m.step = stepSelectModel
		case stepDeleteRole:
			m.step = stepSelectRole
		}
		m.cursor = 0
		return m, nil

	case key.Matches(msg, key.NewBinding(key.WithKeys("up", "k"))):
		if m.cursor > 0 {
			m.cursor--
		}

	case key.Matches(msg, key.NewBinding(key.WithKeys("down", "j"))):
		options := m.getOptions()
		if m.cursor < len(options)-1 {
			m.cursor++
		}

	case key.Matches(msg, key.NewBinding(key.WithKeys("enter"))):
		return m.selectOption()
	}

	return m, nil
}

func (m settingsModel) selectOption() (tea.Model, tea.Cmd) {
	options := m.getOptions()
	if len(options) == 0 || m.cursor >= len(options) {
		return m, nil
	}

	if m.step == stepSelectRole {
		m.message = ""
	}
	selected := options[m.cursor].Value

	switch m.step {
	case stepSelectRole:
		if selected == tuiNewRole {
			m.textInput.SetValue("")
			m.textInput.Placeholder = "Enter role name"
			m.step = stepInputRole
		} else if selected == tuiDeleteRole {
			m.roleName = ""
			m.roleCfg = RoleConfig{}
			m.step = stepDeleteRole
		} else {
			m.roleName = selected
			m.roleCfg = m.cfg.Roles[m.roleName]
			m.step = stepSelectCLI
		}

	case stepSelectCLI:
		if selected == tuiManualValue {
			m.textInput.SetValue(m.roleCfg.CLI)
			m.textInput.Placeholder = "Enter CLI name"
			m.step = stepInputModel
		} else {
			m.roleCfg.CLI = selected
			m.modelCLI = ""
			m.modelOptions = nil
			m.step = stepSelectModel
		}

	case stepSelectModel:
		if selected == tuiManualValue {
			m.textInput.SetValue(m.roleCfg.Model)
			m.textInput.Placeholder = "Enter model name"
			m.step = stepInputModel
		} else {
			m.roleCfg.Model = selected
			if m.roleCfg.CLI == "codex" {
				m.step = stepSelectReasoning
			} else {
				// Save and go back to role selection
				(&m).saveAndReset()
			}
		}

	case stepSelectReasoning:
		if selected != tuiKeepValue {
			m.roleCfg.Reasoning = selected
		}
		// Save and go back to role selection
		(&m).saveAndReset()

	case stepDeleteRole:
		if selected == tuiCancelValue {
			m.step = stepSelectRole
			return m, nil
		}
		if _, ok := m.cfg.Roles[selected]; !ok {
			m.message = "Role not found."
			m.step = stepSelectRole
			return m, nil
		}
		delete(m.cfg.Roles, selected)
		if err := writeConfig(m.configPath, m.cfg); err != nil {
			m.message = "Error: " + err.Error()
		} else {
			m.message = fmt.Sprintf("Deleted %s", selected)
		}
		m.roleName = ""
		m.roleCfg = RoleConfig{}
		m.step = stepSelectRole

	}

	m.cursor = 0
	return m, nil
}

func (m *settingsModel) saveAndReset() {
	if m.cfg.Roles == nil {
		m.cfg.Roles = map[string]RoleConfig{}
	}
	m.cfg.Roles[m.roleName] = m.roleCfg
	if err := writeConfig(m.configPath, m.cfg); err != nil {
		m.message = "Error: " + err.Error()
	} else {
		m.message = fmt.Sprintf("Saved %s", m.roleName)
	}
	m.step = stepSelectRole
	m.roleName = ""
	m.roleCfg = RoleConfig{}
	m.modelCLI = ""
	m.modelOptions = nil
	m.cursor = 0
}

func (m settingsModel) ensureModelOptions() settingsModel {
	if m.roleCfg.CLI == "" {
		m.modelCLI = ""
		m.modelOptions = []tuiOption{{Label: "Manual entry...", Value: tuiManualValue}}
		return m
	}
	if m.modelCLI != m.roleCfg.CLI || m.modelOptions == nil {
		models, _ := listModelsForCLI(m.roleCfg.CLI, false)
		// Only add current model if it belongs to this CLI's model list
		if m.roleCfg.Model != "" && contains(models, m.roleCfg.Model) {
			// Model already in list, no need to add
		} else if m.roleCfg.Model != "" {
			// Model doesn't belong to this CLI, don't add it
			// This prevents mixing models from different CLIs
		}
		options := make([]tuiOption, 0, len(models)+1)
		for _, model := range models {
			options = append(options, tuiOption{Label: model, Value: model})
		}
		options = append(options, tuiOption{Label: "Manual entry...", Value: tuiManualValue})
		m.modelCLI = m.roleCfg.CLI
		m.modelOptions = options
	}
	return m
}

func (m settingsModel) getOptions() []tuiOption {
	switch m.step {
	case stepSelectRole:
		roles := make([]string, 0, len(m.cfg.Roles))
		for name := range m.cfg.Roles {
			roles = append(roles, name)
		}
		sort.Strings(roles)
		options := make([]tuiOption, 0, len(roles)+2)
		for _, name := range roles {
			options = append(options, tuiOption{Label: name, Value: name})
		}
		options = append(options, tuiOption{Label: "+ New role", Value: tuiNewRole})
		if len(roles) > 0 {
			options = append(options, tuiOption{Label: "Delete role...", Value: tuiDeleteRole})
		}
		return options

	case stepSelectCLI:
		return []tuiOption{
			{Label: "codex", Value: "codex"},
			{Label: "claude", Value: "claude"},
			{Label: "gemini", Value: "gemini"},
			{Label: "Manual entry...", Value: tuiManualValue},
		}

	case stepSelectModel:
		modelState := m.ensureModelOptions()
		return modelState.modelOptions

	case stepSelectReasoning:
		return []tuiOption{
			{Label: "Keep current", Value: tuiKeepValue},
			{Label: "low", Value: "low"},
			{Label: "medium", Value: "medium"},
			{Label: "high", Value: "high"},
			{Label: "xhigh", Value: "xhigh"},
		}

	case stepDeleteRole:
		roles := make([]string, 0, len(m.cfg.Roles))
		for name := range m.cfg.Roles {
			roles = append(roles, name)
		}
		sort.Strings(roles)
		options := make([]tuiOption, 0, len(roles)+1)
		for _, name := range roles {
			options = append(options, tuiOption{Label: name, Value: name})
		}
		options = append(options, tuiOption{Label: "Cancel", Value: tuiCancelValue})
		return options

	}
	return nil
}

func (m settingsModel) View() string {
	if m.quitting || m.saved {
		if m.message != "" {
			return m.message + "\n"
		}
		return ""
	}

	var sb strings.Builder

	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("99")).Render("⚙  Conductor Settings")
	sb.WriteString(title + "\n")
	sb.WriteString(pathStyle.Render(m.configPath) + "\n")
	sb.WriteString(renderDivider(45) + "\n\n")

	if m.step == stepInputRole || m.step == stepInputModel {
		return m.renderTextInput(&sb)
	}

	return m.renderSelection(&sb)
}

func (m settingsModel) renderTextInput(sb *strings.Builder) string {
	var prompt string
	if m.step == stepInputRole {
		prompt = "Enter new role name:"
	} else {
		prompt = "Enter value:"
	}

	sb.WriteString(sectionStyle.Render(prompt) + "\n\n")
	sb.WriteString("  " + m.textInput.View() + "\n\n")
	sb.WriteString(helpStyle.Render("enter: confirm • esc: back"))

	return sb.String()
}

func (m settingsModel) renderSelection(sb *strings.Builder) string {
	stepTitle := m.getStepTitle()
	sb.WriteString(sectionStyle.Render(stepTitle) + "\n\n")
	if m.message != "" {
		sb.WriteString(valueStyle.Render(m.message) + "\n\n")
	}

	if m.roleName != "" {
		info := labelStyle.Render("Role: ") + roleNameStyle.Render(m.roleName)
		if m.roleCfg.CLI != "" {
			info += "  " + labelStyle.Render("CLI: ") + valueStyle.Render(m.roleCfg.CLI)
		}
		if m.roleCfg.Model != "" {
			info += "  " + labelStyle.Render("Model: ") + valueStyle.Render(m.roleCfg.Model)
		}
		sb.WriteString(info + "\n\n")
	}

	options := m.getOptions()
	for i, opt := range options {
		cursor := "  "
		style := itemStyle
		if i == m.cursor {
			cursor = lipgloss.NewStyle().Foreground(lipgloss.Color("212")).Render("▸ ")
			style = selectedItemStyle
		}

		line := cursor + style.Render(opt.Label)
		sb.WriteString(line + "\n")
	}

	sb.WriteString("\n")
	sb.WriteString(helpStyle.Render("↑/↓: navigate • enter: select • esc: back • q: quit"))

	return sb.String()
}

func (m settingsModel) getStepTitle() string {
	switch m.step {
	case stepSelectRole:
		return "Select Role"
	case stepSelectCLI:
		return "Select CLI"
	case stepSelectModel:
		return "Select Model"
	case stepSelectReasoning:
		return "Reasoning Effort"
	case stepDeleteRole:
		return "Delete Role"
	default:
		return ""
	}
}

func runSettingsBubbleTea(cfg Config, configPath string) int {
	m := newSettingsModel(cfg, configPath)
	p := tea.NewProgram(m)

	finalModel, err := p.Run()
	if err != nil {
		fmt.Printf("TUI error: %v\n", err)
		return 1
	}

	result := finalModel.(settingsModel)
	if result.saved {
		fmt.Printf("Updated %s\n", configPath)
		return 0
	}
	return 0
}
