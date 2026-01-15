package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestBuiltInModelList(t *testing.T) {
	tests := []struct {
		cli      string
		minCount int
		contains []string
	}{
		{"codex", 10, []string{"gpt-5.2-codex", "o3", "gpt-4o"}},
		{"claude", 8, []string{"opus", "sonnet", "haiku"}},
		{"gemini", 5, []string{"gemini-2.5-pro", "gemini-3-flash"}},
		{"unknown", 0, nil},
	}

	for _, tt := range tests {
		models := builtInModelList(tt.cli)
		if len(models) < tt.minCount {
			t.Errorf("builtInModelList(%q): expected at least %d models, got %d", tt.cli, tt.minCount, len(models))
		}
		for _, expected := range tt.contains {
			if !contains(models, expected) {
				t.Errorf("builtInModelList(%q): expected to contain %q", tt.cli, expected)
			}
		}
	}
}

func TestContains(t *testing.T) {
	items := []string{"a", "b", "c"}

	if !contains(items, "a") {
		t.Error("expected contains to return true for 'a'")
	}
	if !contains(items, "c") {
		t.Error("expected contains to return true for 'c'")
	}
	if contains(items, "d") {
		t.Error("expected contains to return false for 'd'")
	}
	if contains(nil, "a") {
		t.Error("expected contains to return false for nil slice")
	}
	if contains([]string{}, "a") {
		t.Error("expected contains to return false for empty slice")
	}
}

func TestUniqueStrings(t *testing.T) {
	tests := []struct {
		input    []string
		expected []string
	}{
		{nil, []string{}},
		{[]string{}, []string{}},
		{[]string{"a"}, []string{"a"}},
		{[]string{"a", "b", "a"}, []string{"a", "b"}},
		{[]string{"a", "a", "a"}, []string{"a"}},
		{[]string{" a ", "a", " a"}, []string{"a"}},
		{[]string{"", "a", "", "b"}, []string{"a", "b"}},
	}

	for _, tt := range tests {
		got := uniqueStrings(tt.input)
		if len(got) != len(tt.expected) {
			t.Errorf("uniqueStrings(%v): expected %v, got %v", tt.input, tt.expected, got)
			continue
		}
		for i := range got {
			if got[i] != tt.expected[i] {
				t.Errorf("uniqueStrings(%v)[%d]: expected %q, got %q", tt.input, i, tt.expected[i], got[i])
			}
		}
	}
}

func TestLooksLikeModel(t *testing.T) {
	tests := []struct {
		token    string
		expected bool
	}{
		// Valid models
		{"gpt-4", true},
		{"gpt-4o", true},
		{"gpt-5.2-codex", true},
		{"claude-3-opus", true},
		{"gemini-2.5-pro", true},
		{"o3-mini", true},
		{"claude-opus-4-5-20250514", true},

		// Invalid - too short
		{"gpt", false},
		{"a-1", false},

		// Invalid - no digit
		{"claude-opus", false},
		{"gpt-turbo", false},

		// Invalid - no dash
		{"gpt4", false},
		{"claude3", false},

		// Invalid - special characters
		{"gpt-4@", false},
		{"model#1", false},
	}

	for _, tt := range tests {
		got := looksLikeModel(tt.token)
		if got != tt.expected {
			t.Errorf("looksLikeModel(%q): expected %v, got %v", tt.token, tt.expected, got)
		}
	}
}

func TestParseModelOutput(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected []string
	}{
		{
			name:     "empty",
			input:    "",
			expected: nil,
		},
		{
			name:     "json array",
			input:    `["gpt-4", "gpt-3.5-turbo"]`,
			expected: []string{"gpt-4", "gpt-3.5-turbo"},
		},
		{
			name:     "json object with models",
			input:    `{"models": ["gpt-4", "gpt-3.5-turbo"]}`,
			expected: []string{"gpt-4", "gpt-3.5-turbo"},
		},
		{
			name:     "json object with data",
			input:    `{"data": [{"id": "gpt-4"}, {"id": "gpt-3.5-turbo"}]}`,
			expected: []string{"gpt-4", "gpt-3.5-turbo"},
		},
		{
			name:     "json object with name field",
			input:    `{"models": [{"name": "claude-3-opus"}]}`,
			expected: []string{"claude-3-opus"},
		},
		{
			name:     "plain text lines",
			input:    "gpt-4\ngpt-3.5-turbo\nclaude-3-opus",
			expected: []string{"gpt-4", "gpt-3.5-turbo", "claude-3-opus"},
		},
		{
			name:     "plain text with extra columns",
			input:    "gpt-4  available\ngpt-3.5-turbo  deprecated",
			expected: []string{"gpt-4", "gpt-3.5-turbo"},
		},
		{
			name:     "duplicates removed",
			input:    `["gpt-4", "gpt-4", "gpt-3.5-turbo"]`,
			expected: []string{"gpt-4", "gpt-3.5-turbo"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseModelOutput(tt.input)
			if tt.expected == nil {
				if got != nil {
					t.Errorf("expected nil, got %v", got)
				}
				return
			}
			if len(got) != len(tt.expected) {
				t.Errorf("expected %v, got %v", tt.expected, got)
				return
			}
			for i := range got {
				if got[i] != tt.expected[i] {
					t.Errorf("index %d: expected %q, got %q", i, tt.expected[i], got[i])
				}
			}
		})
	}
}

func TestListModelsForCLI(t *testing.T) {
	// Test built-in fallback
	models, source := listModelsForCLI("codex", false)
	if len(models) == 0 {
		t.Error("expected non-empty models for codex")
	}
	if source != "built-in" && source != "config+codex" {
		t.Errorf("expected source 'built-in' or 'config+codex', got %q", source)
	}

	models, source = listModelsForCLI("claude", false)
	if len(models) == 0 {
		t.Error("expected non-empty models for claude")
	}

	models, source = listModelsForCLI("gemini", false)
	if len(models) == 0 {
		t.Error("expected non-empty models for gemini")
	}

	// Test unknown CLI
	models, source = listModelsForCLI("unknown", false)
	if len(models) != 0 {
		t.Error("expected empty models for unknown CLI")
	}
	if source != "none" {
		t.Errorf("expected source 'none', got %q", source)
	}
}

func TestGetModelFromCLIConfig(t *testing.T) {
	// Test codex config parsing
	tmpDir := t.TempDir()
	codexDir := filepath.Join(tmpDir, ".codex")
	if err := os.MkdirAll(codexDir, 0755); err != nil {
		t.Fatal(err)
	}

	configContent := `model = "gpt-5.2-codex"
model_reasoning_effort = "high"
other_setting = true
`
	if err := os.WriteFile(filepath.Join(codexDir, "config.toml"), []byte(configContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Can't easily test without mocking os.UserHomeDir, but we can test claude/gemini return empty
	if model := getModelFromCLIConfig("claude"); model != "" {
		t.Errorf("expected empty model for claude, got %q", model)
	}
	if model := getModelFromCLIConfig("gemini"); model != "" {
		t.Errorf("expected empty model for gemini, got %q", model)
	}
	if model := getModelFromCLIConfig("unknown"); model != "" {
		t.Errorf("expected empty model for unknown, got %q", model)
	}
}
