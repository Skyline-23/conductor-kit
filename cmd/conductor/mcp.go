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
}

type TaskInput struct {
	Prompt    string `json:"prompt"`
	Role      string `json:"role,omitempty"`
	Agent     string `json:"agent,omitempty"`
	Model     string `json:"model,omitempty"`
	Reasoning string `json:"reasoning,omitempty"`
	Config    string `json:"config,omitempty"`
}

type OutputInput struct {
	TaskID  string `json:"task_id"`
	Block   bool   `json:"block,omitempty"`
	Timeout int    `json:"timeout,omitempty"`
	Tail    int    `json:"tail,omitempty"`
}

type CancelInput struct {
	TaskID string `json:"task_id,omitempty"`
	All    bool   `json:"all,omitempty"`
	Force  bool   `json:"force,omitempty"`
}

func runMCP(args []string) int {
	_ = args
	server := mcp.NewServer(&mcp.Implementation{
		Name:    "conductor-kit",
		Version: "0.1.0",
	}, nil)

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.background_task",
		Description: "Run a single role/agent CLI in background and return a task ID.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input TaskInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := runTaskTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.background_batch",
		Description: "Run role/agent background batch for multiple CLIs and return task IDs.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input BatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := runBatchTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.background_output",
		Description: "Fetch output for a background task.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input OutputInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := getTaskOutput(input.TaskID, input.Block, time.Duration(input.Timeout)*time.Millisecond, input.Tail)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.background_cancel",
		Description: "Cancel background tasks.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input CancelInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload := map[string]interface{}{}
		if input.All {
			payload["cancelled"] = cancelAllTasks(input.Force)
		} else if input.TaskID != "" {
			payload["cancelled"] = []map[string]interface{}{cancelTask(input.TaskID, input.Force)}
		} else {
			return nil, nil, errors.New("Missing task_id or all")
		}
		return nil, payload, nil
	})

	transport := mcp.NewStdioTransport()
	if err := server.Connect(context.Background(), transport); err != nil {
		fmt.Println(err.Error())
		return 1
	}
	return 0
}

func runBatchTool(input BatchInput) (map[string]interface{}, error) {
	args := []string{"--prompt", input.Prompt}
	if input.Roles != "" {
		args = append(args, "--roles", input.Roles)
	}
	if input.Agents != "" {
		args = append(args, "--agents", input.Agents)
	}
	if input.Model != "" {
		args = append(args, "--model", input.Model)
	}
	if input.Reasoning != "" {
		args = append(args, "--reasoning", input.Reasoning)
	}
	if input.Config != "" {
		args = append(args, "--config", input.Config)
	}

	return runBatchArgs(args)
}

func runTaskTool(input TaskInput) (map[string]interface{}, error) {
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
	return startTask(spec, input.Prompt)
}
