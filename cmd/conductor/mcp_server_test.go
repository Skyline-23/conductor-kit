package main

import (
	"testing"
)

func TestValidatePrompt(t *testing.T) {
	tests := []struct {
		prompt  string
		wantErr bool
	}{
		{"hello", false},
		{"", true},
		{"   ", true},
		{"\t\n", true},
		{"valid prompt", false},
		{" trimmed ", false},
	}

	for _, tt := range tests {
		err := ValidatePrompt(tt.prompt)
		if tt.wantErr && err == nil {
			t.Errorf("ValidatePrompt(%q): expected error, got nil", tt.prompt)
		}
		if !tt.wantErr && err != nil {
			t.Errorf("ValidatePrompt(%q): unexpected error: %v", tt.prompt, err)
		}
	}
}

func TestMcpBuildCodexArgs(t *testing.T) {
	input := MCPCodexInput{
		Prompt:         "test prompt",
		ApprovalPolicy: "never",
		Sandbox:        "workspace-write",
		Model:          "o3",
		Profile:        "test",
	}

	args := mcpBuildCodexArgs(input)

	// Check that exec is present and prompt is at the end
	foundExec := false
	foundPrompt := false
	for _, arg := range args {
		if arg == "exec" {
			foundExec = true
		}
		if arg == "test prompt" {
			foundPrompt = true
		}
	}

	if !foundExec {
		t.Error("expected 'exec' in args")
	}
	if !foundPrompt {
		t.Error("expected prompt in args")
	}

	// Check last arg is the prompt
	if len(args) > 0 && args[len(args)-1] != "test prompt" {
		t.Errorf("expected last arg to be prompt, got %q", args[len(args)-1])
	}
}

func TestMcpBuildClaudeArgs(t *testing.T) {
	input := MCPClaudeInput{
		Prompt:         "test prompt",
		Model:          "sonnet",
		PermissionMode: "default",
		MaxTurns:       5,
	}

	args := mcpBuildClaudeArgs(input)

	// Check that -p and prompt are present
	foundPrint := false
	foundPrompt := false
	for i, arg := range args {
		if arg == "-p" || arg == "--print" {
			foundPrint = true
		}
		if i > 0 && args[i-1] != "--" && arg == "test prompt" {
			foundPrompt = true
		}
	}

	if !foundPrint {
		t.Error("expected '-p' or '--print' in args")
	}
	if !foundPrompt {
		t.Error("expected prompt in args")
	}
}

func TestMcpBuildGeminiArgs(t *testing.T) {
	input := MCPGeminiInput{
		Prompt: "test prompt",
		Model:  "gemini-2.5-pro",
		Yolo:   true,
	}

	args := mcpBuildGeminiArgs(input)

	// Check that prompt is present and yolo flag
	foundPrompt := false
	foundYolo := false
	for _, arg := range args {
		if arg == "test prompt" {
			foundPrompt = true
		}
		if arg == "-y" || arg == "--yolo" {
			foundYolo = true
		}
	}

	if !foundPrompt {
		t.Error("expected prompt in args")
	}
	if !foundYolo {
		t.Error("expected '-y' or '--yolo' in args")
	}
}

func TestMcpGetAdapter(t *testing.T) {
	tests := []struct {
		cli  string
		want bool
	}{
		{"codex", true},
		{"claude", true},
		{"gemini", true},
		{"unknown", false},
		{"", false},
	}

	for _, tt := range tests {
		adapter := mcpGetAdapter(tt.cli)
		if tt.want && adapter == nil {
			t.Errorf("mcpGetAdapter(%q): expected adapter, got nil", tt.cli)
		}
		if !tt.want && adapter != nil {
			t.Errorf("mcpGetAdapter(%q): expected nil, got adapter", tt.cli)
		}
	}
}

func TestMcpBuildContextPrompt(t *testing.T) {
	messages := []MCPMessage{
		{Role: "user", Content: "first question"},
		{Role: "assistant", Content: "first answer"},
		{Role: "user", Content: "second question"},
		{Role: "assistant", Content: "second answer"},
	}

	result := mcpBuildContextPrompt(messages, "new question")

	// Should contain previous messages and new prompt
	if result == "" {
		t.Error("expected non-empty context prompt")
	}
	if len(result) < len("new question") {
		t.Error("context prompt should include new question")
	}
}

func TestMcpBuildResponseWithMeta(t *testing.T) {
	result := mcpBuildResponseWithMeta("output text", "thread-123", "codex", "oracle", "gpt-4")

	// Check structuredContent
	structured, ok := result["structuredContent"].(map[string]interface{})
	if !ok {
		t.Fatal("expected structuredContent to be a map")
	}

	if structured["threadId"] != "thread-123" {
		t.Errorf("expected threadId 'thread-123', got %v", structured["threadId"])
	}
	if structured["cli"] != "codex" {
		t.Errorf("expected cli 'codex', got %v", structured["cli"])
	}
	if structured["role"] != "oracle" {
		t.Errorf("expected role 'oracle', got %v", structured["role"])
	}
	if structured["model"] != "gpt-4" {
		t.Errorf("expected model 'gpt-4', got %v", structured["model"])
	}

	// Check content array
	content, ok := result["content"].([]map[string]interface{})
	if !ok {
		t.Fatal("expected content to be an array of maps")
	}
	if len(content) == 0 {
		t.Fatal("expected non-empty content")
	}
	if content[0]["type"] != "text" {
		t.Errorf("expected content type 'text', got %v", content[0]["type"])
	}
}

func TestMcpCheckCodexAuth(t *testing.T) {
	// Just ensure it doesn't panic
	auth, msg := mcpCheckCodexAuth()
	_ = auth
	_ = msg
}

func TestMcpCheckClaudeAuth(t *testing.T) {
	// Just ensure it doesn't panic
	auth, msg := mcpCheckClaudeAuth()
	_ = auth
	_ = msg
}

func TestMcpCheckGeminiAuth(t *testing.T) {
	// Just ensure it doesn't panic
	auth, msg := mcpCheckGeminiAuth()
	_ = auth
	_ = msg
}

func TestMcpGetStatus(t *testing.T) {
	status := mcpGetStatus()

	// Check structure
	if _, ok := status["cli"]; !ok {
		t.Error("expected 'cli' in status")
	}
	if _, ok := status["sessions"]; !ok {
		t.Error("expected 'sessions' in status")
	}

	clis, ok := status["cli"].(map[string]interface{})
	if !ok {
		t.Fatal("expected 'cli' to be a map")
	}

	for _, name := range []string{"codex", "claude", "gemini"} {
		if _, ok := clis[name]; !ok {
			t.Errorf("expected '%s' in cli status", name)
		}
	}
}
