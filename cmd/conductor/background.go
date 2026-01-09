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
	"sort"
	"strings"
	"sync"
	"sync/atomic"
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
	ReadyCmd       string
	ReadyArgs      []string
	ReadyTimeoutMs int
	Env            map[string]string
	Cwd            string
	TimeoutMs      int
	IdleTimeoutMs  int
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
	ReadFiles       []string `json:"read_files,omitempty"`
	ChangedFiles    []string `json:"changed_files,omitempty"`
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
		spec.IdleTimeoutMs = defaults.IdleTimeoutMs
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
	roleCfg, err := normalizeRoleConfig(roleCfg)
	if err != nil {
		return CmdSpec{}, err
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
		ReadyCmd:       roleCfg.ReadyCmd,
		ReadyArgs:      roleCfg.ReadyArgs,
		ReadyTimeoutMs: roleCfg.ReadyTimeoutMs,
		Env:            roleCfg.Env,
		Cwd:            roleCfg.Cwd,
		TimeoutMs:      effectiveInt(roleCfg.TimeoutMs, defaults.TimeoutMs),
		IdleTimeoutMs:  effectiveInt(roleCfg.IdleTimeoutMs, defaults.IdleTimeoutMs),
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

type activityWriter struct {
	w          io.Writer
	activityCh chan struct{}
}

func (a *activityWriter) Write(p []byte) (int, error) {
	n, err := a.w.Write(p)
	if n > 0 && a.activityCh != nil {
		select {
		case a.activityCh <- struct{}{}:
		default:
		}
	}
	return n, err
}

func startIdleTimer(ctx context.Context, idle time.Duration, activityCh <-chan struct{}, onTimeout func()) func() {
	if idle <= 0 || activityCh == nil {
		return func() {}
	}
	stopCh := make(chan struct{})
	go func() {
		timer := time.NewTimer(idle)
		defer timer.Stop()
		for {
			select {
			case <-activityCh:
				if !timer.Stop() {
					select {
					case <-timer.C:
					default:
					}
				}
				timer.Reset(idle)
			case <-ctx.Done():
				return
			case <-stopCh:
				return
			case <-timer.C:
				onTimeout()
				return
			}
		}
	}()
	return func() { close(stopCh) }
}

func statusFromErrorWithTimeout(ctx context.Context, err error, timedOut bool) (string, int, string) {
	if err == nil {
		return "ok", 0, ""
	}
	status := "error"
	switch {
	case timedOut:
		status = "timeout"
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

func cwdForSpec(spec CmdSpec) string {
	if spec.Cwd != "" {
		return spec.Cwd
	}
	cwd, err := os.Getwd()
	if err != nil {
		return ""
	}
	return cwd
}

func gitStatusSnapshot(cwd string) (map[string]string, error) {
	if cwd == "" {
		return nil, errors.New("missing cwd")
	}
	cmd := exec.Command("git", "-C", cwd, "status", "--porcelain")
	out, err := cmd.Output()
	if err != nil {
		return nil, err
	}
	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	if len(lines) == 1 && lines[0] == "" {
		return map[string]string{}, nil
	}
	snapshot := map[string]string{}
	for _, line := range lines {
		if len(line) < 3 {
			continue
		}
		status := strings.TrimSpace(line[:2])
		path := strings.TrimSpace(line[2:])
		if path == "" {
			continue
		}
		if arrow := strings.LastIndex(path, "->"); arrow >= 0 {
			path = strings.TrimSpace(path[arrow+2:])
		}
		snapshot[path] = status
	}
	return snapshot, nil
}

func diffGitStatus(before, after map[string]string) []string {
	if before == nil || after == nil {
		return nil
	}
	changes := []string{}
	for path, status := range after {
		if prev, ok := before[path]; !ok || prev != status {
			if status == "" {
				changes = append(changes, path)
			} else {
				changes = append(changes, status+" "+path)
			}
		}
	}
	sort.Strings(changes)
	return changes
}

func extractReadFiles(output string) []string {
	lines := strings.Split(output, "\n")
	reads := []string{}
	seen := map[string]bool{}
	prefixes := []string{
		"read file ",
		"read_file ",
		"open file ",
		"opened file ",
		"read: ",
		"open: ",
	}
	for _, line := range lines {
		text := strings.TrimSpace(line)
		if text == "" {
			continue
		}
		lower := strings.ToLower(text)
		for _, prefix := range prefixes {
			if strings.HasPrefix(lower, prefix) {
				path := strings.TrimSpace(text[len(prefix):])
				path = strings.Trim(path, "\"'`")
				if path != "" && !seen[path] {
					seen[path] = true
					reads = append(reads, path)
				}
				break
			}
		}
	}
	sort.Strings(reads)
	return reads
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

	if err := checkReady(spec); err != nil {
		now := time.Now().UTC()
		payload := map[string]interface{}{
			"run_id":      newRunID(),
			"status":      "not_ready",
			"agent":       firstNonEmpty(spec.Role, spec.Agent),
			"role":        spec.Role,
			"model":       spec.Model,
			"cmd":         spec.Cmd,
			"args":        spec.Args,
			"attempt":     0,
			"attempts":    attempts,
			"exit_code":   1,
			"duration_ms": int64(0),
			"started_at":  now.Format(time.RFC3339),
			"ended_at":    now.Format(time.RFC3339),
			"error":       err.Error(),
		}
		record := RunRecord{
			ID:         payload["run_id"].(string),
			Agent:      spec.Agent,
			Role:       spec.Role,
			Model:      spec.Model,
			Cmd:        spec.Cmd,
			Args:       spec.Args,
			Status:     "not_ready",
			ExitCode:   1,
			StartedAt:  payload["started_at"].(string),
			EndedAt:    payload["ended_at"].(string),
			DurationMs: 0,
			PromptHash: spec.PromptHash,
			PromptLen:  spec.PromptLen,
			Prompt:     spec.Prompt,
			Error:      err.Error(),
		}
		_ = appendRunRecord(record, spec.LogPrompt)
		return payload, nil
	}

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

const defaultReadyTimeoutMs = 5000

func checkReady(spec CmdSpec) error {
	if spec.ReadyCmd == "" {
		return nil
	}
	if !isCommandAvailable(spec.ReadyCmd) {
		return fmt.Errorf("Ready check missing CLI on PATH: %s", spec.ReadyCmd)
	}
	timeoutMs := spec.ReadyTimeoutMs
	if timeoutMs <= 0 {
		timeoutMs = defaultReadyTimeoutMs
	}
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(timeoutMs)*time.Millisecond)
	defer cancel()
	cmd := exec.CommandContext(ctx, spec.ReadyCmd, spec.ReadyArgs...)
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
	cmd.Stdin = strings.NewReader("")
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	if ctx.Err() == context.DeadlineExceeded {
		return fmt.Errorf("ready check timed out after %dms", timeoutMs)
	}
	if err != nil {
		msg := strings.TrimSpace(stderr.String())
		if msg == "" {
			msg = strings.TrimSpace(stdout.String())
		}
		if msg != "" {
			return fmt.Errorf("ready check failed: %s", msg)
		}
		return fmt.Errorf("ready check failed")
	}
	return nil
}

func runCommandOnce(spec CmdSpec, attempt, attempts int) (map[string]interface{}, error) {
	timeout := time.Duration(spec.TimeoutMs) * time.Millisecond
	idleTimeout := time.Duration(spec.IdleTimeoutMs) * time.Millisecond
	ctx := context.Background()
	var cancel context.CancelFunc
	if timeout > 0 {
		ctx, cancel = context.WithTimeout(ctx, timeout)
	} else {
		ctx, cancel = context.WithCancel(ctx)
	}
	defer cancel()

	cwd := cwdForSpec(spec)
	var beforeStatus map[string]string
	if snapshot, err := gitStatusSnapshot(cwd); err == nil {
		beforeStatus = snapshot
	}

	activityCh := make(chan struct{}, 1)
	var idleTimedOut atomic.Bool
	stopIdle := startIdleTimer(ctx, idleTimeout, activityCh, func() {
		idleTimedOut.Store(true)
		cancel()
	})
	defer stopIdle()

	runID := newRunID()
	start := time.Now().UTC()
	cmd := exec.CommandContext(ctx, spec.Cmd, spec.Args...)
	stdoutPipe, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	stderrPipe, err := cmd.StderrPipe()
	if err != nil {
		return nil, err
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	stdoutWriter := &activityWriter{w: &stdout, activityCh: activityCh}
	stderrWriter := &activityWriter{w: &stderr, activityCh: activityCh}
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

	if err := cmd.Start(); err != nil {
		return nil, err
	}
	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		_, _ = io.Copy(stdoutWriter, stdoutPipe)
		wg.Done()
	}()
	go func() {
		_, _ = io.Copy(stderrWriter, stderrPipe)
		wg.Done()
	}()
	err = cmd.Wait()
	wg.Wait()
	end := time.Now().UTC()
	duration := end.Sub(start).Milliseconds()

	var changedFiles []string
	if snapshot, err := gitStatusSnapshot(cwd); err == nil {
		changedFiles = diffGitStatus(beforeStatus, snapshot)
	}
	readFiles := extractReadFiles(stdout.String() + "\n" + stderr.String())
	status, exitCode, errMsg := statusFromErrorWithTimeout(ctx, err, idleTimedOut.Load())

	payload := map[string]interface{}{
		"run_id":        runID,
		"status":        status,
		"agent":         firstNonEmpty(spec.Role, spec.Agent),
		"role":          spec.Role,
		"model":         spec.Model,
		"cmd":           spec.Cmd,
		"args":          spec.Args,
		"attempt":       attempt,
		"attempts":      attempts,
		"exit_code":     exitCode,
		"stdout":        strings.TrimSpace(stdout.String()),
		"stderr":        strings.TrimSpace(stderr.String()),
		"duration_ms":   duration,
		"started_at":    start.Format(time.RFC3339),
		"ended_at":      end.Format(time.RFC3339),
		"read_files":    readFiles,
		"changed_files": changedFiles,
	}
	if errMsg != "" {
		payload["error"] = errMsg
	}

	record := RunRecord{
		ID:           runID,
		Agent:        spec.Agent,
		Role:         spec.Role,
		Model:        spec.Model,
		Cmd:          spec.Cmd,
		Args:         spec.Args,
		Status:       status,
		ExitCode:     exitCode,
		StartedAt:    payload["started_at"].(string),
		EndedAt:      payload["ended_at"].(string),
		DurationMs:   duration,
		PromptHash:   spec.PromptHash,
		PromptLen:    spec.PromptLen,
		Prompt:       spec.Prompt,
		ReadFiles:    readFiles,
		ChangedFiles: changedFiles,
		Error:        errMsg,
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
	if err := checkReady(spec); err != nil {
		now := time.Now().UTC()
		runDir := asyncRunDir(runID)
		if err := os.MkdirAll(runDir, 0o755); err != nil {
			return nil, err
		}
		meta := AsyncMeta{
			ID:         runID,
			Status:     "not_ready",
			Agent:      spec.Agent,
			Role:       spec.Role,
			Model:      spec.Model,
			Cmd:        spec.Cmd,
			Args:       spec.Args,
			Attempt:    0,
			Attempts:   spec.Retry + 1,
			ExitCode:   1,
			Error:      err.Error(),
			StartedAt:  now.Format(time.RFC3339),
			EndedAt:    now.Format(time.RFC3339),
			PromptHash: spec.PromptHash,
			PromptLen:  spec.PromptLen,
		}
		_ = writeAsyncMeta(meta)
		record := RunRecord{
			ID:         runID,
			Agent:      spec.Agent,
			Role:       spec.Role,
			Model:      spec.Model,
			Cmd:        spec.Cmd,
			Args:       spec.Args,
			Status:     "not_ready",
			ExitCode:   1,
			StartedAt:  meta.StartedAt,
			EndedAt:    meta.EndedAt,
			DurationMs: 0,
			PromptHash: spec.PromptHash,
			PromptLen:  spec.PromptLen,
			Prompt:     spec.Prompt,
			Error:      err.Error(),
		}
		_ = appendRunRecord(record, spec.LogPrompt)
		return map[string]interface{}{
			"run_id": runID,
			"status": "not_ready",
			"agent":  firstNonEmpty(spec.Role, spec.Agent),
			"error":  err.Error(),
		}, nil
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

	cwd := cwdForSpec(spec)
	var beforeStatus map[string]string
	if snapshot, err := gitStatusSnapshot(cwd); err == nil {
		beforeStatus = snapshot
	}

	var startedAt time.Time
	var endedAt time.Time
	var status string
	var exitCode int
	var errMsg string
	lastAttempt := 0
	var changedFiles []string

	for attempt := 1; attempt <= attempts; attempt++ {
		lastAttempt = attempt
		ctx := context.Background()
		var cancel context.CancelFunc
		if spec.TimeoutMs > 0 {
			ctx, cancel = context.WithTimeout(ctx, time.Duration(spec.TimeoutMs)*time.Millisecond)
		} else {
			ctx, cancel = context.WithCancel(ctx)
		}
		activityCh := make(chan struct{}, 1)
		var idleTimedOut atomic.Bool
		stopIdle := startIdleTimer(ctx, time.Duration(spec.IdleTimeoutMs)*time.Millisecond, activityCh, func() {
			idleTimedOut.Store(true)
			cancel()
		})

		cmd := exec.CommandContext(ctx, spec.Cmd, spec.Args...)
		cmd.Stdout = &activityWriter{w: stdoutFile, activityCh: activityCh}
		cmd.Stderr = &activityWriter{w: stderrFile, activityCh: activityCh}
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
			cancel()
			stopIdle()
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
		cancel()
		stopIdle()
		endedAt = time.Now().UTC()
		status, exitCode, errMsg = statusFromErrorWithTimeout(ctx, err, idleTimedOut.Load())
		if snapshot, err := gitStatusSnapshot(cwd); err == nil {
			changedFiles = diffGitStatus(beforeStatus, snapshot)
		}

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
		ID:           runID,
		Status:       status,
		Agent:        spec.Agent,
		Role:         spec.Role,
		Model:        spec.Model,
		Cmd:          spec.Cmd,
		Args:         spec.Args,
		Attempt:      lastAttempt,
		Attempts:     attempts,
		ExitCode:     exitCode,
		Error:        errMsg,
		StartedAt:    startedAt.Format(time.RFC3339),
		EndedAt:      endedAt.Format(time.RFC3339),
		PromptHash:   spec.PromptHash,
		PromptLen:    spec.PromptLen,
		ChangedFiles: changedFiles,
	}
	if current, _, err := loadAsyncMeta(runID); err == nil {
		finalMeta.CancelRequested = current.CancelRequested
		if finalMeta.CancelRequested && finalMeta.Status != "ok" {
			finalMeta.Status = "canceled"
		}
	}
	_ = writeAsyncMeta(finalMeta)

	record := RunRecord{
		ID:           runID,
		Agent:        spec.Agent,
		Role:         spec.Role,
		Model:        spec.Model,
		Cmd:          spec.Cmd,
		Args:         spec.Args,
		Status:       finalMeta.Status,
		ExitCode:     exitCode,
		StartedAt:    finalMeta.StartedAt,
		EndedAt:      finalMeta.EndedAt,
		DurationMs:   endedAt.Sub(startedAt).Milliseconds(),
		PromptHash:   spec.PromptHash,
		PromptLen:    spec.PromptLen,
		Prompt:       spec.Prompt,
		ChangedFiles: changedFiles,
		Error:        errMsg,
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
		"run_id":        runID,
		"status":        status,
		"agent":         firstNonEmpty(meta.Role, meta.Agent),
		"role":          meta.Role,
		"model":         meta.Model,
		"pid":           meta.PID,
		"attempt":       meta.Attempt,
		"attempts":      meta.Attempts,
		"exit_code":     meta.ExitCode,
		"stdout":        strings.TrimSpace(stdout),
		"stderr":        strings.TrimSpace(stderr),
		"error":         meta.Error,
		"started_at":    meta.StartedAt,
		"ended_at":      meta.EndedAt,
		"read_files":    meta.ReadFiles,
		"changed_files": meta.ChangedFiles,
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

func runBatch(prompt, roles, configPath, modelOverride, reasoningOverride string, timeoutMs, idleTimeoutMs int, report progressReporter) (map[string]interface{}, error) {
	if prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if roles == "" {
		return nil, errors.New("Missing roles")
	}
	if roles == "auto" {
		return nil, errors.New("role:auto is not supported; specify roles explicitly")
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

	tasks := []DelegatedTask{}
	if roles == "auto" {
		tasks = nil
	} else {
		tasks = tasksFromRoles(splitList(roles), prompt)
	}
	if len(tasks) == 0 {
		return map[string]interface{}{"status": "no_roles"}, nil
	}
	agentList = []string{}
	seenRoles := map[string]bool{}
	for _, task := range tasks {
		if task.Role != "" && !seenRoles[task.Role] {
			seenRoles[task.Role] = true
			agentList = append(agentList, task.Role)
		}
	}

	for _, task := range tasks {
		role := task.Role
		roleCfg, ok := cfg.Roles[role]
		if !ok {
			results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": "unknown role"})
			continue
		}
		if roleCfg.MaxParallel > 0 && roleCfg.MaxParallel < maxParallel {
			maxParallel = roleCfg.MaxParallel
		}
		models := expandModelEntries(roleCfg, modelOverride, reasoningOverride)
		if len(models) == 0 {
			models = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
		}
		taskPrompt := strings.TrimSpace(task.Prompt)
		if taskPrompt == "" {
			taskPrompt = prompt
		}
		for _, entry := range models {
			spec, err := buildSpecFromRole(cfg, role, taskPrompt, entry.Name, entry.ReasoningEffort, logPrompt)
			if err != nil {
				results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": err.Error()})
				continue
			}
			if timeoutMs > 0 {
				spec.TimeoutMs = timeoutMs
			}
			if idleTimeoutMs > 0 {
				spec.IdleTimeoutMs = idleTimeoutMs
			}
			entries = append(entries, specEntry{agent: role, spec: spec})
		}
	}

	if maxParallel <= 0 {
		maxParallel = 1
	}
	total := len(entries)
	if report != nil {
		report("starting", 0, float64(total))
	}
	sem := make(chan struct{}, maxParallel)

	var wg sync.WaitGroup
	var mu sync.Mutex
	var completed int64
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
				if report != nil {
					done := atomic.AddInt64(&completed, 1)
					report(fmt.Sprintf("finished %s (error)", e.agent), float64(done), float64(total))
				}
				return
			}
			results = append(results, res)
			if report != nil {
				done := atomic.AddInt64(&completed, 1)
				report(fmt.Sprintf("finished %s", e.agent), float64(done), float64(total))
			}
		}(entry)
	}
	wg.Wait()
	if report != nil {
		report("completed", float64(total), float64(total))
	}

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
			if modelOverride != "" || reasoningOverride != "" {
				return "Model overrides apply only to roles mode"
			}
			return nil
		}(),
	}, nil
}

func runBatchAsync(prompt, roles, configPath, modelOverride, reasoningOverride string, timeoutMs, idleTimeoutMs int, report progressReporter) (map[string]interface{}, error) {
	if prompt == "" {
		return nil, errors.New("Missing prompt")
	}
	if roles == "" {
		return nil, errors.New("Missing roles")
	}
	if roles == "auto" {
		return nil, errors.New("role:auto is not supported; specify roles explicitly")
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

	tasks := []DelegatedTask{}
	if roles == "auto" {
		tasks = nil
	} else {
		tasks = tasksFromRoles(splitList(roles), prompt)
	}
	if len(tasks) == 0 {
		return map[string]interface{}{"status": "no_roles"}, nil
	}
	agentList = []string{}
	seenRoles := map[string]bool{}
	for _, task := range tasks {
		if task.Role != "" && !seenRoles[task.Role] {
			seenRoles[task.Role] = true
			agentList = append(agentList, task.Role)
		}
	}

	for _, task := range tasks {
		role := task.Role
		roleCfg, ok := cfg.Roles[role]
		if !ok {
			results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": "unknown role"})
			continue
		}
		models := expandModelEntries(roleCfg, modelOverride, reasoningOverride)
		if len(models) == 0 {
			models = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
		}
		taskPrompt := strings.TrimSpace(task.Prompt)
		if taskPrompt == "" {
			taskPrompt = prompt
		}
		for _, entry := range models {
			spec, err := buildSpecFromRole(cfg, role, taskPrompt, entry.Name, entry.ReasoningEffort, logPrompt)
			if err != nil {
				results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": err.Error()})
				continue
			}
			if timeoutMs > 0 {
				spec.TimeoutMs = timeoutMs
			}
			if idleTimeoutMs > 0 {
				spec.IdleTimeoutMs = idleTimeoutMs
			}
			entries = append(entries, specEntry{agent: role, spec: spec})
		}
	}

	total := len(entries)
	if report != nil {
		report("starting", 0, float64(total))
	}
	started := 0
	for _, entry := range entries {
		res, err := startAsync(entry.spec)
		if err != nil {
			results = append(results, map[string]interface{}{"agent": entry.agent, "status": "error", "error": err.Error()})
			if report != nil {
				started++
				report(fmt.Sprintf("failed %s", entry.agent), float64(started), float64(total))
			}
			continue
		}
		res["agent"] = entry.agent
		results = append(results, res)
		if report != nil {
			started++
			report(fmt.Sprintf("started %s", entry.agent), float64(started), float64(total))
		}
	}
	if report != nil {
		report("completed", float64(total), float64(total))
	}

	return map[string]interface{}{
		"status":  "started",
		"agents":  agentList,
		"results": results,
		"count":   len(results),
	}, nil
}
