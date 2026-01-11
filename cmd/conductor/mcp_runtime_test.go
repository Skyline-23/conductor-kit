package main

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func writeTempConfig(t *testing.T, approvalRequired bool) string {
	t.Helper()
	content := `{
  "defaults": {
    "timeout_ms": 120000,
    "idle_timeout_ms": 0,
    "summary_only": false,
    "max_parallel": 4,
    "retry": 0,
    "retry_backoff_ms": 500,
    "log_prompt": false
  },
  "runtime": {
    "max_parallel": 4,
    "queue": {
      "on_mode_change": "none"
    },
    "approval": {
      "required": %t,
      "roles": [],
      "agents": []
    }
  },
  "roles": {
    "oracle": {
      "cli": "codex"
    }
  }
}`
	config := []byte(fmt.Sprintf(content, approvalRequired))
	path := filepath.Join(t.TempDir(), "config.json")
	if err := os.WriteFile(path, config, 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}
	return path
}

func resetRuntime() {
	mcpRuntimeMu.Lock()
	if mcpRuntime != nil {
		stopMcpRuntimeLocked(mcpRuntime)
		mcpRuntime = nil
		mcpRuntimeConfigPath = ""
	}
	mcpRuntimeMu.Unlock()
}

func TestRuntimeQueueApprovalFlow(t *testing.T) {
	resetRuntime()
	configPath := writeTempConfig(t, true)

	payload, err := runAsyncTool(RunInput{Prompt: "hi", Role: "oracle", Config: configPath}, nil)
	if err != nil {
		t.Fatalf("runAsyncTool: %v", err)
	}
	runID, _ := payload["run_id"].(string)
	if runID == "" {
		t.Fatalf("expected run_id")
	}
	if status, _ := payload["status"].(string); status != "awaiting_approval" {
		t.Fatalf("expected awaiting_approval, got %q", status)
	}

	runtime := mcpRuntimeSnapshot()
	if runtime == nil {
		t.Fatalf("expected runtime")
	}
	runtime.cfg.MaxParallel = 0

	list, err := approvalListTool(QueueListInput{})
	if err != nil {
		t.Fatalf("approvalListTool: %v", err)
	}
	if count, ok := list["count"].(int); ok && count == 0 {
		t.Fatalf("expected approval list count > 0")
	}

	payload, err = approvalApproveTool(ApprovalInput{RunID: runID})
	if err != nil {
		t.Fatalf("approvalApproveTool: %v", err)
	}
	if status, _ := payload["status"].(string); status != "queued" {
		t.Fatalf("expected queued, got %q", status)
	}

	queued, err := queueListTool(QueueListInput{Status: "queued"})
	if err != nil {
		t.Fatalf("queueListTool: %v", err)
	}
	if count, ok := queued["count"].(int); ok && count == 0 {
		t.Fatalf("expected queued count > 0")
	}

	cancel, err := runCancelTool(CancelInput{RunID: runID})
	if err != nil {
		t.Fatalf("runCancelTool: %v", err)
	}
	if status, _ := cancel["status"].(string); status != "canceled" {
		t.Fatalf("expected canceled, got %q", status)
	}
}

func TestRuntimeBatchQueued(t *testing.T) {
	resetRuntime()
	configPath := writeTempConfig(t, true)

	_, err := ensureMcpRuntime(configPath)
	if err != nil {
		t.Fatalf("ensureMcpRuntime: %v", err)
	}
	runtime := mcpRuntimeSnapshot()
	if runtime == nil {
		t.Fatalf("expected runtime")
	}
	runtime.cfg.MaxParallel = 0

	payload, err := runBatchAsyncTool(BatchInput{Prompt: "hi", Roles: "oracle,oracle", Config: configPath, RequireApproval: true}, nil)
	if err != nil {
		t.Fatalf("runBatchAsyncTool: %v", err)
	}
	if status, _ := payload["status"].(string); status != "queued" {
		t.Fatalf("expected queued, got %q", status)
	}
	runs, ok := payload["runs"].([]map[string]interface{})
	if !ok || len(runs) != 2 {
		t.Fatalf("expected 2 runs")
	}
	for _, run := range runs {
		if status, _ := run["status"].(string); status != "awaiting_approval" {
			t.Fatalf("expected awaiting_approval, got %q", status)
		}
	}
}

func TestRuntimeStatus(t *testing.T) {
	resetRuntime()
	configPath := writeTempConfig(t, false)

	runtime, err := ensureMcpRuntime(configPath)
	if err != nil {
		t.Fatalf("ensureMcpRuntime: %v", err)
	}
	runtime.cfg.MaxParallel = 2

	payload, err := runtimeStatusTool()
	if err != nil {
		t.Fatalf("runtimeStatusTool: %v", err)
	}
	if ok, _ := payload["ok"].(bool); !ok {
		t.Fatalf("expected ok status")
	}
	if max, _ := payload["max_parallel"].(int); max != 2 {
		t.Fatalf("expected max_parallel=2, got %v", max)
	}
}
