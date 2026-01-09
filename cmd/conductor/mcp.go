package main

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

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
		payload, err := runStatusTool(input)
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
		payload, err := runWaitTool(input)
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
		payload, err := runCancelTool(input)
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

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.queue_list",
		Description: "List queued/running/completed runs from the daemon.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input QueueListInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := queueListTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.approval_list",
		Description: "List runs awaiting approval (daemon required).",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input QueueListInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := approvalListTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.approval_approve",
		Description: "Approve a queued run (daemon required).",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input ApprovalInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if input.RunID == "" {
			return nil, nil, errors.New("Missing run_id")
		}
		payload, err := approvalApproveTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.approval_reject",
		Description: "Reject a queued run (daemon required).",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input ApprovalInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if input.RunID == "" {
			return nil, nil, errors.New("Missing run_id")
		}
		payload, err := approvalRejectTool(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.daemon_status",
		Description: "Get daemon health/status if running.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input QueueListInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := daemonStatusTool()
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

func runBatchAsyncTool(input BatchInput) (map[string]interface{}, error) {
	configPath := resolveConfigPath(input.Config)
	if !input.NoDaemon {
		if baseURL := resolveDaemonURL(configPath); baseURL != "" {
			if payload, err := daemonRunBatch(baseURL, input); err == nil {
				return payload, nil
			}
		}
	}
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
	if !input.NoDaemon {
		if baseURL := resolveDaemonURL(configPath); baseURL != "" {
			if payload, err := daemonRun(baseURL, input); err == nil {
				return payload, nil
			}
		}
	}

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

func runStatusTool(input StatusInput) (map[string]interface{}, error) {
	tail := input.Tail
	if tail <= 0 {
		tail = 4000
	}
	if baseURL := resolveDaemonURL(resolveConfigPath("")); baseURL != "" {
		if payload, err := daemonRunStatus(baseURL, input.RunID, tail); err == nil {
			return payload, nil
		}
	}
	return getRunStatus(input.RunID, tail)
}

func runWaitTool(input WaitInput) (map[string]interface{}, error) {
	tail := input.Tail
	if tail <= 0 {
		tail = 4000
	}
	timeout := time.Duration(input.TimeoutMs) * time.Millisecond
	if timeout <= 0 {
		timeout = time.Duration(defaultTimeoutMs) * time.Millisecond
	}
	if baseURL := resolveDaemonURL(resolveConfigPath("")); baseURL != "" {
		if payload, err := daemonRunWait(baseURL, input.RunID, timeout, tail); err == nil {
			return payload, nil
		}
	}
	return waitRun(input.RunID, timeout, tail)
}

func runCancelTool(input CancelInput) (map[string]interface{}, error) {
	if baseURL := resolveDaemonURL(resolveConfigPath("")); baseURL != "" {
		if payload, err := daemonRunCancel(baseURL, input.RunID, input.Force); err == nil {
			return payload, nil
		}
	}
	return cancelRun(input.RunID, input.Force)
}

func queueListTool(input QueueListInput) (map[string]interface{}, error) {
	baseURL := resolveDaemonURL(resolveConfigPath(""))
	if baseURL == "" {
		return map[string]interface{}{"status": "daemon_not_running"}, nil
	}
	return daemonQueueList(baseURL, input.Status, input.Limit)
}

func approvalListTool(input QueueListInput) (map[string]interface{}, error) {
	baseURL := resolveDaemonURL(resolveConfigPath(""))
	if baseURL == "" {
		return map[string]interface{}{"status": "daemon_not_running"}, nil
	}
	return daemonApprovalList(baseURL)
}

func approvalApproveTool(input ApprovalInput) (map[string]interface{}, error) {
	baseURL := resolveDaemonURL(resolveConfigPath(""))
	if baseURL == "" {
		return map[string]interface{}{"status": "daemon_not_running"}, nil
	}
	return daemonApprovalApprove(baseURL, input.RunID)
}

func approvalRejectTool(input ApprovalInput) (map[string]interface{}, error) {
	baseURL := resolveDaemonURL(resolveConfigPath(""))
	if baseURL == "" {
		return map[string]interface{}{"status": "daemon_not_running"}, nil
	}
	return daemonApprovalReject(baseURL, input.RunID)
}

func daemonStatusTool() (map[string]interface{}, error) {
	baseURL := resolveDaemonURL(resolveConfigPath(""))
	if baseURL == "" {
		return map[string]interface{}{"status": "daemon_not_running"}, nil
	}
	return daemonStatus(baseURL)
}
