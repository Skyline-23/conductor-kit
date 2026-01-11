package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

type CodexPromptInput struct {
	Prompt    string `json:"prompt"`
	Model     string `json:"model,omitempty"`
	Config    string `json:"config,omitempty"`
	Profile   string `json:"profile,omitempty"`
	TimeoutMs int    `json:"timeout_ms,omitempty"`
}

type CodexBatchInput struct {
	Prompt    string `json:"prompt"`
	Models    string `json:"models,omitempty"`
	Config    string `json:"config,omitempty"`
	Profile   string `json:"profile,omitempty"`
	TimeoutMs int    `json:"timeout_ms,omitempty"`
}

type CodexAuthInput struct{}

func runCodexMCP(args []string) int {
	_ = args
	server := mcp.NewServer(&mcp.Implementation{
		Name:    "conductor-codex-cli",
		Version: "0.1.0",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "codex.prompt",
		Description: "Run a single Codex CLI prompt and return the parsed output.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input CodexPromptInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if strings.TrimSpace(input.Prompt) == "" {
			return nil, nil, errors.New("missing prompt")
		}
		payload, err := runCodexPrompt(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "codex.batch",
		Description: "Run a Codex CLI prompt for multiple models.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input CodexBatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if strings.TrimSpace(input.Prompt) == "" {
			return nil, nil, errors.New("missing prompt")
		}
		payload, err := runCodexBatch(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "codex.auth_status",
		Description: "Check Codex CLI availability.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input CodexAuthInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if isCommandAvailable("codex") {
			return nil, map[string]interface{}{"status": "available"}, nil
		}
		return nil, map[string]interface{}{"status": "missing"}, nil
	})

	transport := mcp.NewStdioTransport()
	session, err := server.Connect(context.Background(), transport, nil)
	if err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		return 1
	}
	if err := session.Wait(); err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		return 1
	}
	return 0
}

func runCodexPrompt(ctx context.Context, input CodexPromptInput) (map[string]interface{}, error) {
	output, err := runCodexCLI(ctx, input.Prompt, input.Model, input.Config, input.Profile, input.TimeoutMs)
	if err != nil {
		return nil, err
	}
	parsed := parseCodexOutput(output)
	return map[string]interface{}{"model": input.Model, "events": parsed, "raw": output}, nil
}

func runCodexBatch(ctx context.Context, input CodexBatchInput) (map[string]interface{}, error) {
	models := splitCodexModels(input.Models)
	if len(models) == 0 {
		models = []string{""}
	}
	responses := make([]map[string]interface{}, 0, len(models))
	for _, model := range models {
		output, err := runCodexCLI(ctx, input.Prompt, model, input.Config, input.Profile, input.TimeoutMs)
		if err != nil {
			return nil, err
		}
		responses = append(responses, map[string]interface{}{
			"model":  model,
			"events": parseCodexOutput(output),
			"raw":    output,
		})
	}
	return map[string]interface{}{"count": len(responses), "responses": responses}, nil
}

func splitCodexModels(models string) []string {
	parts := strings.Split(models, ",")
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		trimmed := strings.TrimSpace(part)
		if trimmed == "" {
			continue
		}
		out = append(out, trimmed)
	}
	return out
}

func runCodexCLI(ctx context.Context, prompt, model, config, profile string, timeoutMs int) (string, error) {
	if !isCommandAvailable("codex") {
		return "", errors.New("codex CLI not found")
	}
	if strings.TrimSpace(prompt) == "" {
		return "", errors.New("prompt is required")
	}
	args := []string{"exec", "--json"}
	if config != "" {
		args = append(args, "-c", config)
	}
	if profile != "" {
		args = append(args, "-p", profile)
	}
	if model != "" {
		args = append(args, "-m", model)
	}
	args = append(args, prompt)
	ctx, cancel := withCodexTimeout(ctx, timeoutMs)
	defer cancel()

	cmd := exec.CommandContext(ctx, "codex", args...)
	output, err := cmd.CombinedOutput()
	if ctx.Err() == context.DeadlineExceeded {
		return "", errors.New("codex CLI timed out")
	}
	if err != nil {
		return "", fmt.Errorf("codex CLI failed: %w", err)
	}
	return strings.TrimSpace(string(output)), nil
}

func withCodexTimeout(ctx context.Context, timeoutMs int) (context.Context, context.CancelFunc) {
	timeout := time.Duration(effectiveMcpTimeoutMs(timeoutMs)) * time.Millisecond
	if timeout <= 0 {
		return context.WithCancel(ctx)
	}
	return context.WithTimeout(ctx, timeout)
}

func parseCodexOutput(output string) []interface{} {
	if output == "" {
		return nil
	}
	lines := strings.Split(output, "\n")
	parsed := make([]interface{}, 0, len(lines))
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var payload interface{}
		if err := json.Unmarshal([]byte(line), &payload); err == nil {
			parsed = append(parsed, payload)
			continue
		}
		parsed = append(parsed, line)
	}
	return parsed
}
