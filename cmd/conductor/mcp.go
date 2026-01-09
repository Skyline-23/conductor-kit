package main

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

type BatchInput struct {
	Prompt    string `json:"prompt"`
	Roles     string `json:"roles,omitempty"`
	Agents    string `json:"agents,omitempty"`
	Model     string `json:"model,omitempty"`
	Reasoning string `json:"reasoning,omitempty"`
	Config    string `json:"config,omitempty"`
	TimeoutMs int    `json:"timeout_ms,omitempty"`
}

type RunInput struct {
	Prompt    string `json:"prompt"`
	Role      string `json:"role,omitempty"`
	Agent     string `json:"agent,omitempty"`
	Model     string `json:"model,omitempty"`
	Reasoning string `json:"reasoning,omitempty"`
	Config    string `json:"config,omitempty"`
	TimeoutMs int    `json:"timeout_ms,omitempty"`
}

type StatusInput struct {
	RunID string `json:"run_id"`
	Tail  int    `json:"tail,omitempty"`
}

type WaitInput struct {
	RunID     string `json:"run_id"`
	TimeoutMs int    `json:"timeout_ms,omitempty"`
	Tail      int    `json:"tail,omitempty"`
}

type CancelInput struct {
	RunID string `json:"run_id"`
	Force bool   `json:"force,omitempty"`
}

type HistoryInput struct {
	Limit  int    `json:"limit,omitempty"`
	Status string `json:"status,omitempty"`
	Role   string `json:"role,omitempty"`
	Agent  string `json:"agent,omitempty"`
}

type InfoInput struct {
	RunID string `json:"run_id"`
}

func runMCP(args []string) int {
	_ = args
	server := mcp.NewServer(&mcp.Implementation{
		Name:    "conductor-kit",
		Version: "0.1.0",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run",
		Description: "Run a single role/agent synchronously and return output.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input RunInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := runTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_batch",
		Description: "Run multiple roles/agents in parallel and return outputs.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input BatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := runBatchTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_async",
		Description: "Run a single role/agent asynchronously and return run_id.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input RunInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := runAsyncTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_batch_async",
		Description: "Run multiple roles/agents asynchronously and return run_ids.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input BatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := runBatchAsyncTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_status",
		Description: "Get status and output tail for an async run.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input StatusInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if input.RunID == "" {
			return nil, nil, errors.New("Missing run_id")
		}
		tail := input.Tail
		if tail <= 0 {
			tail = 4000
		}
		payload, err := getRunStatus(input.RunID, tail)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_wait",
		Description: "Block until an async run completes or timeout is reached.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input WaitInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if input.RunID == "" {
			return nil, nil, errors.New("Missing run_id")
		}
		tail := input.Tail
		if tail <= 0 {
			tail = 4000
		}
		timeout := time.Duration(input.TimeoutMs) * time.Millisecond
		if timeout <= 0 {
			timeout = time.Duration(defaultTimeoutMs) * time.Millisecond
		}
		payload, err := waitRun(input.RunID, timeout, tail)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_cancel",
		Description: "Cancel an async run.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input CancelInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if input.RunID == "" {
			return nil, nil, errors.New("Missing run_id")
		}
		payload, err := cancelRun(input.RunID, input.Force)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_history",
		Description: "List recent run records.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input HistoryInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		records, err := readRunHistory(input.Limit, input.Status, input.Role, input.Agent)
		if err != nil {
			return nil, nil, err
		}
		return nil, map[string]interface{}{"count": len(records), "runs": records}, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_info",
		Description: "Get a single run record by run_id.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input InfoInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if input.RunID == "" {
			return nil, nil, errors.New("Missing run_id")
		}
		record, ok, err := findRunRecord(input.RunID)
		if err != nil {
			return nil, nil, err
		}
		return nil, map[string]interface{}{"found": ok, "run": record}, nil
	})

	transport := mcp.NewStdioTransport()
	if _, err := server.Connect(context.Background(), transport, nil); err != nil {
		fmt.Println(err.Error())
		return 1
	}
	return 0
}

func runBatchTool(input BatchInput) (map[string]interface{}, error) {
	return runBatch(input.Prompt, input.Roles, input.Agents, input.Config, input.Model, input.Reasoning, input.TimeoutMs)
}

func runBatchAsyncTool(input BatchInput) (map[string]interface{}, error) {
	return runBatchAsync(input.Prompt, input.Roles, input.Agents, input.Config, input.Model, input.Reasoning, input.TimeoutMs)
}

func runTool(input RunInput) (map[string]interface{}, error) {
	if input.Prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if input.Role == "" && input.Agent == "" {
		return nil, errors.New("Missing role or agent")
	}
	configPath := resolveConfigPath(input.Config)

	var cfg Config
	var err error
	if input.Role != "" {
		cfg, err = loadConfig(configPath)
		if err != nil {
			return nil, err
		}
	} else {
		cfg, err = loadConfigOrEmpty(configPath)
		if err != nil {
			return nil, err
		}
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt

	var spec CmdSpec
	if input.Role != "" {
		spec, err = buildSpecFromRole(cfg, input.Role, input.Prompt, input.Model, input.Reasoning, logPrompt)
	} else {
		spec, err = buildSpecFromAgent(input.Agent, input.Prompt, defaults, logPrompt)
	}
	if err != nil {
		return nil, err
	}
	if input.TimeoutMs > 0 {
		spec.TimeoutMs = input.TimeoutMs
	}
	return runCommand(spec)
}

func runAsyncTool(input RunInput) (map[string]interface{}, error) {
	if input.Prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if input.Role == "" && input.Agent == "" {
		return nil, errors.New("Missing role or agent")
	}
	configPath := resolveConfigPath(input.Config)

	var cfg Config
	var err error
	if input.Role != "" {
		cfg, err = loadConfig(configPath)
		if err != nil {
			return nil, err
		}
	} else {
		cfg, err = loadConfigOrEmpty(configPath)
		if err != nil {
			return nil, err
		}
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt

	var spec CmdSpec
	if input.Role != "" {
		spec, err = buildSpecFromRole(cfg, input.Role, input.Prompt, input.Model, input.Reasoning, logPrompt)
	} else {
		spec, err = buildSpecFromAgent(input.Agent, input.Prompt, defaults, logPrompt)
	}
	if err != nil {
		return nil, err
	}
	if input.TimeoutMs > 0 {
		spec.TimeoutMs = input.TimeoutMs
	}
	return startAsync(spec)
}
