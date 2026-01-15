package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

const (
	maxCompletedRuns        = 200
	runtimeQueueResourceURI = "conductor://runtime/queue"
)

var (
	mcpRuntimeMu         sync.Mutex
	mcpRuntime           *Runtime
	mcpRuntimeConfigPath string
	runtimeNotifyMu      sync.Mutex
	runtimeNotify        func(context.Context) error
)

func setRuntimeNotify(notify func(context.Context) error) {
	runtimeNotifyMu.Lock()
	runtimeNotify = notify
	runtimeNotifyMu.Unlock()
}

func notifyRuntimeChanged() {
	runtimeNotifyMu.Lock()
	notify := runtimeNotify
	runtimeNotifyMu.Unlock()
	if notify == nil {
		return
	}
	_ = notify(context.Background())
}

func runtimeQueueSnapshot(runtime *Runtime) map[string]interface{} {
	if runtime == nil {
		return map[string]interface{}{"status": "runtime_not_running"}
	}
	runtime.mu.Lock()
	queued := append([]*RunItem{}, runtime.queue...)
	running := make([]*RunItem, 0, len(runtime.running))
	for _, item := range runtime.running {
		running = append(running, item)
	}
	completed := append([]*RunItem{}, runtime.completed...)
	modeHash := runtime.lastMode
	maxParallel := runtime.cfg.MaxParallel
	runtime.mu.Unlock()

	queueViews := make([]map[string]interface{}, 0, len(queued))
	for _, item := range queued {
		queueViews = append(queueViews, item.view())
	}
	runningViews := make([]map[string]interface{}, 0, len(running))
	for _, item := range running {
		runningViews = append(runningViews, item.view())
	}
	completedViews := make([]map[string]interface{}, 0, len(completed))
	for _, item := range completed {
		completedViews = append(completedViews, item.view())
	}

	return map[string]interface{}{
		"status":          "ok",
		"mode_hash":       modeHash,
		"max_parallel":    maxParallel,
		"queued":          len(queueViews),
		"running":         len(runningViews),
		"completed":       len(completedViews),
		"queue":           queueViews,
		"running_items":   runningViews,
		"completed_items": completedViews,
		"updated_at":      time.Now().UTC().Format(time.RFC3339),
	}
}

func runtimeQueueResourceHandler(ctx context.Context, req *mcp.ReadResourceRequest) (*mcp.ReadResourceResult, error) {
	if req.Params.URI != runtimeQueueResourceURI {
		return nil, fmt.Errorf("unknown resource: %s", req.Params.URI)
	}
	snapshot := runtimeQueueSnapshot(mcpRuntimeSnapshot())
	payload, err := json.Marshal(snapshot)
	if err != nil {
		return nil, err
	}
	return &mcp.ReadResourceResult{
		Contents: []*mcp.ResourceContents{
			{
				URI:      runtimeQueueResourceURI,
				MIMEType: "application/json",
				Text:     string(payload),
			},
		},
	}, nil
}

type RunItem struct {
	ID              string
	Status          string
	Spec            CmdSpec
	ModeHash        string
	RequireApproval bool
	CreatedAt       time.Time
	StartedAt       time.Time
	EndedAt         time.Time
	Error           string
	ExitCode        int
}

type Runtime struct {
	cfg        RuntimeConfig
	queue      []*RunItem
	running    map[string]*RunItem
	completed  []*RunItem
	lastMode   string
	startedAt  time.Time
	shutdownCh chan struct{}
	mu         sync.Mutex
}

func normalizeRuntime(cfg Config) RuntimeConfig {
	r := cfg.Runtime
	if r.MaxParallel <= 0 {
		r.MaxParallel = normalizeDefaults(cfg.Defaults).MaxParallel
	}
	if r.Queue.OnModeChange == "" {
		r.Queue.OnModeChange = "none"
	}
	return r
}

func ensureMcpRuntime(configPath string) (*Runtime, error) {
	resolved := resolveConfigPath(configPath)
	cfg, err := loadConfigOrEmpty(resolved)
	if err != nil {
		return nil, err
	}
	rcfg := normalizeRuntime(cfg)

	mcpRuntimeMu.Lock()
	defer mcpRuntimeMu.Unlock()

	if mcpRuntime != nil && resolved != "" && mcpRuntimeConfigPath != "" && mcpRuntimeConfigPath != resolved {
		if !mcpRuntimeIdle(mcpRuntime) {
			return nil, fmt.Errorf("mcp runtime already running with config %s", mcpRuntimeConfigPath)
		}
		stopMcpRuntimeLocked(mcpRuntime)
		mcpRuntime = nil
	}

	if mcpRuntime == nil {
		mcpRuntime = &Runtime{
			cfg:        rcfg,
			queue:      []*RunItem{},
			running:    map[string]*RunItem{},
			completed:  []*RunItem{},
			startedAt:  time.Now().UTC(),
			shutdownCh: make(chan struct{}),
		}
		mcpRuntimeConfigPath = resolved
		go mcpRuntime.schedulerLoop()
	}

	return mcpRuntime, nil
}

func mcpRuntimeSnapshot() *Runtime {
	mcpRuntimeMu.Lock()
	runtime := mcpRuntime
	mcpRuntimeMu.Unlock()
	return runtime
}

func stopMcpRuntimeLocked(runtime *Runtime) {
	runtime.mu.Lock()
	if runtime.shutdownCh != nil {
		close(runtime.shutdownCh)
		runtime.shutdownCh = nil
	}
	runtime.mu.Unlock()
}

func mcpRuntimeIdle(runtime *Runtime) bool {
	runtime.mu.Lock()
	idle := len(runtime.queue) == 0 && len(runtime.running) == 0
	runtime.mu.Unlock()
	return idle
}

func mcpRuntimeHealthPayload(runtime *Runtime) map[string]interface{} {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	return map[string]interface{}{
		"ok":           true,
		"pid":          os.Getpid(),
		"host":         "mcp",
		"port":         0,
		"started_at":   runtime.startedAt.Format(time.RFC3339),
		"uptime_ms":    time.Since(runtime.startedAt).Milliseconds(),
		"queued":       runtime.countStatus("queued"),
		"awaiting":     runtime.countStatus("awaiting_approval"),
		"running":      len(runtime.running),
		"completed":    len(runtime.completed),
		"max_parallel": runtime.cfg.MaxParallel,
		"mode_hash":    runtime.lastMode,
	}
}

func mcpRuntimeRunStatus(runtime *Runtime, runID string, tail int) (map[string]interface{}, error) {
	if payload, ok := runtime.findQueued(runID); ok {
		return payload, nil
	}
	res, err := getRunStatus(runID, tail)
	if err == nil {
		return res, nil
	}
	if record, ok, _ := findRunRecord(runID); ok {
		return map[string]interface{}{
			"run_id":     record.ID,
			"status":     record.Status,
			"agent":      firstNonEmpty(record.Role, record.Agent),
			"role":       record.Role,
			"model":      record.Model,
			"exit_code":  record.ExitCode,
			"error":      record.Error,
			"started_at": record.StartedAt,
			"ended_at":   record.EndedAt,
		}, nil
	}
	return nil, errors.New("not_found")
}

func mcpRuntimeWait(runtime *Runtime, runID string, timeout time.Duration, tail int) (map[string]interface{}, error) {
	deadline := time.Now().Add(timeout)
	for {
		res, err := mcpRuntimeRunStatus(runtime, runID, tail)
		if err != nil {
			return nil, err
		}
		status, _ := res["status"].(string)
		if status != "running" && status != "queued" && status != "awaiting_approval" {
			return res, nil
		}
		if time.Now().After(deadline) {
			return res, nil
		}
		time.Sleep(500 * time.Millisecond)
	}
}

func mcpRuntimeRun(input RunInput, spec CmdSpec) (map[string]interface{}, error) {
	runtime, err := ensureMcpRuntime(input.Config)
	if err != nil {
		return nil, err
	}
	modeHash := computeModeHash(spec, input.Mode)
	requiresApproval := input.RequireApproval || needsApproval(spec, runtime.cfg)
	item := &RunItem{
		ID:              newRunID(),
		Status:          "queued",
		Spec:            spec,
		ModeHash:        modeHash,
		RequireApproval: requiresApproval,
		CreatedAt:       time.Now().UTC(),
	}
	if requiresApproval {
		item.Status = "awaiting_approval"
	}
	runtime.enqueue(item)
	return map[string]interface{}{
		"run_id":            item.ID,
		"status":            item.Status,
		"mode_hash":         item.ModeHash,
		"approval_required": item.RequireApproval,
	}, nil
}

func mcpRuntimeRunBatch(input BatchInput) (map[string]interface{}, error) {
	runtime, err := ensureMcpRuntime(input.Config)
	if err != nil {
		return nil, err
	}
	configPath := resolveConfigPath(input.Config)
	entries, err := buildBatchSpecs(input, configPath)
	if err != nil {
		return nil, err
	}
	results := []map[string]interface{}{}
	for _, entry := range entries {
		modeHash := computeModeHash(entry.spec, input.Mode)
		requiresApproval := input.RequireApproval || needsApproval(entry.spec, runtime.cfg)
		item := &RunItem{
			ID:              newRunID(),
			Status:          "queued",
			Spec:            entry.spec,
			ModeHash:        modeHash,
			RequireApproval: requiresApproval,
			CreatedAt:       time.Now().UTC(),
		}
		if requiresApproval {
			item.Status = "awaiting_approval"
		}
		runtime.enqueue(item)
		results = append(results, map[string]interface{}{
			"run_id":            item.ID,
			"status":            item.Status,
			"agent":             entry.agent,
			"mode_hash":         item.ModeHash,
			"approval_required": item.RequireApproval,
		})
	}
	return map[string]interface{}{
		"status": "queued",
		"count":  len(results),
		"runs":   results,
	}, nil
}

func (d *Runtime) enqueue(item *RunItem) {
	d.mu.Lock()
	_ = d.handleModeChange(item.ModeHash)
	d.queue = append(d.queue, item)
	d.mu.Unlock()
	notifyRuntimeChanged()
}

func (d *Runtime) handleModeChange(newHash string) bool {
	if newHash == "" {
		return false
	}
	if d.lastMode == "" {
		d.lastMode = newHash
		return false
	}
	if d.lastMode == newHash {
		return false
	}
	changed := false
	switch d.cfg.Queue.OnModeChange {
	case "cancel_pending":
		if d.cancelPendingLocked("mode_changed") {
			changed = true
		}
	case "cancel_running":
		if d.cancelPendingLocked("mode_changed") {
			changed = true
		}
		if d.cancelRunningLocked() {
			changed = true
		}
	}
	d.lastMode = newHash
	return changed
}

func (d *Runtime) cancelPendingLocked(reason string) bool {
	if len(d.queue) == 0 {
		return false
	}
	for _, item := range d.queue {
		item.Status = "canceled"
		item.Error = reason
		item.EndedAt = time.Now().UTC()
		d.appendCompletedLocked(item)
	}
	d.queue = []*RunItem{}
	return true
}

func (d *Runtime) cancelRunningLocked() bool {
	if len(d.running) == 0 {
		return false
	}
	for id, item := range d.running {
		item.Error = "mode_changed"
		_, _ = cancelRun(id, false)
	}
	return true
}

func (d *Runtime) approve(runID string) bool {
	d.mu.Lock()
	changed := false
	for _, item := range d.queue {
		if item.ID == runID && item.Status == "awaiting_approval" {
			item.Status = "queued"
			changed = true
			break
		}
	}
	d.mu.Unlock()
	if changed {
		notifyRuntimeChanged()
	}
	return changed
}

func (d *Runtime) reject(runID string) bool {
	d.mu.Lock()
	changed := false
	for i, item := range d.queue {
		if item.ID == runID {
			item.Status = "rejected"
			item.EndedAt = time.Now().UTC()
			d.queue = append(d.queue[:i], d.queue[i+1:]...)
			d.appendCompletedLocked(item)
			changed = true
			break
		}
	}
	d.mu.Unlock()
	if changed {
		notifyRuntimeChanged()
	}
	return changed
}

func (d *Runtime) cancel(runID string, force bool) string {
	changed := false
	cancelRunning := false
	d.mu.Lock()
	for i, item := range d.queue {
		if item.ID == runID {
			item.Status = "canceled"
			item.EndedAt = time.Now().UTC()
			d.queue = append(d.queue[:i], d.queue[i+1:]...)
			d.appendCompletedLocked(item)
			changed = true
			d.mu.Unlock()
			if changed {
				notifyRuntimeChanged()
			}
			return "canceled"
		}
	}
	if _, ok := d.running[runID]; ok {
		cancelRunning = true
	}
	d.mu.Unlock()
	if cancelRunning {
		res, _ := cancelRun(runID, force)
		if status, ok := res["status"].(string); ok {
			return status
		}
		return "cancelled"
	}
	return "not_found"
}

func (d *Runtime) countStatus(status string) int {
	count := 0
	for _, item := range d.queue {
		if item.Status == status {
			count++
		}
	}
	return count
}

func (d *Runtime) findQueued(runID string) (map[string]interface{}, bool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	for _, item := range d.queue {
		if item.ID == runID {
			return item.view(), true
		}
	}
	return nil, false
}

func (d *Runtime) listRuns(status string, limit int) []map[string]interface{} {
	d.mu.Lock()
	defer d.mu.Unlock()
	out := []map[string]interface{}{}
	for _, item := range d.queue {
		if status == "" || item.Status == status {
			out = append(out, item.view())
		}
	}
	for _, item := range d.running {
		if status == "" || item.Status == status || status == "running" {
			out = append(out, item.view())
		}
	}
	for i := len(d.completed) - 1; i >= 0; i-- {
		item := d.completed[i]
		if status == "" || item.Status == status {
			out = append(out, item.view())
		}
		if limit > 0 && len(out) >= limit {
			break
		}
	}
	return out
}

func (d *Runtime) schedulerLoop() {
	ticker := time.NewTicker(500 * time.Millisecond)
	defer ticker.Stop()
	for {
		select {
		case <-d.shutdownCh:
			return
		case <-ticker.C:
			d.tick()
		}
	}
}

func (d *Runtime) tick() {
	d.syncRunning()
	for {
		d.mu.Lock()
		if len(d.running) >= d.cfg.MaxParallel {
			d.mu.Unlock()
			return
		}
		next := d.popReadyLocked()
		d.mu.Unlock()
		if next == nil {
			return
		}
		_, err := startAsyncWithID(next.ID, next.Spec)
		changed := false
		d.mu.Lock()
		if err != nil {
			next.Status = "error"
			next.Error = err.Error()
			next.EndedAt = time.Now().UTC()
			d.appendCompletedLocked(next)
			changed = true
		} else {
			next.Status = "running"
			next.StartedAt = time.Now().UTC()
			d.running[next.ID] = next
			changed = true
		}
		d.mu.Unlock()
		if changed {
			notifyRuntimeChanged()
		}
	}
}

func (d *Runtime) syncRunning() {
	d.mu.Lock()
	ids := make([]string, 0, len(d.running))
	for id := range d.running {
		ids = append(ids, id)
	}
	d.mu.Unlock()
	for _, id := range ids {
		res, err := getRunStatus(id, 0)
		if err != nil {
			continue
		}
		status, _ := res["status"].(string)
		if status == "running" {
			continue
		}
		changed := false
		d.mu.Lock()
		item, ok := d.running[id]
		if ok {
			item.Status = status
			item.EndedAt = time.Now().UTC()
			if errStr, ok := res["error"].(string); ok {
				item.Error = errStr
			}
			if exit, ok := res["exit_code"].(float64); ok {
				item.ExitCode = int(exit)
			}
			delete(d.running, id)
			d.appendCompletedLocked(item)
			changed = true
		}
		d.mu.Unlock()
		if changed {
			notifyRuntimeChanged()
		}
	}
}

func (d *Runtime) popReadyLocked() *RunItem {
	for i, item := range d.queue {
		if item.Status == "queued" {
			d.queue = append(d.queue[:i], d.queue[i+1:]...)
			return item
		}
	}
	return nil
}

func (d *Runtime) appendCompletedLocked(item *RunItem) {
	d.completed = append(d.completed, item)
	if len(d.completed) > maxCompletedRuns {
		d.completed = d.completed[len(d.completed)-maxCompletedRuns:]
	}
}

func (r *RunItem) view() map[string]interface{} {
	return map[string]interface{}{
		"run_id":            r.ID,
		"status":            r.Status,
		"agent":             firstNonEmpty(r.Spec.Role, r.Spec.Agent),
		"role":              r.Spec.Role,
		"model":             r.Spec.Model,
		"mode_hash":         r.ModeHash,
		"approval_required": r.RequireApproval,
		"created_at":        r.CreatedAt.Format(time.RFC3339),
		"started_at":        formatTime(r.StartedAt),
		"ended_at":          formatTime(r.EndedAt),
		"exit_code":         r.ExitCode,
		"error":             r.Error,
	}
}

func formatTime(t time.Time) string {
	if t.IsZero() {
		return ""
	}
	return t.Format(time.RFC3339)
}

func needsApproval(spec CmdSpec, cfg RuntimeConfig) bool {
	if cfg.Approval.Required {
		return true
	}
	for _, role := range cfg.Approval.Roles {
		if role != "" && role == spec.Role {
			return true
		}
	}
	for _, agent := range cfg.Approval.Agents {
		if agent != "" && agent == spec.Agent {
			return true
		}
	}
	return false
}

func computeModeHash(spec CmdSpec, explicit string) string {
	if explicit != "" {
		return hashString(explicit)
	}
	parts := []string{
		spec.Agent,
		spec.Role,
		spec.Model,
		spec.Reasoning,
		spec.Cmd,
		strings.Join(spec.Args, " "),
		spec.Cwd,
	}
	if len(spec.Env) > 0 {
		keys := make([]string, 0, len(spec.Env))
		for k := range spec.Env {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			parts = append(parts, k+"="+spec.Env[k])
		}
	}
	return hashString(strings.Join(parts, "|"))
}

func hashString(val string) string {
	sum := sha256.Sum256([]byte(val))
	return hex.EncodeToString(sum[:])
}

type specEntry struct {
	agent string
	spec  CmdSpec
}

func buildSpecForInput(input RunInput, configPath string) (CmdSpec, error) {
	configPath = resolveConfigPath(configPath)
	cfg, err := loadConfigOrEmpty(configPath)
	if err != nil {
		return CmdSpec{}, err
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt
	if input.Role == "" {
		return CmdSpec{}, errors.New("missing role")
	}
	cfg, err = loadConfig(configPath)
	if err != nil {
		return CmdSpec{}, err
	}
	if _, ok := cfg.Roles[input.Role]; !ok {
		return CmdSpec{}, fmt.Errorf("%s", unknownRoleMessage(cfg, input.Role))
	}
	spec, err := buildSpecFromRole(cfg, input.Role, input.Prompt, input.Model, input.Reasoning, logPrompt)
	if err != nil {
		return CmdSpec{}, err
	}
	if input.IdleTimeoutMs > 0 {
		spec.IdleTimeoutMs = input.IdleTimeoutMs
	}
	return spec, nil
}

func buildBatchSpecs(input BatchInput, configPath string) ([]specEntry, error) {
	configPath = resolveConfigPath(configPath)
	if input.Roles == "" {
		return nil, errors.New("missing roles")
	}

	results := []specEntry{}
	agentList := []string{}

	cfg, err := loadConfig(configPath)
	if err != nil {
		return nil, err
	}
	defaults := normalizeDefaults(cfg.Defaults)
	logPrompt := defaults.LogPrompt

	tasks := []DelegatedTask{}
	tasks = tasksFromRoles(splitList(input.Roles), input.Prompt)
	if len(tasks) == 0 {
		return nil, errors.New("no_roles")
	}
	missing := []string{}
	seenMissing := map[string]bool{}
	for _, task := range tasks {
		if task.Role == "" {
			continue
		}
		if _, ok := cfg.Roles[task.Role]; !ok {
			if !seenMissing[task.Role] {
				seenMissing[task.Role] = true
				missing = append(missing, task.Role)
			}
		}
	}
	if len(missing) > 0 {
		return nil, fmt.Errorf("Unknown role(s): %s (available: %s)", strings.Join(missing, ", "), strings.Join(roleNames(cfg), ", "))
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
			continue
		}
		models := expandModelEntries(roleCfg, input.Model, input.Reasoning)
		if len(models) == 0 {
			models = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
		}
		taskPrompt := strings.TrimSpace(task.Prompt)
		if taskPrompt == "" {
			taskPrompt = input.Prompt
		}
		for _, entry := range models {
			spec, err := buildSpecFromRole(cfg, role, taskPrompt, entry.Name, entry.ReasoningEffort, logPrompt)
			if err != nil {
				continue
			}
			if input.IdleTimeoutMs > 0 {
				spec.IdleTimeoutMs = input.IdleTimeoutMs
			}
			results = append(results, specEntry{agent: role, spec: spec})
		}
	}
	if len(results) == 0 {
		return nil, errors.New("no_roles")
	}
	_ = agentList
	return results, nil
}
