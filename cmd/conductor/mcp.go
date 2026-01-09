package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
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

func runTool(input RunInput) (map[string]interface{}, error) {
	if input.Prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if input.Role == "" && input.Agent == "" {
		return nil, errors.New("Missing role or agent")
	}
	configPath := input.Config
	if configPath == "" {
		configPath = getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json"))
	}

	var spec CmdSpec
	var err error
	if input.Role != "" {
		spec, err = buildSpecFromRole(configPath, input.Role, input.Prompt, input.Model, input.Reasoning)
	} else {
		spec, err = buildSpecFromAgent(input.Agent, input.Prompt)
	}
	if err != nil {
		return nil, err
	}

	timeout := time.Duration(input.TimeoutMs) * time.Millisecond
	return runCommand(spec, timeout)
}
