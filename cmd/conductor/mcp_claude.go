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

type ClaudePromptInput struct {
	Prompt             string `json:"prompt"`
	Model              string `json:"model,omitempty"`
	OutputFormat       string `json:"output_format,omitempty"`
	PermissionMode     string `json:"permission_mode,omitempty"`
	AllowedTools       string `json:"allowed_tools,omitempty"`
	DisallowedTools    string `json:"disallowed_tools,omitempty"`
	Tools              string `json:"tools,omitempty"`
	SystemPrompt       string `json:"system_prompt,omitempty"`
	AppendSystemPrompt string `json:"append_system_prompt,omitempty"`
	TimeoutMs          int    `json:"timeout_ms,omitempty"`
}

type ClaudeBatchInput struct {
	Prompt             string `json:"prompt"`
	Models             string `json:"models,omitempty"`
	OutputFormat       string `json:"output_format,omitempty"`
	PermissionMode     string `json:"permission_mode,omitempty"`
	AllowedTools       string `json:"allowed_tools,omitempty"`
	DisallowedTools    string `json:"disallowed_tools,omitempty"`
	Tools              string `json:"tools,omitempty"`
	SystemPrompt       string `json:"system_prompt,omitempty"`
	AppendSystemPrompt string `json:"append_system_prompt,omitempty"`
	TimeoutMs          int    `json:"timeout_ms,omitempty"`
}

type ClaudeAuthInput struct{}

func runClaudeMCP(args []string) int {
	_ = args
	server := mcp.NewServer(&mcp.Implementation{
		Name:    "conductor-claude-cli",
		Version: "0.1.0",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "claude.prompt",
		Description: "Run a single Claude CLI prompt and return the parsed output.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input ClaudePromptInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if strings.TrimSpace(input.Prompt) == "" {
			return nil, nil, errors.New("missing prompt")
		}
		payload, err := runClaudePrompt(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "claude.batch",
		Description: "Run a Claude CLI prompt for multiple models.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input ClaudeBatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if strings.TrimSpace(input.Prompt) == "" {
			return nil, nil, errors.New("missing prompt")
		}
		payload, err := runClaudeBatch(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "claude.auth_status",
		Description: "Check Claude CLI auth readiness.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input ClaudeAuthInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		status, detail := checkClaudeAuth()
		return nil, map[string]interface{}{"status": status, "detail": detail}, nil
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

func runClaudePrompt(ctx context.Context, input ClaudePromptInput) (map[string]interface{}, error) {
	outputFormat := normalizeClaudeFormat(input.OutputFormat)
	output, err := runClaudeCLI(ctx, input.Prompt, input.Model, outputFormat, input)
	if err != nil {
		return nil, err
	}
	parsed := parseClaudeOutput(output)
	return map[string]interface{}{"model": input.Model, "output": parsed, "raw": output}, nil
}

func runClaudeBatch(ctx context.Context, input ClaudeBatchInput) (map[string]interface{}, error) {
	outputFormat := normalizeClaudeFormat(input.OutputFormat)
	models := splitClaudeModels(input.Models)
	if len(models) == 0 {
		models = []string{""}
	}
	responses := make([]map[string]interface{}, 0, len(models))
	for _, model := range models {
		output, err := runClaudeCLI(ctx, input.Prompt, model, outputFormat, ClaudePromptInput{
			Prompt:             input.Prompt,
			Model:              model,
			OutputFormat:       outputFormat,
			PermissionMode:     input.PermissionMode,
			AllowedTools:       input.AllowedTools,
			DisallowedTools:    input.DisallowedTools,
			Tools:              input.Tools,
			SystemPrompt:       input.SystemPrompt,
			AppendSystemPrompt: input.AppendSystemPrompt,
			TimeoutMs:          input.TimeoutMs,
		})
		if err != nil {
			return nil, err
		}
		responses = append(responses, map[string]interface{}{
			"model":  model,
			"output": parseClaudeOutput(output),
			"raw":    output,
		})
	}
	return map[string]interface{}{"count": len(responses), "responses": responses}, nil
}

func normalizeClaudeFormat(format string) string {
	format = strings.TrimSpace(format)
	if format == "" {
		return "json"
	}
	return format
}

func splitClaudeModels(models string) []string {
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

func runClaudeCLI(ctx context.Context, prompt, model, outputFormat string, input ClaudePromptInput) (string, error) {
	if !isCommandAvailable("claude") {
		return "", errors.New("claude CLI not found")
	}
	if strings.TrimSpace(prompt) == "" {
		return "", errors.New("prompt is required")
	}
	permissionMode := strings.TrimSpace(input.PermissionMode)
	if permissionMode == "" {
		permissionMode = "dontAsk"
	}
	args := []string{"-p", prompt, "--output-format", outputFormat, "--permission-mode", permissionMode}
	if model != "" {
		args = append(args, "--model", model)
	}
	if tools := strings.TrimSpace(input.Tools); tools != "" {
		args = append(args, "--tools", tools)
	}
	if allowedTools := strings.TrimSpace(input.AllowedTools); allowedTools != "" {
		args = append(args, "--allowed-tools", allowedTools)
	}
	if disallowedTools := strings.TrimSpace(input.DisallowedTools); disallowedTools != "" {
		args = append(args, "--disallowed-tools", disallowedTools)
	}
	if systemPrompt := strings.TrimSpace(input.SystemPrompt); systemPrompt != "" {
		args = append(args, "--system-prompt", systemPrompt)
	}
	if appendPrompt := strings.TrimSpace(input.AppendSystemPrompt); appendPrompt != "" {
		args = append(args, "--append-system-prompt", appendPrompt)
	}
	ctx, cancel := withClaudeTimeout(ctx, input.TimeoutMs)
	defer cancel()

	cmd := exec.CommandContext(ctx, "claude", args...)
	output, err := cmd.CombinedOutput()
	if ctx.Err() == context.DeadlineExceeded {
		return "", errors.New("claude CLI timed out")
	}
	if err != nil {
		return "", fmt.Errorf("claude CLI failed: %w", err)
	}
	return strings.TrimSpace(string(output)), nil
}

func withClaudeTimeout(ctx context.Context, timeoutMs int) (context.Context, context.CancelFunc) {
	timeout := time.Duration(effectiveMcpTimeoutMs(timeoutMs)) * time.Millisecond
	if timeout <= 0 {
		return context.WithCancel(ctx)
	}
	return context.WithTimeout(ctx, timeout)
}

func parseClaudeOutput(output string) interface{} {
	if output == "" {
		return ""
	}
	var parsed interface{}
	if err := json.Unmarshal([]byte(output), &parsed); err == nil {
		return parsed
	}
	return output
}
