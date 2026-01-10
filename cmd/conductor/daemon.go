package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

const maxCompletedRuns = 200

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

type Daemon struct {
	cfg        DaemonConfig
	queue      []*RunItem
	running    map[string]*RunItem
	completed  []*RunItem
	lastMode   string
	startedAt  time.Time
	shutdownCh chan struct{}
	mu         sync.Mutex
}

func runDaemon(args []string) int {
	fs := flag.NewFlagSet("daemon", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	mode := fs.String("mode", "start", "start|stop|status")
	detach := fs.Bool("detach", false, "run in background")
	host := fs.String("host", "", "daemon host")
	port := fs.Int("port", 0, "daemon port")
	configPath := fs.String("config", "", "config path")
	if err := fs.Parse(args); err != nil {
		fmt.Println(daemonHelp())
		return 1
	}

	cfgPath := resolveConfigPath(*configPath)
	cfg, _ := loadConfigOrEmpty(cfgPath)
	dcfg := normalizeDaemon(cfg)
	if *host != "" {
		dcfg.Host = *host
	}
	if *port > 0 {
		dcfg.Port = *port
	}

	switch *mode {
	case "start":
		if *detach {
			return startDaemonDetached(dcfg, cfgPath)
		}
		return runDaemonServer(dcfg, cfgPath)
	case "stop":
		url := resolveDaemonURL(cfgPath)
		if url == "" {
			fmt.Println("Daemon not running.")
			return 1
		}
		if _, err := daemonPostJSON(url, "/shutdown", map[string]interface{}{}); err != nil {
			fmt.Println(err.Error())
			return 1
		}
		fmt.Println("Daemon stopped.")
		return 0
	case "status":
		url := resolveDaemonURL(cfgPath)
		if url == "" {
			fmt.Println("Daemon not running.")
			return 1
		}
		res, err := daemonStatus(url)
		if err != nil {
			fmt.Println(err.Error())
			return 1
		}
		printJSON(res)
		return 0
	default:
		fmt.Println(daemonHelp())
		return 1
	}
}

func daemonHelp() string {
	return `conductor daemon

Usage:
  conductor daemon --mode start|stop|status [--detach] [--host HOST] [--port PORT] [--config PATH]
`
}

func startDaemonDetached(cfg DaemonConfig, configPath string) int {
	exe, err := os.Executable()
	if err != nil {
		fmt.Println(err.Error())
		return 1
	}
	logPath := daemonLogPath()
	_ = os.MkdirAll(filepath.Dir(logPath), 0o755)
	logFile, err := os.OpenFile(logPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		fmt.Println(err.Error())
		return 1
	}
	defer logFile.Close()

	args := []string{"daemon", "--mode", "start", "--host", cfg.Host, "--port", strconv.Itoa(cfg.Port), "--config", configPath}
	cmd := exec.Command(exe, args...)
	cmd.Stdout = logFile
	cmd.Stderr = logFile
	if err := cmd.Start(); err != nil {
		fmt.Println(err.Error())
		return 1
	}
	_ = cmd.Process.Release()
	fmt.Printf("Daemon started (pid %d).\n", cmd.Process.Pid)
	return 0
}

func runDaemonServer(cfg DaemonConfig, configPath string) int {
	d := &Daemon{
		cfg:        cfg,
		queue:      []*RunItem{},
		running:    map[string]*RunItem{},
		completed:  []*RunItem{},
		startedAt:  time.Now().UTC(),
		shutdownCh: make(chan struct{}),
	}

	state := newDaemonState(cfg)
	if err := writeDaemonState(state); err != nil {
		fmt.Println(err.Error())
		return 1
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/health", d.handleHealth)
	mux.HandleFunc("/run", d.handleRun(configPath))
	mux.HandleFunc("/run_batch", d.handleRunBatch(configPath))
	mux.HandleFunc("/run/", d.handleRunStatus)
	mux.HandleFunc("/runs", d.handleRuns)
	mux.HandleFunc("/approve", d.handleApprove)
	mux.HandleFunc("/reject", d.handleReject)
	mux.HandleFunc("/approvals", d.handleApprovals)
	mux.HandleFunc("/cancel", d.handleCancel)
	mux.HandleFunc("/shutdown", d.handleShutdown)

	server := &http.Server{
		Addr:    fmt.Sprintf("%s:%d", cfg.Host, cfg.Port),
		Handler: mux,
	}

	go d.schedulerLoop()

	fmt.Printf("Conductor daemon listening on %s\n", server.Addr)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		fmt.Println(err.Error())
		return 1
	}
	return 0
}

func (d *Daemon) handleHealth(w http.ResponseWriter, r *http.Request) {
	d.mu.Lock()
	defer d.mu.Unlock()
	payload := map[string]interface{}{
		"ok":           true,
		"pid":          os.Getpid(),
		"host":         d.cfg.Host,
		"port":         d.cfg.Port,
		"started_at":   d.startedAt.Format(time.RFC3339),
		"uptime_ms":    time.Since(d.startedAt).Milliseconds(),
		"queued":       d.countStatus("queued"),
		"awaiting":     d.countStatus("awaiting_approval"),
		"running":      len(d.running),
		"completed":    len(d.completed),
		"max_parallel": d.cfg.MaxParallel,
		"mode_hash":    d.lastMode,
	}
	writeJSON(w, http.StatusOK, payload)
}

func (d *Daemon) handleRun(configPath string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
			return
		}
		var input RunInput
		if err := json.NewDecoder(r.Body).Decode(&input); err != nil {
			http.Error(w, "Invalid JSON", http.StatusBadRequest)
			return
		}
		if input.Prompt == "" || input.Role == "" {
			http.Error(w, "Missing prompt or role", http.StatusBadRequest)
			return
		}
		spec, err := buildSpecForInput(input, configPath)
		if err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		modeHash := computeModeHash(spec, input.Mode)
		requiresApproval := input.RequireApproval || needsApproval(spec, d.cfg)
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
		d.enqueue(item)
		payload := map[string]interface{}{
			"run_id":            item.ID,
			"status":            item.Status,
			"mode_hash":         item.ModeHash,
			"approval_required": item.RequireApproval,
		}
		writeJSON(w, http.StatusOK, payload)
	}
}

func (d *Daemon) handleRunBatch(configPath string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
			return
		}
		var input BatchInput
		if err := json.NewDecoder(r.Body).Decode(&input); err != nil {
			http.Error(w, "Invalid JSON", http.StatusBadRequest)
			return
		}
		if input.Prompt == "" || input.Roles == "" {
			http.Error(w, "Missing prompt or roles", http.StatusBadRequest)
			return
		}
		entries, err := buildBatchSpecs(input, configPath)
		if err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		results := []map[string]interface{}{}
		for _, entry := range entries {
			modeHash := computeModeHash(entry.spec, input.Mode)
			requiresApproval := input.RequireApproval || needsApproval(entry.spec, d.cfg)
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
			d.enqueue(item)
			results = append(results, map[string]interface{}{
				"run_id":            item.ID,
				"status":            item.Status,
				"agent":             entry.agent,
				"mode_hash":         item.ModeHash,
				"approval_required": item.RequireApproval,
			})
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{
			"status": "queued",
			"count":  len(results),
			"runs":   results,
		})
	}
}

func (d *Daemon) handleRunStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	runID := strings.TrimPrefix(r.URL.Path, "/run/")
	if runID == "" {
		http.Error(w, "Missing run_id", http.StatusBadRequest)
		return
	}
	tail := 4000
	if t := r.URL.Query().Get("tail"); t != "" {
		if val, err := strconv.Atoi(t); err == nil && val >= 0 {
			tail = val
		}
	}
	if payload, ok := d.findQueued(runID); ok {
		writeJSON(w, http.StatusOK, payload)
		return
	}
	res, err := getRunStatus(runID, tail)
	if err == nil {
		writeJSON(w, http.StatusOK, res)
		return
	}
	if record, ok, _ := findRunRecord(runID); ok {
		writeJSON(w, http.StatusOK, map[string]interface{}{
			"run_id":     record.ID,
			"status":     record.Status,
			"agent":      firstNonEmpty(record.Role, record.Agent),
			"role":       record.Role,
			"model":      record.Model,
			"exit_code":  record.ExitCode,
			"error":      record.Error,
			"started_at": record.StartedAt,
			"ended_at":   record.EndedAt,
		})
		return
	}
	http.Error(w, "not_found", http.StatusNotFound)
}

func (d *Daemon) handleRuns(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	status := r.URL.Query().Get("status")
	limit := 0
	if val := r.URL.Query().Get("limit"); val != "" {
		if parsed, err := strconv.Atoi(val); err == nil && parsed > 0 {
			limit = parsed
		}
	}
	runs := d.listRuns(status, limit)
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"count": len(runs),
		"runs":  runs,
	})
}

func (d *Daemon) handleApprovals(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	runs := d.listRuns("awaiting_approval", 0)
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"count": len(runs),
		"runs":  runs,
	})
}

func (d *Daemon) handleApprove(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	var payload map[string]string
	_ = json.NewDecoder(r.Body).Decode(&payload)
	runID := payload["run_id"]
	if runID == "" {
		http.Error(w, "Missing run_id", http.StatusBadRequest)
		return
	}
	if ok := d.approve(runID); !ok {
		http.Error(w, "not_found", http.StatusNotFound)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"run_id": runID, "status": "queued"})
}

func (d *Daemon) handleReject(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	var payload map[string]string
	_ = json.NewDecoder(r.Body).Decode(&payload)
	runID := payload["run_id"]
	if runID == "" {
		http.Error(w, "Missing run_id", http.StatusBadRequest)
		return
	}
	if ok := d.reject(runID); !ok {
		http.Error(w, "not_found", http.StatusNotFound)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"run_id": runID, "status": "rejected"})
}

func (d *Daemon) handleCancel(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	var payload map[string]interface{}
	_ = json.NewDecoder(r.Body).Decode(&payload)
	runID, _ := payload["run_id"].(string)
	force, _ := payload["force"].(bool)
	if runID == "" {
		http.Error(w, "Missing run_id", http.StatusBadRequest)
		return
	}
	status := d.cancel(runID, force)
	writeJSON(w, http.StatusOK, map[string]interface{}{"run_id": runID, "status": status})
}

func (d *Daemon) handleShutdown(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method Not Allowed", http.StatusMethodNotAllowed)
		return
	}
	d.mu.Lock()
	if d.shutdownCh != nil {
		close(d.shutdownCh)
		d.shutdownCh = nil
	}
	d.mu.Unlock()
	writeJSON(w, http.StatusOK, map[string]interface{}{"status": "stopping"})
	go func() {
		time.Sleep(200 * time.Millisecond)
		_ = os.Remove(daemonStatePath())
		os.Exit(0)
	}()
}

func (d *Daemon) enqueue(item *RunItem) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.handleModeChange(item.ModeHash)
	d.queue = append(d.queue, item)
}

func (d *Daemon) handleModeChange(newHash string) {
	if newHash == "" {
		return
	}
	if d.lastMode == "" {
		d.lastMode = newHash
		return
	}
	if d.lastMode == newHash {
		return
	}
	switch d.cfg.Queue.OnModeChange {
	case "cancel_pending":
		d.cancelPendingLocked("mode_changed")
	case "cancel_running":
		d.cancelPendingLocked("mode_changed")
		d.cancelRunningLocked()
	}
	d.lastMode = newHash
}

func (d *Daemon) cancelPendingLocked(reason string) {
	if len(d.queue) == 0 {
		return
	}
	for _, item := range d.queue {
		item.Status = "canceled"
		item.Error = reason
		item.EndedAt = time.Now().UTC()
		d.appendCompletedLocked(item)
	}
	d.queue = []*RunItem{}
}

func (d *Daemon) cancelRunningLocked() {
	for id, item := range d.running {
		item.Error = "mode_changed"
		_, _ = cancelRun(id, false)
	}
}

func (d *Daemon) approve(runID string) bool {
	d.mu.Lock()
	defer d.mu.Unlock()
	for _, item := range d.queue {
		if item.ID == runID && item.Status == "awaiting_approval" {
			item.Status = "queued"
			return true
		}
	}
	return false
}

func (d *Daemon) reject(runID string) bool {
	d.mu.Lock()
	defer d.mu.Unlock()
	for i, item := range d.queue {
		if item.ID == runID {
			item.Status = "rejected"
			item.EndedAt = time.Now().UTC()
			d.queue = append(d.queue[:i], d.queue[i+1:]...)
			d.appendCompletedLocked(item)
			return true
		}
	}
	return false
}

func (d *Daemon) cancel(runID string, force bool) string {
	d.mu.Lock()
	for i, item := range d.queue {
		if item.ID == runID {
			item.Status = "canceled"
			item.EndedAt = time.Now().UTC()
			d.queue = append(d.queue[:i], d.queue[i+1:]...)
			d.appendCompletedLocked(item)
			d.mu.Unlock()
			return "canceled"
		}
	}
	if _, ok := d.running[runID]; ok {
		d.mu.Unlock()
		res, _ := cancelRun(runID, force)
		if status, ok := res["status"].(string); ok {
			return status
		}
		return "cancelled"
	}
	d.mu.Unlock()
	return "not_found"
}

func (d *Daemon) countStatus(status string) int {
	count := 0
	for _, item := range d.queue {
		if item.Status == status {
			count++
		}
	}
	return count
}

func (d *Daemon) findQueued(runID string) (map[string]interface{}, bool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	for _, item := range d.queue {
		if item.ID == runID {
			return item.view(), true
		}
	}
	return nil, false
}

func (d *Daemon) listRuns(status string, limit int) []map[string]interface{} {
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

func (d *Daemon) schedulerLoop() {
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

func (d *Daemon) tick() {
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
		d.mu.Lock()
		if err != nil {
			next.Status = "error"
			next.Error = err.Error()
			next.EndedAt = time.Now().UTC()
			d.appendCompletedLocked(next)
		} else {
			next.Status = "running"
			next.StartedAt = time.Now().UTC()
			d.running[next.ID] = next
		}
		d.mu.Unlock()
	}
}

func (d *Daemon) syncRunning() {
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
		}
		d.mu.Unlock()
	}
}

func (d *Daemon) popReadyLocked() *RunItem {
	for i, item := range d.queue {
		if item.Status == "queued" {
			d.queue = append(d.queue[:i], d.queue[i+1:]...)
			return item
		}
	}
	return nil
}

func (d *Daemon) appendCompletedLocked(item *RunItem) {
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

func writeJSON(w http.ResponseWriter, status int, payload map[string]interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(payload)
}

func needsApproval(spec CmdSpec, cfg DaemonConfig) bool {
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
	if input.TimeoutMs > 0 {
		spec.TimeoutMs = input.TimeoutMs
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

	var cfg Config
	var err error
	cfg, err = loadConfig(configPath)
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
			if input.TimeoutMs > 0 {
				spec.TimeoutMs = input.TimeoutMs
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
