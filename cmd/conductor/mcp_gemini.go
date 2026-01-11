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

type GeminiPromptInput struct {
	Prompt       string `json:"prompt"`
	Model        string `json:"model,omitempty"`
	OutputFormat string `json:"output_format,omitempty"`
	TimeoutMs    int    `json:"timeout_ms,omitempty"`
}

type GeminiBatchInput struct {
	Prompt       string `json:"prompt"`
	Models       string `json:"models,omitempty"`
	OutputFormat string `json:"output_format,omitempty"`
	TimeoutMs    int    `json:"timeout_ms,omitempty"`
}

type GeminiAuthInput struct{}

func runGeminiMCP(args []string) int {
	_ = args
	server := mcp.NewServer(&mcp.Implementation{
		Name:    "conductor-gemini-cli",
		Version: "0.1.0",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "gemini.prompt",
		Description: "Run a single Gemini CLI prompt and return the parsed output.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input GeminiPromptInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if strings.TrimSpace(input.Prompt) == "" {
			return nil, nil, errors.New("missing prompt")
		}
		payload, err := runGeminiPrompt(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "gemini.batch",
		Description: "Run a Gemini CLI prompt for multiple models.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input GeminiBatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if strings.TrimSpace(input.Prompt) == "" {
			return nil, nil, errors.New("missing prompt")
		}
		payload, err := runGeminiBatch(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "gemini.auth_status",
		Description: "Check Gemini CLI auth readiness.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input GeminiAuthInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		status, detail := checkGeminiAuth()
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

func runGeminiPrompt(ctx context.Context, input GeminiPromptInput) (map[string]interface{}, error) {
	outputFormat := normalizeGeminiFormat(input.OutputFormat)
	output, err := runGeminiCLI(ctx, input.Prompt, input.Model, outputFormat, input.TimeoutMs)
	if err != nil {
		return nil, err
	}
	parsed := parseGeminiOutput(output)
	return map[string]interface{}{"model": input.Model, "output": parsed, "raw": output}, nil
}

func runGeminiBatch(ctx context.Context, input GeminiBatchInput) (map[string]interface{}, error) {
	outputFormat := normalizeGeminiFormat(input.OutputFormat)
	models := splitGeminiModels(input.Models)
	if len(models) == 0 {
		models = []string{""}
	}
	responses := make([]map[string]interface{}, 0, len(models))
	for _, model := range models {
		output, err := runGeminiCLI(ctx, input.Prompt, model, outputFormat, input.TimeoutMs)
		if err != nil {
			return nil, err
		}
		responses = append(responses, map[string]interface{}{
			"model":  model,
			"output": parseGeminiOutput(output),
			"raw":    output,
		})
	}
	return map[string]interface{}{"count": len(responses), "responses": responses}, nil
}

func normalizeGeminiFormat(format string) string {
	format = strings.TrimSpace(format)
	if format == "" {
		return "json"
	}
	return format
}

func splitGeminiModels(models string) []string {
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

func runGeminiCLI(ctx context.Context, prompt, model, outputFormat string, timeoutMs int) (string, error) {
	if !isCommandAvailable("gemini") {
		return "", errors.New("gemini CLI not found")
	}
	if strings.TrimSpace(prompt) == "" {
		return "", errors.New("prompt is required")
	}
	args := []string{"-p", prompt, "--output-format", outputFormat}
	if model != "" {
		args = append(args, "-m", model)
	}
	ctx, cancel := withGeminiTimeout(ctx, timeoutMs)
	defer cancel()

	cmd := exec.CommandContext(ctx, "gemini", args...)
	output, err := cmd.CombinedOutput()
	if ctx.Err() == context.DeadlineExceeded {
		return "", errors.New("gemini CLI timed out")
	}
	if err != nil {
		return "", fmt.Errorf("gemini CLI failed: %w", err)
	}
	return strings.TrimSpace(string(output)), nil
}

func withGeminiTimeout(ctx context.Context, timeoutMs int) (context.Context, context.CancelFunc) {
	timeout := time.Duration(effectiveMcpTimeoutMs(timeoutMs)) * time.Millisecond
	if timeout <= 0 {
		return context.WithCancel(ctx)
	}
	return context.WithTimeout(ctx, timeout)
}

func parseGeminiOutput(output string) interface{} {
	if output == "" {
		return ""
	}
	var parsed interface{}
	if err := json.Unmarshal([]byte(output), &parsed); err == nil {
		return parsed
	}
	return output
}
