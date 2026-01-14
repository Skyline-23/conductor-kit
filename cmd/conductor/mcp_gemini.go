package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

var geminiAdapter = &CLIAdapter{Name: "Gemini", Cmd: "gemini"}

type GeminiPromptInput struct {
	Prompt        string `json:"prompt"`
	Model         string `json:"model,omitempty"`
	OutputFormat  string `json:"output_format,omitempty"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
}

type GeminiBatchInput struct {
	Prompt        string `json:"prompt"`
	Models        string `json:"models,omitempty"`
	OutputFormat  string `json:"output_format,omitempty"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
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
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
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
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
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
		if isCommandAvailable("gemini") {
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

func runGeminiPrompt(ctx context.Context, input GeminiPromptInput) (map[string]interface{}, error) {
	outputFormat := normalizeGeminiFormat(input.OutputFormat)
	args := buildGeminiArgs(input.Prompt, input.Model, outputFormat)
	output, err := geminiAdapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: input.IdleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}
	parsed := parseGeminiOutput(output)
	return map[string]interface{}{"model": input.Model, "output": parsed, "raw": output}, nil
}

func runGeminiBatch(ctx context.Context, input GeminiBatchInput) (map[string]interface{}, error) {
	outputFormat := normalizeGeminiFormat(input.OutputFormat)
	models := SplitModels(input.Models)
	if len(models) == 0 {
		models = []string{""}
	}
	responses := make([]map[string]interface{}, 0, len(models))
	for _, model := range models {
		args := buildGeminiArgs(input.Prompt, model, outputFormat)
		output, err := geminiAdapter.Run(ctx, CLIRunOptions{
			Args:          args,
			IdleTimeoutMs: input.IdleTimeoutMs,
		})
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

func buildGeminiArgs(prompt, model, outputFormat string) []string {
	args := []string{"-p", prompt, "--output-format", outputFormat}
	if model != "" {
		args = append(args, "-m", model)
	}
	return args
}

func normalizeGeminiFormat(format string) string {
	format = strings.TrimSpace(format)
	if format == "" {
		return "stream-json"
	}
	return format
}

func parseGeminiOutput(output string) interface{} {
	if output == "" {
		return ""
	}
	// For stream-json format, parse JSONL and extract relevant events
	lines := strings.Split(output, "\n")
	events := make([]interface{}, 0, len(lines))
	var lastResult interface{}
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var event map[string]interface{}
		if err := json.Unmarshal([]byte(line), &event); err == nil {
			events = append(events, event)
			// Capture the final result event
			if eventType, ok := event["type"].(string); ok && eventType == "result" {
				lastResult = event
			}
		}
	}
	// Return structured output with events and final result
	if lastResult != nil {
		return map[string]interface{}{
			"events": events,
			"result": lastResult,
		}
	}
	// Fallback: try parsing as single JSON object (for json format)
	var parsed interface{}
	if err := json.Unmarshal([]byte(output), &parsed); err == nil {
		return parsed
	}
	return output
}
