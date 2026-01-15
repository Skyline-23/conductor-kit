package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestGetenv(t *testing.T) {
	// Test with existing env var
	os.Setenv("TEST_GETENV_VAR", "test_value")
	defer os.Unsetenv("TEST_GETENV_VAR")

	if got := getenv("TEST_GETENV_VAR", "fallback"); got != "test_value" {
		t.Errorf("expected test_value, got %s", got)
	}

	// Test with non-existing env var
	if got := getenv("NON_EXISTING_VAR_12345", "fallback"); got != "fallback" {
		t.Errorf("expected fallback, got %s", got)
	}
}

func TestSplitList(t *testing.T) {
	tests := []struct {
		input    string
		expected []string
	}{
		{"", []string{}},
		{"a", []string{"a"}},
		{"a,b,c", []string{"a", "b", "c"}},
		{"a, b, c", []string{"a", "b", "c"}},
		{" a , b , c ", []string{"a", "b", "c"}},
		{"a,,b", []string{"a", "b"}},
		{",a,b,", []string{"a", "b"}},
	}

	for _, tt := range tests {
		got := splitList(tt.input)
		if len(got) != len(tt.expected) {
			t.Errorf("splitList(%q): expected %v, got %v", tt.input, tt.expected, got)
			continue
		}
		for i := range got {
			if got[i] != tt.expected[i] {
				t.Errorf("splitList(%q)[%d]: expected %q, got %q", tt.input, i, tt.expected[i], got[i])
			}
		}
	}
}

func TestExpandPath(t *testing.T) {
	home := os.Getenv("HOME")

	tests := []struct {
		input    string
		expected string
	}{
		{"", ""},
		{"~/test", filepath.Join(home, "test")},
		{"~/.config", filepath.Join(home, ".config")},
		{"/absolute/path", "/absolute/path"},
		{"relative/path", "relative/path"},
	}

	for _, tt := range tests {
		got := expandPath(tt.input)
		if got != tt.expected {
			t.Errorf("expandPath(%q): expected %q, got %q", tt.input, tt.expected, got)
		}
	}
}

func TestPathExists(t *testing.T) {
	// Test existing path
	tmpDir := t.TempDir()
	if !pathExists(tmpDir) {
		t.Errorf("expected pathExists(%q) to be true", tmpDir)
	}

	// Test existing file
	tmpFile := filepath.Join(tmpDir, "test.txt")
	if err := os.WriteFile(tmpFile, []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}
	if !pathExists(tmpFile) {
		t.Errorf("expected pathExists(%q) to be true", tmpFile)
	}

	// Test non-existing path
	if pathExists("/non/existing/path/12345") {
		t.Error("expected pathExists for non-existing path to be false")
	}
}

func TestResolveConfigPath(t *testing.T) {
	// Test explicit path
	if got := resolveConfigPath("/explicit/path.json"); got != "/explicit/path.json" {
		t.Errorf("expected /explicit/path.json, got %s", got)
	}

	// Test CONDUCTOR_CONFIG env var
	os.Setenv("CONDUCTOR_CONFIG", "/env/config.json")
	defer os.Unsetenv("CONDUCTOR_CONFIG")
	if got := resolveConfigPath(""); got != "/env/config.json" {
		t.Errorf("expected /env/config.json, got %s", got)
	}
}

func TestDefaultOpenCodeHome(t *testing.T) {
	// Test with OPENCODE_CONFIG_DIR
	os.Setenv("OPENCODE_CONFIG_DIR", "/custom/opencode")
	got := defaultOpenCodeHome()
	os.Unsetenv("OPENCODE_CONFIG_DIR")
	if got != "/custom/opencode" {
		t.Errorf("expected /custom/opencode, got %s", got)
	}

	// Test with XDG_CONFIG_HOME
	os.Setenv("XDG_CONFIG_HOME", "/custom/xdg")
	got = defaultOpenCodeHome()
	os.Unsetenv("XDG_CONFIG_HOME")
	if got != "/custom/xdg/opencode" {
		t.Errorf("expected /custom/xdg/opencode, got %s", got)
	}
}

func TestConductorBinAliases(t *testing.T) {
	aliases := conductorBinAliases()
	if len(aliases) == 0 {
		t.Error("expected non-empty aliases")
	}

	// Check for essential aliases
	expected := []string{"conductor", "conductor-kit"}
	for _, e := range expected {
		found := false
		for _, a := range aliases {
			if a == e {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("expected alias %q not found", e)
		}
	}
}
