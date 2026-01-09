package main

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math/rand"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"
)

type CmdSpec struct {
	Agent          string
	Role           string
	Model          string
	Reasoning      string
	Cmd            string
	Args           []string
	Env            map[string]string
	Cwd            string
	TimeoutMs      int
	Retry          int
	RetryBackoffMs int
	PromptHash     string
	PromptLen      int
	Prompt         string
	LogPrompt      bool
}

type AsyncMeta struct {
	ID              string   `json:"id"`
	Status          string   `json:"status"`
	Agent           string   `json:"agent,omitempty"`
	Role            string   `json:"role,omitempty"`
	Model           string   `json:"model,omitempty"`
	Cmd             string   `json:"cmd"`
	Args            []string `json:"args,omitempty"`
	PID             int      `json:"pid"`
	Attempt         int      `json:"attempt"`
	Attempts        int      `json:"attempts"`
	ExitCode        int      `json:"exit_code,omitempty"`
	Error           string   `json:"error,omitempty"`
	StartedAt       string   `json:"started_at,omitempty"`
	EndedAt         string   `json:"ended_at,omitempty"`
	PromptHash      string   `json:"prompt_hash,omitempty"`
	PromptLen       int      `json:"prompt_len,omitempty"`
	CancelRequested bool     `json:"cancel_requested,omitempty"`
}

func buildSpecFromAgent(agent, prompt string, defaults Defaults, logPrompt bool) (CmdSpec, error) {
	mapping := map[string]CmdSpec{
		"claude": {Agent: "claude", Cmd: "claude", Args: []string{"-p", prompt}},
		"codex":  {Agent: "codex", Cmd: "codex", Args: []string{"exec", prompt}},
		"gemini": {Agent: "gemini", Cmd: "gemini", Args: []string{prompt}},
	}
	if spec, ok := mapping[agent]; ok {
		spec.TimeoutMs = defaults.TimeoutMs
		spec.Retry = defaults.Retry
		spec.RetryBackoffMs = defaults.RetryBackoffMs
		spec.PromptHash, spec.PromptLen = promptMeta(prompt)
		if logPrompt {
			spec.Prompt = prompt
			spec.LogPrompt = true
		}
		return spec, nil
	}
	return CmdSpec{}, fmt.Errorf("Unknown agent: %s", agent)
}

func buildSpecFromRole(cfg Config, role, prompt, modelOverride, reasoningOverride string, logPrompt bool) (CmdSpec, error) {
	roleCfg, ok := cfg.Roles[role]
	if !ok {
		return CmdSpec{}, fmt.Errorf("Missing role config for: %s", role)
	}
	defaults := normalizeDefaults(cfg.Defaults)
	model := modelOverride
	if model == "" {
		model = roleCfg.Model
	}
	reasoning := reasoningOverride
	if reasoning == "" {
		reasoning = roleCfg.Reasoning
	}
	args := append([]string{}, roleCfg.Args...)
	promptIndex := indexOf(args, "{prompt}")
	insertIndex := promptIndex
	if insertIndex < 0 {
		insertIndex = len(args)
	}
	extra := []string{}
	if reasoning != "" && roleCfg.ReasoningFlag != "" && roleCfg.ReasoningKey != "" {
		extra = append(extra, roleCfg.ReasoningFlag, fmt.Sprintf("%s=%s", roleCfg.ReasoningKey, reasoning))
	}
	if model != "" && roleCfg.ModelFlag != "" {
		extra = append(extra, roleCfg.ModelFlag, model)
	}
	if len(extra) > 0 {
		args = append(args[:insertIndex], append(extra, args[insertIndex:]...)...)
	}
	if promptIndex >= 0 {
		for i := range args {
			if args[i] == "{prompt}" {
				args[i] = prompt
			}
		}
	} else {
		args = append(args, prompt)
	}
	spec := CmdSpec{
		Agent:          roleCfg.CLI,
		Role:           role,
		Model:          model,
		Reasoning:      reasoning,
		Cmd:            roleCfg.CLI,
		Args:           args,
		Env:            roleCfg.Env,
		Cwd:            roleCfg.Cwd,
		TimeoutMs:      effectiveInt(roleCfg.TimeoutMs, defaults.TimeoutMs),
		Retry:          effectiveInt(roleCfg.Retry, defaults.Retry),
		RetryBackoffMs: effectiveInt(roleCfg.RetryBackoffMs, defaults.RetryBackoffMs),
	}
	spec.PromptHash, spec.PromptLen = promptMeta(prompt)
	if logPrompt {
		spec.Prompt = prompt
		spec.LogPrompt = true
	}
	return spec, nil
}

func indexOf(items []string, target string) int {
	for i, v := range items {
		if v == target {
			return i
		}
	}
	return -1
}

func newRunID() string {
	return fmt.Sprintf("run-%d-%04x", time.Now().UnixMilli(), rand.Intn(65535))
}

func promptMeta(prompt string) (string, int) {
	sum := sha256.Sum256([]byte(prompt))
	return hex.EncodeToString(sum[:]), len(prompt)
}

func asyncRunDir(runID string) string {
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	return filepath.Join(baseDir, "runs", "async", runID)
}

func asyncMetaPath(runID string) string {
	return filepath.Join(asyncRunDir(runID), "meta.json")
}

func writeAsyncMeta(meta AsyncMeta) error {
	path := asyncMetaPath(meta.ID)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(meta, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}

func loadAsyncMeta(runID string) (AsyncMeta, string, error) {
	var meta AsyncMeta
	path := asyncMetaPath(runID)
	data, err := os.ReadFile(path)
	if err != nil {
		return meta, "", err
	}
	if err := json.Unmarshal(data, &meta); err != nil {
		return meta, "", err
	}
	return meta, asyncRunDir(runID), nil
}

func isRunning(pid int) bool {
	if pid <= 0 {
		return false
	}
	err := syscall.Kill(pid, 0)
	return err == nil || err == syscall.EPERM
}

func readTail(path string, bytes int) string {
	file, err := os.Open(path)
	if err != nil {
		return ""
	}
	defer file.Close()

	info, err := file.Stat()
	if err != nil {
		return ""
	}
	size := info.Size()
	start := size - int64(bytes)
	if start < 0 {
		start = 0
	}
	_, _ = file.Seek(start, 0)
	data, _ := io.ReadAll(file)
	return string(data)
}

func statusFromError(ctx context.Context, err error) (string, int, string) {
	if err == nil {
		return "ok", 0, ""
	}
	status := "error"
	switch {
	case errors.Is(ctx.Err(), context.DeadlineExceeded):
		status = "timeout"
	case errors.Is(ctx.Err(), context.Canceled):
		status = "canceled"
	}
	exitCode := 1
	if exitErr, ok := err.(*exec.ExitError); ok {
		if waitStatus, ok := exitErr.Sys().(syscall.WaitStatus); ok {
			exitCode = waitStatus.ExitStatus()
		}
	}
	return status, exitCode, err.Error()
}

func isCommandAvailable(cmd string) bool {
	pathEnv := os.Getenv("PATH")
	for _, dir := range strings.Split(pathEnv, ":") {
		candidate := filepath.Join(dir, cmd)
		if pathExists(candidate) {
			return true
		}
	}
	return false
}

func runCommand(spec CmdSpec) (map[string]interface{}, error) {
	if !isCommandAvailable(spec.Cmd) {
		return nil, fmt.Errorf("Missing CLI on PATH: %s", spec.Cmd)
	}

	attempts := spec.Retry + 1
	if attempts < 1 {
		attempts = 1
	}
	backoff := time.Duration(spec.RetryBackoffMs) * time.Millisecond

	var last map[string]interface{}
	for i := 1; i <= attempts; i++ {
		res, err := runCommandOnce(spec, i, attempts)
		if err != nil {
			return nil, err
		}
		last = res
		if res["status"] == "ok" {
			return res, nil
		}
		if i < attempts && backoff > 0 {
			time.Sleep(backoff)
		}
	}
	return last, nil
}

func runCommandOnce(spec CmdSpec, attempt, attempts int) (map[string]interface{}, error) {
	timeout := time.Duration(spec.TimeoutMs) * time.Millisecond
	ctx := context.Background()
	var cancel context.CancelFunc
	if timeout > 0 {
		ctx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}

	runID := newRunID()
	start := time.Now().UTC()
	cmd := exec.CommandContext(ctx, spec.Cmd, spec.Args...)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if spec.Cwd != "" {
		cmd.Dir = spec.Cwd
	}
	if len(spec.Env) > 0 {
		env := append([]string{}, os.Environ()...)
		for k, v := range spec.Env {
			env = append(env, fmt.Sprintf("%s=%s", k, v))
		}
		cmd.Env = env
	}

	err := cmd.Run()
	end := time.Now().UTC()
	duration := end.Sub(start).Milliseconds()

	status, exitCode, errMsg := statusFromError(ctx, err)

	payload := map[string]interface{}{
		"run_id":      runID,
		"status":      status,
		"agent":       firstNonEmpty(spec.Role, spec.Agent),
		"role":        spec.Role,
		"model":       spec.Model,
		"cmd":         spec.Cmd,
		"args":        spec.Args,
		"attempt":     attempt,
		"attempts":    attempts,
		"exit_code":   exitCode,
		"stdout":      strings.TrimSpace(stdout.String()),
		"stderr":      strings.TrimSpace(stderr.String()),
		"duration_ms": duration,
		"started_at":  start.Format(time.RFC3339),
		"ended_at":    end.Format(time.RFC3339),
	}
	if errMsg != "" {
		payload["error"] = errMsg
	}

	record := RunRecord{
		ID:         runID,
		Agent:      spec.Agent,
		Role:       spec.Role,
		Model:      spec.Model,
		Cmd:        spec.Cmd,
		Args:       spec.Args,
		Status:     status,
		ExitCode:   exitCode,
		StartedAt:  payload["started_at"].(string),
		EndedAt:    payload["ended_at"].(string),
		DurationMs: duration,
		PromptHash: spec.PromptHash,
		PromptLen:  spec.PromptLen,
		Prompt:     spec.Prompt,
		Error:      errMsg,
	}
	_ = appendRunRecord(record, spec.LogPrompt)

	return payload, nil
}

func startAsync(spec CmdSpec) (map[string]interface{}, error) {
	return startAsyncWithID(newRunID(), spec)
}

func startAsyncWithID(runID string, spec CmdSpec) (map[string]interface{}, error) {
	if !isCommandAvailable(spec.Cmd) {
		return nil, fmt.Errorf("Missing CLI on PATH: %s", spec.Cmd)
	}

	runDir := asyncRunDir(runID)
	if err := os.MkdirAll(runDir, 0o755); err != nil {
		return nil, err
	}
	stdoutPath := filepath.Join(runDir, "stdout.log")
	stderrPath := filepath.Join(runDir, "stderr.log")

	stdoutFile, err := os.OpenFile(stdoutPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return nil, err
	}
	stderrFile, err := os.OpenFile(stderrPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		_ = stdoutFile.Close()
		return nil, err
	}

	attempts := spec.Retry + 1
	if attempts < 1 {
		attempts = 1
	}
	meta := AsyncMeta{
		ID:         runID,
		Status:     "starting",
		Agent:      spec.Agent,
		Role:       spec.Role,
		Model:      spec.Model,
		Cmd:        spec.Cmd,
		Args:       spec.Args,
		Attempt:    0,
		Attempts:   attempts,
		PromptHash: spec.PromptHash,
		PromptLen:  spec.PromptLen,
	}
	_ = writeAsyncMeta(meta)

	go runAsyncAttempts(runID, spec, stdoutFile, stderrFile)

	return map[string]interface{}{
		"run_id":   runID,
		"status":   "running",
		"agent":    firstNonEmpty(spec.Role, spec.Agent),
		"attempts": attempts,
	}, nil
}

func runAsyncAttempts(runID string, spec CmdSpec, stdoutFile, stderrFile *os.File) {
	defer stdoutFile.Close()
	defer stderrFile.Close()

	attempts := spec.Retry + 1
	if attempts < 1 {
		attempts = 1
	}
	backoff := time.Duration(spec.RetryBackoffMs) * time.Millisecond

	var startedAt time.Time
	var endedAt time.Time
	var status string
	var exitCode int
	var errMsg string
	lastAttempt := 0

	for attempt := 1; attempt <= attempts; attempt++ {
		lastAttempt = attempt
		ctx := context.Background()
		var cancel context.CancelFunc
		if spec.TimeoutMs > 0 {
			ctx, cancel = context.WithTimeout(ctx, time.Duration(spec.TimeoutMs)*time.Millisecond)
		}

		cmd := exec.CommandContext(ctx, spec.Cmd, spec.Args...)
		cmd.Stdout = stdoutFile
		cmd.Stderr = stderrFile
		if spec.Cwd != "" {
			cmd.Dir = spec.Cwd
		}
		if len(spec.Env) > 0 {
			env := append([]string{}, os.Environ()...)
			for k, v := range spec.Env {
				env = append(env, fmt.Sprintf("%s=%s", k, v))
			}
			cmd.Env = env
		}

		start := time.Now().UTC()
		if startedAt.IsZero() {
			startedAt = start
		}
		if err := cmd.Start(); err != nil {
			status = "error"
			exitCode = 1
			errMsg = err.Error()
			endedAt = time.Now().UTC()
			if cancel != nil {
				cancel()
			}
			break
		}

		meta := AsyncMeta{
			ID:         runID,
			Status:     "running",
			Agent:      spec.Agent,
			Role:       spec.Role,
			Model:      spec.Model,
			Cmd:        spec.Cmd,
			Args:       spec.Args,
			PID:        cmd.Process.Pid,
			Attempt:    attempt,
			Attempts:   attempts,
			StartedAt:  startedAt.Format(time.RFC3339),
			PromptHash: spec.PromptHash,
			PromptLen:  spec.PromptLen,
		}
		_ = writeAsyncMeta(meta)

		err := cmd.Wait()
		if cancel != nil {
			cancel()
		}
		endedAt = time.Now().UTC()
		status, exitCode, errMsg = statusFromError(ctx, err)

		cancelRequested := false
		if current, _, err := loadAsyncMeta(runID); err == nil {
			cancelRequested = current.CancelRequested
		}
		if cancelRequested && status != "ok" {
			status = "canceled"
		}

		if status == "ok" {
			break
		}
		if attempt < attempts && backoff > 0 {
			time.Sleep(backoff)
		}
	}

	finalMeta := AsyncMeta{
		ID:         runID,
		Status:     status,
		Agent:      spec.Agent,
		Role:       spec.Role,
		Model:      spec.Model,
		Cmd:        spec.Cmd,
		Args:       spec.Args,
		Attempt:    lastAttempt,
		Attempts:   attempts,
		ExitCode:   exitCode,
		Error:      errMsg,
		StartedAt:  startedAt.Format(time.RFC3339),
		EndedAt:    endedAt.Format(time.RFC3339),
		PromptHash: spec.PromptHash,
		PromptLen:  spec.PromptLen,
	}
	if current, _, err := loadAsyncMeta(runID); err == nil {
		finalMeta.CancelRequested = current.CancelRequested
		if finalMeta.CancelRequested && finalMeta.Status != "ok" {
			finalMeta.Status = "canceled"
		}
	}
	_ = writeAsyncMeta(finalMeta)

	record := RunRecord{
		ID:         runID,
		Agent:      spec.Agent,
		Role:       spec.Role,
		Model:      spec.Model,
		Cmd:        spec.Cmd,
		Args:       spec.Args,
		Status:     finalMeta.Status,
		ExitCode:   exitCode,
		StartedAt:  finalMeta.StartedAt,
		EndedAt:    finalMeta.EndedAt,
		DurationMs: endedAt.Sub(startedAt).Milliseconds(),
		PromptHash: spec.PromptHash,
		PromptLen:  spec.PromptLen,
		Prompt:     spec.Prompt,
		Error:      errMsg,
	}
	if finalMeta.Status == "ok" {
		record.Error = ""
	}
	_ = appendRunRecord(record, spec.LogPrompt)
}

func getRunStatus(runID string, tailBytes int) (map[string]interface{}, error) {
	meta, dir, err := loadAsyncMeta(runID)
	if err != nil {
		return nil, err
	}
	running := isRunning(meta.PID)
	status := meta.Status
	if running {
		status = "running"
	}
	stdout := readTail(filepath.Join(dir, "stdout.log"), tailBytes)
	stderr := readTail(filepath.Join(dir, "stderr.log"), tailBytes)
	return map[string]interface{}{
		"run_id":     runID,
		"status":     status,
		"agent":      firstNonEmpty(meta.Role, meta.Agent),
		"role":       meta.Role,
		"model":      meta.Model,
		"pid":        meta.PID,
		"attempt":    meta.Attempt,
		"attempts":   meta.Attempts,
		"exit_code":  meta.ExitCode,
		"stdout":     strings.TrimSpace(stdout),
		"stderr":     strings.TrimSpace(stderr),
		"error":      meta.Error,
		"started_at": meta.StartedAt,
		"ended_at":   meta.EndedAt,
	}, nil
}

func waitRun(runID string, timeout time.Duration, tailBytes int) (map[string]interface{}, error) {
	deadline := time.Now().Add(timeout)
	for {
		res, err := getRunStatus(runID, tailBytes)
		if err != nil {
			return nil, err
		}
		if res["status"] != "running" || time.Now().After(deadline) {
			return res, nil
		}
		time.Sleep(1 * time.Second)
	}
}

func cancelRun(runID string, force bool) (map[string]interface{}, error) {
	meta, _, err := loadAsyncMeta(runID)
	if err != nil {
		return map[string]interface{}{"run_id": runID, "status": "not_found"}, nil
	}
	if meta.PID <= 0 {
		return map[string]interface{}{"run_id": runID, "status": "not_running"}, nil
	}
	meta.CancelRequested = true
	_ = writeAsyncMeta(meta)
	status := "cancelled"
	if err := syscall.Kill(meta.PID, syscall.SIGTERM); err != nil {
		status = "not_running"
	}
	if force {
		time.Sleep(500 * time.Millisecond)
		_ = syscall.Kill(meta.PID, syscall.SIGKILL)
	}
	return map[string]interface{}{"run_id": runID, "status": status}, nil
}

func firstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return ""
}

func runBatch(prompt, roles, agents, configPath, modelOverride, reasoningOverride string, timeoutMs int) (map[string]interface{}, error) {
	if prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	configPath = resolveConfigPath(configPath)

	results := []map[string]interface{}{}
	agentList := []string{}

	type specEntry struct {
		agent string
		spec  CmdSpec
	}
	entries := []specEntry{}

	var cfg Config
	var err error
	if roles != "" {
		cfg, err = loadConfig(configPath)
		if err != nil {
			return map[string]interface{}{"status": "missing_config", "note": "Role-based batch requested but config is missing or invalid.", "config": configPath}, nil
		}
	} else {
		cfg, err = loadConfigOrEmpty(configPath)
		if err != nil {
			return map[string]interface{}{"status": "invalid_config", "note": err.Error(), "config": configPath}, nil
		}
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt
	maxParallel := defaults.MaxParallel

	if roles != "" {
		roleNames := []string{}
		if roles == "auto" {
			for k := range cfg.Roles {
				roleNames = append(roleNames, k)
			}
		} else {
			roleNames = splitList(roles)
		}
		if len(roleNames) == 0 {
			return map[string]interface{}{"status": "no_roles"}, nil
		}
		agentList = roleNames

		for _, role := range roleNames {
			roleCfg := cfg.Roles[role]
			if roleCfg.MaxParallel > 0 && roleCfg.MaxParallel < maxParallel {
				maxParallel = roleCfg.MaxParallel
			}
			models := expandModelEntries(roleCfg, modelOverride, reasoningOverride)
			if len(models) == 0 {
				models = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
			}
			for _, entry := range models {
				spec, err := buildSpecFromRole(cfg, role, prompt, entry.Name, entry.ReasoningEffort, logPrompt)
				if err != nil {
					results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": err.Error()})
					continue
				}
				if timeoutMs > 0 {
					spec.TimeoutMs = timeoutMs
				}
				entries = append(entries, specEntry{agent: role, spec: spec})
			}
		}
	} else {
		agentArg := agents
		if agentArg == "" {
			agentArg = "auto"
		}
		if agentArg == "auto" {
			agentList = detectAgents()
		} else {
			agentList = splitList(agentArg)
		}
		if len(agentList) == 0 {
			return map[string]interface{}{"status": "no_agents", "note": "No CLI agents detected. Install codex/claude/gemini or pass agents."}, nil
		}
		for _, agent := range agentList {
			spec, err := buildSpecFromAgent(agent, prompt, defaults, logPrompt)
			if err != nil {
				results = append(results, map[string]interface{}{"agent": agent, "status": "error", "error": err.Error()})
				continue
			}
			if timeoutMs > 0 {
				spec.TimeoutMs = timeoutMs
			}
			entries = append(entries, specEntry{agent: agent, spec: spec})
		}
	}

	if maxParallel <= 0 {
		maxParallel = 1
	}
	sem := make(chan struct{}, maxParallel)

	var wg sync.WaitGroup
	var mu sync.Mutex
	for _, entry := range entries {
		wg.Add(1)
		go func(e specEntry) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			res, err := runCommand(e.spec)
			mu.Lock()
			defer mu.Unlock()
			if err != nil {
				results = append(results, map[string]interface{}{"agent": e.agent, "status": "error", "error": err.Error()})
				return
			}
			results = append(results, res)
		}(entry)
	}
	wg.Wait()

	status := "ok"
	for _, r := range results {
		if s, ok := r["status"].(string); ok && s != "ok" {
			status = "partial"
			break
		}
	}

	return map[string]interface{}{
		"status":       status,
		"agents":       agentList,
		"results":      results,
		"count":        len(results),
		"max_parallel": maxParallel,
		"warning": func() interface{} {
			if roles == "" && (modelOverride != "" || reasoningOverride != "") {
				return "Model overrides apply only to roles mode"
			}
			return nil
		}(),
	}, nil
}

func runBatchAsync(prompt, roles, agents, configPath, modelOverride, reasoningOverride string, timeoutMs int) (map[string]interface{}, error) {
	if prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	configPath = resolveConfigPath(configPath)

	results := []map[string]interface{}{}
	agentList := []string{}

	type specEntry struct {
		agent string
		spec  CmdSpec
	}
	entries := []specEntry{}

	var cfg Config
	var err error
	if roles != "" {
		cfg, err = loadConfig(configPath)
		if err != nil {
			return map[string]interface{}{"status": "missing_config", "note": "Role-based batch requested but config is missing or invalid.", "config": configPath}, nil
		}
	} else {
		cfg, err = loadConfigOrEmpty(configPath)
		if err != nil {
			return map[string]interface{}{"status": "invalid_config", "note": err.Error(), "config": configPath}, nil
		}
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt

	if roles != "" {
		roleNames := []string{}
		if roles == "auto" {
			for k := range cfg.Roles {
				roleNames = append(roleNames, k)
			}
		} else {
			roleNames = splitList(roles)
		}
		if len(roleNames) == 0 {
			return map[string]interface{}{"status": "no_roles"}, nil
		}
		agentList = roleNames

		for _, role := range roleNames {
			roleCfg := cfg.Roles[role]
			models := expandModelEntries(roleCfg, modelOverride, reasoningOverride)
			if len(models) == 0 {
				models = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
			}
			for _, entry := range models {
				spec, err := buildSpecFromRole(cfg, role, prompt, entry.Name, entry.ReasoningEffort, logPrompt)
				if err != nil {
					results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": err.Error()})
					continue
				}
				if timeoutMs > 0 {
					spec.TimeoutMs = timeoutMs
				}
				entries = append(entries, specEntry{agent: role, spec: spec})
			}
		}
	} else {
		agentArg := agents
		if agentArg == "" {
			agentArg = "auto"
		}
		if agentArg == "auto" {
			agentList = detectAgents()
		} else {
			agentList = splitList(agentArg)
		}
		if len(agentList) == 0 {
			return map[string]interface{}{"status": "no_agents", "note": "No CLI agents detected. Install codex/claude/gemini or pass agents."}, nil
		}
		for _, agent := range agentList {
			spec, err := buildSpecFromAgent(agent, prompt, defaults, logPrompt)
			if err != nil {
				results = append(results, map[string]interface{}{"agent": agent, "status": "error", "error": err.Error()})
				continue
			}
			if timeoutMs > 0 {
				spec.TimeoutMs = timeoutMs
			}
			entries = append(entries, specEntry{agent: agent, spec: spec})
		}
	}

	for _, entry := range entries {
		res, err := startAsync(entry.spec)
		if err != nil {
			results = append(results, map[string]interface{}{"agent": entry.agent, "status": "error", "error": err.Error()})
			continue
		}
		res["agent"] = entry.agent
		results = append(results, res)
	}

	return map[string]interface{}{
		"status":  "started",
		"agents":  agentList,
		"results": results,
		"count":   len(results),
	}, nil
}

func detectAgents() []string {
	out := []string{}
	if isCommandAvailable("codex") {
		out = append(out, "codex")
	}
	if isCommandAvailable("claude") {
		out = append(out, "claude")
	}
	if isCommandAvailable("gemini") {
		out = append(out, "gemini")
	}
	return out
}
