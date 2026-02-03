package main

import "testing"

func TestBuildRoleArgsKeepsClaudePromptAdjacent(t *testing.T) {
	role := RoleConfig{
		CLI:       "claude",
		Args:      []string{"-p", "{prompt}"},
		ModelFlag: "--model",
	}
	args := buildRoleArgs(role, "hello", "sonnet", "")
	if len(args) < 4 {
		t.Fatalf("expected args with model and prompt, got %v", args)
	}
	if args[0] != "--model" || args[1] != "sonnet" {
		t.Fatalf("expected model flags first, got %v", args)
	}
	if args[2] != "-p" || args[3] != "hello" {
		t.Fatalf("expected -p hello adjacency, got %v", args)
	}
}
