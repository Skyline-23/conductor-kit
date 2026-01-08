package main

import (
	"encoding/json"
	"errors"
	"flag"
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

type TaskMeta struct {
	ID        string   `json:"id"`
	Agent     string   `json:"agent"`
	Role      string   `json:"role"`
	Model     string   `json:"model"`
	Reasoning string   `json:"reasoning"`
	Cmd       string   `json:"cmd"`
	Args      []string `json:"args"`
	Prompt    string   `json:"prompt"`
	PID       int      `json:"pid"`
	StartedAt string   `json:"startedAt"`
}

type CmdSpec struct {
	Agent     string
	Role      string
	Model     string
	Reasoning string
	Cmd       string
	Args      []string
}

func runBackgroundTask(args []string) int {
	fs := flag.NewFlagSet("background-task", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	agent := fs.String("agent", "", "agent")
	role := fs.String("role", "", "role")
	prompt := fs.String("prompt", "", "prompt")
	configPath := fs.String("config", getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")), "config path")
	modelOverride := fs.String("model", "", "model override")
	reasoningOverride := fs.String("reasoning", "", "reasoning override")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}
	if *prompt == "" || (*agent == "" && *role == "") {
		fmt.Println("Missing --agent/--role or --prompt.")
		return 1
	}

	var spec CmdSpec
	var err error
	if *role != "" {
		spec, err = buildSpecFromRole(*configPath, *role, *prompt, *modelOverride, *reasoningOverride)
	} else {
		spec, err = buildSpecFromAgent(*agent, *prompt)
	}
	if err != nil {
		fmt.Println(err.Error())
		return 1
	}

	res, err := startTask(spec, *prompt)
	if err != nil {
		fmt.Println(err.Error())
		return 1
	}
	out, _ := jsonMarshal(res)
	fmt.Println(string(out))
	return 0
}

func buildSpecFromAgent(agent, prompt string) (CmdSpec, error) {
	mapping := map[string]CmdSpec{
		"claude": {Agent: "claude", Cmd: "claude", Args: []string{"-p", prompt}},
		"codex":  {Agent: "codex", Cmd: "codex", Args: []string{"exec", prompt}},
		"gemini": {Agent: "gemini", Cmd: "gemini", Args: []string{prompt}},
	}
	if spec, ok := mapping[agent]; ok {
		return spec, nil
	}
	return CmdSpec{}, fmt.Errorf("Unknown agent: %s", agent)
}

func buildSpecFromRole(configPath, role, prompt, modelOverride, reasoningOverride string) (CmdSpec, error) {
	cfg, err := loadConfig(configPath)
	if err != nil {
		return CmdSpec{}, err
	}
	roleCfg, ok := cfg.Roles[role]
	if !ok {
		return CmdSpec{}, fmt.Errorf("Missing role config for: %s", role)
	}
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
	return CmdSpec{Agent: roleCfg.CLI, Role: role, Model: model, Reasoning: reasoning, Cmd: roleCfg.CLI, Args: args}, nil
}

func indexOf(items []string, target string) int {
	for i, v := range items {
		if v == target {
			return i
		}
	}
	return -1
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

func startTask(spec CmdSpec, prompt string) (map[string]interface{}, error) {
	if !isCommandAvailable(spec.Cmd) {
		return nil, fmt.Errorf("Missing CLI on PATH: %s", spec.Cmd)
	}
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	tasksDir := filepath.Join(baseDir, "tasks")
	_ = os.MkdirAll(tasksDir, 0o755)

	taskID := fmt.Sprintf("task-%d-%04x", time.Now().UnixMilli(), rand.Intn(65535))
	taskDir := filepath.Join(tasksDir, taskID)
	if err := os.MkdirAll(taskDir, 0o755); err != nil {
		return nil, err
	}

	stdoutPath := filepath.Join(taskDir, "stdout.log")
	stderrPath := filepath.Join(taskDir, "stderr.log")
	metaPath := filepath.Join(taskDir, "meta.json")

	stdoutFile, err := os.OpenFile(stdoutPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return nil, err
	}
	stderrFile, err := os.OpenFile(stderrPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return nil, err
	}

	cmd := exec.Command(spec.Cmd, spec.Args...)
	cmd.Stdout = stdoutFile
	cmd.Stderr = stderrFile
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}

	if err := cmd.Start(); err != nil {
		return nil, err
	}
	_ = cmd.Process.Release()
	_ = stdoutFile.Close()
	_ = stderrFile.Close()

	meta := TaskMeta{
		ID:        taskID,
		Agent:     spec.Agent,
		Role:      spec.Role,
		Model:     spec.Model,
		Reasoning: spec.Reasoning,
		Cmd:       spec.Cmd,
		Args:      spec.Args,
		Prompt:    prompt,
		PID:       cmd.Process.Pid,
		StartedAt: time.Now().Format(time.RFC3339),
	}
	metaBytes, _ := jsonMarshal(meta)
	_ = os.WriteFile(metaPath, metaBytes, 0o644)

	return map[string]interface{}{
		"task_id": taskID,
		"status":  "running",
		"agent":   firstNonEmpty(spec.Role, spec.Agent),
		"model":   spec.Model,
		"note":    "Use conductor-background-output --task-id <id> to fetch results",
	}, nil
}

func firstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return ""
}

func runBackgroundOutput(args []string) int {
	fs := flag.NewFlagSet("background-output", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	taskID := fs.String("task-id", "", "task id")
	block := fs.Bool("block", false, "block")
	timeout := fs.Int("timeout", 120000, "timeout ms")
	tail := fs.Int("tail", 4000, "tail bytes")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}
	if *taskID == "" {
		fmt.Println("Missing --task-id.")
		return 1
	}

	res, err := getTaskOutput(*taskID, *block, time.Duration(*timeout)*time.Millisecond, *tail)
	if err != nil {
		fmt.Println(err.Error())
		return 1
	}
	out, _ := jsonMarshal(res)
	fmt.Println(string(out))
	return 0
}

func getTaskOutput(taskID string, block bool, timeout time.Duration, tailBytes int) (map[string]interface{}, error) {
	meta, taskDir, err := loadTaskMeta(taskID)
	if err != nil {
		return nil, err
	}

	deadline := time.Now().Add(timeout)
	for {
		running := isRunning(meta.PID)
		if !block || !running || time.Now().After(deadline) {
			stdout := readTail(filepath.Join(taskDir, "stdout.log"), tailBytes)
			stderr := readTail(filepath.Join(taskDir, "stderr.log"), tailBytes)
			return map[string]interface{}{
				"task_id": taskID,
				"agent":   firstNonEmpty(meta.Role, meta.Agent),
				"status":  map[bool]string{true: "running", false: "done"}[running],
				"stdout":  strings.TrimSpace(stdout),
				"stderr":  strings.TrimSpace(stderr),
			}, nil
		}
		time.Sleep(1 * time.Second)
	}
}

func loadTaskMeta(taskID string) (TaskMeta, string, error) {
	var meta TaskMeta
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	taskDir := filepath.Join(baseDir, "tasks", taskID)
	metaPath := filepath.Join(taskDir, "meta.json")
	data, err := os.ReadFile(metaPath)
	if err != nil {
		return meta, "", errors.New("Unknown task: " + taskID)
	}
	if err := jsonUnmarshal(data, &meta); err != nil {
		return meta, "", err
	}
	return meta, taskDir, nil
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
	_, _ = file.Seek(start, io.SeekStart)
	data, _ := io.ReadAll(file)
	return string(data)
}

func runBackgroundCancel(args []string) int {
	fs := flag.NewFlagSet("background-cancel", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	all := fs.Bool("all", false, "cancel all")
	taskID := fs.String("task-id", "", "task id")
	force := fs.Bool("force", false, "force")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}

	res := map[string]interface{}{}
	if *all {
		res["cancelled"] = cancelAllTasks(*force)
	} else if *taskID != "" {
		res["cancelled"] = []map[string]interface{}{cancelTask(*taskID, *force)}
	} else {
		fmt.Println("Missing --task-id or --all.")
		return 1
	}

	out, _ := jsonMarshal(res)
	fmt.Println(string(out))
	return 0
}

func cancelTask(taskID string, force bool) map[string]interface{} {
	meta, _, err := loadTaskMeta(taskID)
	if err != nil {
		return map[string]interface{}{"task_id": taskID, "status": "not_found"}
	}
	status := "cancelled"
	if meta.PID <= 0 {
		status = "not_running"
		return map[string]interface{}{"task_id": taskID, "status": status}
	}
	if err := syscall.Kill(meta.PID, syscall.SIGTERM); err != nil {
		status = "not_running"
	}
	if force {
		time.Sleep(500 * time.Millisecond)
		_ = syscall.Kill(meta.PID, syscall.SIGKILL)
	}
	return map[string]interface{}{"task_id": taskID, "status": status}
}

func cancelAllTasks(force bool) []map[string]interface{} {
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	tasksDir := filepath.Join(baseDir, "tasks")
	entries, err := os.ReadDir(tasksDir)
	if err != nil {
		return []map[string]interface{}{}
	}
	results := []map[string]interface{}{}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		results = append(results, cancelTask(entry.Name(), force))
	}
	return results
}

func runBackgroundBatch(args []string) int {
	fs := flag.NewFlagSet("background-batch", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	prompt := fs.String("prompt", "", "prompt")
	roles := fs.String("roles", "", "roles")
	agents := fs.String("agents", "", "agents")
	configPath := fs.String("config", getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")), "config")
	modelOverride := fs.String("model", "", "model override")
	reasoningOverride := fs.String("reasoning", "", "reasoning override")
	if err := fs.Parse(args); err != nil {
		fmt.Println("Invalid flags.")
		return 1
	}
	if *prompt == "" {
		fmt.Println("Missing --prompt.")
		return 1
	}

	results := []map[string]interface{}{}
	agentList := []string{}

	if *roles != "" {
		cfg, err := loadConfig(*configPath)
		if err != nil {
			printJSON(map[string]interface{}{
				"status": "missing_config",
				"note":   "Role-based batch requested but config is missing or invalid.",
				"config": *configPath,
			})
			return 1
		}
		roleNames := []string{}
		if *roles == "auto" {
			for k := range cfg.Roles {
				roleNames = append(roleNames, k)
			}
		} else {
			roleNames = splitList(*roles)
		}
		if len(roleNames) == 0 {
			printJSON(map[string]interface{}{"status": "no_roles"})
			return 0
		}
		agentList = roleNames

		var wg sync.WaitGroup
		var mu sync.Mutex
		for _, role := range roleNames {
			roleCfg := cfg.Roles[role]
			entries := expandModelEntries(roleCfg, *modelOverride, *reasoningOverride)
			if len(entries) == 0 {
				entries = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
			}
			for _, entry := range entries {
				wg.Add(1)
				go func(r string, e ModelEntry) {
					defer wg.Done()
					spec, err := buildSpecFromRole(*configPath, r, *prompt, e.Name, e.ReasoningEffort)
					if err != nil {
						mu.Lock()
						results = append(results, map[string]interface{}{"agent": r, "status": "error", "error": err.Error()})
						mu.Unlock()
						return
					}
					res, err := startTask(spec, *prompt)
					mu.Lock()
					if err != nil {
						results = append(results, map[string]interface{}{"agent": r, "status": "error", "error": err.Error(), "model": spec.Model})
					} else {
						res["agent"] = r
						res["model"] = spec.Model
						results = append(results, res)
					}
					mu.Unlock()
				}(role, entry)
			}
		}
		wg.Wait()
	} else {
		agentArg := *agents
		if agentArg == "" {
			agentArg = "auto"
		}
		if agentArg == "auto" {
			agentList = detectAgents()
		} else {
			agentList = splitList(agentArg)
		}
		if len(agentList) == 0 {
			printJSON(map[string]interface{}{
				"status": "no_agents",
				"note":   "No CLI agents detected. Install codex/claude/gemini or pass --agents.",
			})
			return 0
		}
		var wg sync.WaitGroup
		var mu sync.Mutex
		for _, agent := range agentList {
			wg.Add(1)
			go func(a string) {
				defer wg.Done()
				spec, err := buildSpecFromAgent(a, *prompt)
				if err != nil {
					mu.Lock()
					results = append(results, map[string]interface{}{"agent": a, "status": "error", "error": err.Error()})
					mu.Unlock()
					return
				}
				res, err := startTask(spec, *prompt)
				mu.Lock()
				if err != nil {
					results = append(results, map[string]interface{}{"agent": a, "status": "error", "error": err.Error()})
				} else {
					res["agent"] = a
					results = append(results, res)
				}
				mu.Unlock()
			}(agent)
		}
		wg.Wait()
	}

	printJSON(map[string]interface{}{
		"status":  "started",
		"agents":  agentList,
		"results": results,
		"note":    "Use conductor-background-output --task-id <id> to fetch results",
		"warning": func() interface{} { if *roles == "" && (*modelOverride != "" || *reasoningOverride != "") { return "Model overrides apply only to --roles mode" }; return nil }(),
	})
	return 0
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

func runBatchArgs(args []string) (map[string]interface{}, error) {
	fs := flag.NewFlagSet("background-batch", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	prompt := fs.String("prompt", "", "prompt")
	roles := fs.String("roles", "", "roles")
	agents := fs.String("agents", "", "agents")
	configPath := fs.String("config", getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json")), "config")
	modelOverride := fs.String("model", "", "model override")
	reasoningOverride := fs.String("reasoning", "", "reasoning override")
	if err := fs.Parse(args); err != nil {
		return nil, err
	}
	if *prompt == "" {
		return nil, errors.New("Missing prompt")
	}

	results := []map[string]interface{}{}
	agentList := []string{}

	if *roles != "" {
		cfg, err := loadConfig(*configPath)
		if err != nil {
			return map[string]interface{}{"status": "missing_config", "note": "Role-based batch requested but config is missing or invalid.", "config": *configPath}, nil
		}
		roleNames := []string{}
		if *roles == "auto" {
			for k := range cfg.Roles {
				roleNames = append(roleNames, k)
			}
		} else {
			roleNames = splitList(*roles)
		}
		if len(roleNames) == 0 {
			return map[string]interface{}{"status": "no_roles"}, nil
		}
		agentList = roleNames

		var wg sync.WaitGroup
		var mu sync.Mutex
		for _, role := range roleNames {
			roleCfg := cfg.Roles[role]
			entries := expandModelEntries(roleCfg, *modelOverride, *reasoningOverride)
			if len(entries) == 0 {
				entries = []ModelEntry{{Name: roleCfg.Model, ReasoningEffort: roleCfg.Reasoning}}
			}
			for _, entry := range entries {
				wg.Add(1)
				go func(r string, e ModelEntry) {
					defer wg.Done()
					spec, err := buildSpecFromRole(*configPath, r, *prompt, e.Name, e.ReasoningEffort)
					if err != nil {
						mu.Lock()
						results = append(results, map[string]interface{}{"agent": r, "status": "error", "error": err.Error()})
						mu.Unlock()
						return
					}
					res, err := startTask(spec, *prompt)
					mu.Lock()
					if err != nil {
						results = append(results, map[string]interface{}{"agent": r, "status": "error", "error": err.Error(), "model": spec.Model})
					} else {
						res["agent"] = r
						res["model"] = spec.Model
						results = append(results, res)
					}
					mu.Unlock()
				}(role, entry)
			}
		}
		wg.Wait()
	} else {
		agentArg := *agents
		if agentArg == "" {
			agentArg = "auto"
		}
		if agentArg == "auto" {
			agentList = detectAgents()
		} else {
			agentList = splitList(agentArg)
		}
		if len(agentList) == 0 {
			return map[string]interface{}{"status": "no_agents", "note": "No CLI agents detected. Install codex/claude/gemini or pass --agents."}, nil
		}
		var wg sync.WaitGroup
		var mu sync.Mutex
		for _, agent := range agentList {
			wg.Add(1)
			go func(a string) {
				defer wg.Done()
				spec, err := buildSpecFromAgent(a, *prompt)
				if err != nil {
					mu.Lock()
					results = append(results, map[string]interface{}{"agent": a, "status": "error", "error": err.Error()})
					mu.Unlock()
					return
				}
				res, err := startTask(spec, *prompt)
				mu.Lock()
				if err != nil {
					results = append(results, map[string]interface{}{"agent": a, "status": "error", "error": err.Error()})
				} else {
					res["agent"] = a
					results = append(results, res)
				}
				mu.Unlock()
			}(agent)
		}
		wg.Wait()
	}

	return map[string]interface{}{
		"status":  "started",
		"agents":  agentList,
		"results": results,
		"note":    "Use conductor-background-output --task-id <id> to fetch results",
		"warning": func() interface{} { if *roles == "" && (*modelOverride != "" || *reasoningOverride != "") { return "Model overrides apply only to --roles mode" }; return nil }(),
	}, nil
}

func jsonMarshal(v interface{}) ([]byte, error) {
	return json.MarshalIndent(v, "", "  ")
}

func jsonUnmarshal(data []byte, v interface{}) error {
	return json.Unmarshal(data, v)
}
