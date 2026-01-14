package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

var codexAdapter = &CLIAdapter{Name: "Codex", Cmd: "codex"}

type CodexPromptInput struct {
	Prompt        string `json:"prompt"`
	Model         string `json:"model,omitempty"`
	Reasoning     string `json:"reasoning,omitempty"`
	Config        string `json:"config,omitempty"`
	Profile       string `json:"profile,omitempty"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
}

type CodexBatchInput struct {
	Prompt        string `json:"prompt"`
	Models        string `json:"models,omitempty"`
	Reasoning     string `json:"reasoning,omitempty"`
	Config        string `json:"config,omitempty"`
	Profile       string `json:"profile,omitempty"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
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
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
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
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
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
	args := buildCodexArgs(input.Prompt, input.Model, input.Reasoning, input.Config, input.Profile)
	output, err := codexAdapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: input.IdleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}
	parsed := parseCodexOutput(output)
	return map[string]interface{}{"model": input.Model, "reasoning": input.Reasoning, "events": parsed, "raw": output}, nil
}

func runCodexBatch(ctx context.Context, input CodexBatchInput) (map[string]interface{}, error) {
	models := SplitModels(input.Models)
	if len(models) == 0 {
		models = []string{""}
	}
	responses := make([]map[string]interface{}, 0, len(models))
	for _, model := range models {
		args := buildCodexArgs(input.Prompt, model, input.Reasoning, input.Config, input.Profile)
		output, err := codexAdapter.Run(ctx, CLIRunOptions{
			Args:          args,
			IdleTimeoutMs: input.IdleTimeoutMs,
		})
		if err != nil {
			return nil, err
		}
		responses = append(responses, map[string]interface{}{
			"model":     model,
			"reasoning": input.Reasoning,
			"events":    parseCodexOutput(output),
			"raw":       output,
		})
	}
	return map[string]interface{}{"count": len(responses), "responses": responses}, nil
}

func buildCodexArgs(prompt, model, reasoning, config, profile string) []string {
	args := []string{"exec", "--json"}
	if reasoning != "" {
		args = append(args, "-c", fmt.Sprintf("model_reasoning_effort=\"%s\"", reasoning))
	}
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
	return args
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
