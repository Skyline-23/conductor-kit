package main

import (
	"context"
	"errors"
	"fmt"
	"os"
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
		Description: "Run a single role/agent asynchronously and return run_id(s).",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input RunInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		report := progressReporterForRequest(ctx, req)
		if report != nil {
			report("started", 0, 1)
		}
		payload, err := runAsyncTool(input, report)
		if err != nil {
			return nil, nil, err
		}
		if report != nil {
			report("completed", 1, 1)
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_batch",
		Description: "Run multiple roles/agents in parallel and return outputs.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input BatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		report := progressReporterForRequest(ctx, req)
		payload, err := runBatchTool(input, report)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_async",
		Description: "Run a single role/agent asynchronously and return run_id.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input RunInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		report := progressReporterForRequest(ctx, req)
		if report != nil {
			report("starting", 0, 1)
		}
		payload, err := runAsyncTool(input, report)
		if err != nil {
			return nil, nil, err
		}
		if report != nil {
			report("started", 1, 1)
		}
		return nil, payload, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name:        "conductor.run_batch_async",
		Description: "Run multiple roles/agents asynchronously and return run_ids.",
	}, func(ctx context.Context, req *mcp.CallToolRequest, input BatchInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		report := progressReporterForRequest(ctx, req)
		payload, err := runBatchAsyncTool(input, report)
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
		report := progressReporterForRequest(ctx, req)
		if report != nil {
			report("waiting", 0, 1)
		}
		payload, err := runWaitTool(input)
		if err != nil {
			return nil, nil, err
		}
		if report != nil {
			report("done", 1, 1)
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

func progressReporterForRequest(ctx context.Context, req *mcp.CallToolRequest) progressReporter {
	if req == nil || req.Session == nil || req.Params == nil {
		return nil
	}
	token := req.Params.GetProgressToken()
	if token == nil {
		return nil
	}
	return func(message string, progress float64, total float64) {
		params := &mcp.ProgressNotificationParams{
			Message:       message,
			Progress:      progress,
			ProgressToken: token,
		}
		if total > 0 {
			params.Total = total
		}
		_ = req.Session.NotifyProgress(ctx, params)
	}
}

const defaultMcpTimeoutMs = 55000

func effectiveMcpTimeoutMs(timeoutMs int) int {
	if timeoutMs > 0 {
		return timeoutMs
	}
	return defaultMcpTimeoutMs
}

func applyTimeout(spec *CmdSpec, timeoutMs int) {
	if timeoutMs <= 0 {
		return
	}
	if spec.TimeoutMs == 0 || timeoutMs < spec.TimeoutMs {
		spec.TimeoutMs = timeoutMs
	}
}

func applyIdleTimeout(spec *CmdSpec, idleTimeoutMs int) {
	if idleTimeoutMs <= 0 {
		return
	}
	spec.IdleTimeoutMs = idleTimeoutMs
}

func resolveSummaryOnly(input *bool, cfg Config) bool {
	if input != nil {
		return *input
	}
	return cfg.Defaults.SummaryOnly
}

func summarizePayload(payload map[string]interface{}) map[string]interface{} {
	if payload == nil {
		return payload
	}
	keep := []string{
		"run_id",
		"status",
		"agent",
		"role",
		"model",
		"duration_ms",
		"exit_code",
		"started_at",
		"ended_at",
		"error",
		"read_files",
		"changed_files",
	}
	out := map[string]interface{}{}
	for _, key := range keep {
		if val, ok := payload[key]; ok {
			out[key] = val
		}
	}
	return out
}

func summarizeBatchPayload(payload map[string]interface{}) map[string]interface{} {
	if payload == nil {
		return payload
	}
	out := map[string]interface{}{}
	for _, key := range []string{"status", "agents", "count", "max_parallel", "warning", "note", "config"} {
		if val, ok := payload[key]; ok {
			out[key] = val
		}
	}
	results, _ := payload["results"].([]map[string]interface{})
	if results != nil {
		summary := make([]map[string]interface{}, 0, len(results))
		for _, res := range results {
			summary = append(summary, summarizePayload(res))
		}
		out["results"] = summary
	}
	return out
}

func runBatchTool(input BatchInput, report progressReporter) (map[string]interface{}, error) {
	timeoutMs := effectiveMcpTimeoutMs(input.TimeoutMs)
	payload, err := runBatch(input.Prompt, input.Roles, input.Config, input.Model, input.Reasoning, timeoutMs, input.IdleTimeoutMs, report)
	if err != nil {
		return payload, err
	}
	cfg, cfgErr := loadConfig(resolveConfigPath(input.Config))
	if cfgErr == nil && resolveSummaryOnly(input.SummaryOnly, cfg) {
		return summarizeBatchPayload(payload), nil
	}
	return payload, nil
}

func runBatchAsyncTool(input BatchInput, report progressReporter) (map[string]interface{}, error) {
	configPath := resolveConfigPath(input.Config)
	if !input.NoDaemon {
		if baseURL := resolveDaemonURL(configPath); baseURL != "" {
			if payload, err := daemonRunBatch(baseURL, input); err == nil {
				return payload, nil
			}
		}
	}
	return runBatchAsync(input.Prompt, input.Roles, input.Config, input.Model, input.Reasoning, input.TimeoutMs, input.IdleTimeoutMs, report)
}

func runTool(input RunInput, report progressReporter) (map[string]interface{}, error) {
	if input.Prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if input.Role == "" {
		return nil, errors.New("Missing role")
	}
	timeoutMs := effectiveMcpTimeoutMs(input.TimeoutMs)
	configPath := resolveConfigPath(input.Config)

	var cfg Config
	var err error
	cfg, err = loadConfig(configPath)
	if err != nil {
		return nil, err
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt

	var spec CmdSpec
	spec, err = buildSpecFromRole(cfg, input.Role, input.Prompt, input.Model, input.Reasoning, logPrompt)
	if err != nil {
		return nil, err
	}
	applyTimeout(&spec, timeoutMs)
	applyIdleTimeout(&spec, input.IdleTimeoutMs)
	payload, err := runCommand(spec)
	if err != nil {
		return payload, err
	}
	if resolveSummaryOnly(input.SummaryOnly, cfg) {
		return summarizePayload(payload), nil
	}
	return payload, nil
}

func runAsyncTool(input RunInput, report progressReporter) (map[string]interface{}, error) {
	if input.Prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if input.Role == "" {
		return nil, errors.New("Missing role")
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
	cfg, err = loadConfig(configPath)
	if err != nil {
		return nil, err
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt

	var spec CmdSpec
	spec, err = buildSpecFromRole(cfg, input.Role, input.Prompt, input.Model, input.Reasoning, logPrompt)
	if err != nil {
		return nil, err
	}
	if input.TimeoutMs > 0 {
		spec.TimeoutMs = input.TimeoutMs
	}
	if input.IdleTimeoutMs > 0 {
		spec.IdleTimeoutMs = input.IdleTimeoutMs
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
