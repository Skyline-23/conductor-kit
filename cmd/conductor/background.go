package main

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"
)

type CmdSpec struct {
	Agent     string
	Role      string
	Model     string
	Reasoning string
	Cmd       string
	Args      []string
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

func runCommand(spec CmdSpec, timeout time.Duration) (map[string]interface{}, error) {
	if !isCommandAvailable(spec.Cmd) {
		return nil, fmt.Errorf("Missing CLI on PATH: %s", spec.Cmd)
	}

	ctx := context.Background()
	var cancel context.CancelFunc
	if timeout > 0 {
		ctx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}

	start := time.Now()
	cmd := exec.CommandContext(ctx, spec.Cmd, spec.Args...)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err := cmd.Run()
	duration := time.Since(start).Milliseconds()

	status := "ok"
	exitCode := 0
	if err != nil {
		switch {
		case errors.Is(ctx.Err(), context.DeadlineExceeded):
			status = "timeout"
		case errors.Is(ctx.Err(), context.Canceled):
			status = "canceled"
		default:
			status = "error"
		}
		if exitErr, ok := err.(*exec.ExitError); ok {
			if waitStatus, ok := exitErr.Sys().(syscall.WaitStatus); ok {
				exitCode = waitStatus.ExitStatus()
			} else {
				exitCode = 1
			}
		} else {
			exitCode = 1
		}
	}

	payload := map[string]interface{}{
		"status":      status,
		"agent":       firstNonEmpty(spec.Role, spec.Agent),
		"role":        spec.Role,
		"model":       spec.Model,
		"cmd":         spec.Cmd,
		"args":        spec.Args,
		"exit_code":   exitCode,
		"stdout":      strings.TrimSpace(stdout.String()),
		"stderr":      strings.TrimSpace(stderr.String()),
		"duration_ms": duration,
	}
	if err != nil {
		payload["error"] = err.Error()
	}
	return payload, nil
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
	if configPath == "" {
		configPath = getenv("CONDUCTOR_CONFIG", filepath.Join(os.Getenv("HOME"), ".conductor-kit", "conductor.json"))
	}

	timeout := time.Duration(timeoutMs) * time.Millisecond
	results := []map[string]interface{}{}
	agentList := []string{}

	type specEntry struct {
		agent string
		spec  CmdSpec
	}
	entries := []specEntry{}

	if roles != "" {
		cfg, err := loadConfig(configPath)
		if err != nil {
			return map[string]interface{}{"status": "missing_config", "note": "Role-based batch requested but config is missing or invalid.", "config": configPath}, nil
		}
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
				spec, err := buildSpecFromRole(configPath, role, prompt, entry.Name, entry.ReasoningEffort)
				if err != nil {
					results = append(results, map[string]interface{}{"agent": role, "status": "error", "error": err.Error()})
					continue
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
			spec, err := buildSpecFromAgent(agent, prompt)
			if err != nil {
				results = append(results, map[string]interface{}{"agent": agent, "status": "error", "error": err.Error()})
				continue
			}
			entries = append(entries, specEntry{agent: agent, spec: spec})
		}
	}

	var wg sync.WaitGroup
	var mu sync.Mutex
	for _, entry := range entries {
		wg.Add(1)
		go func(e specEntry) {
			defer wg.Done()
			res, err := runCommand(e.spec, timeout)
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
		"status":  status,
		"agents":  agentList,
		"results": results,
		"count":   len(results),
		"warning": func() interface{} {
			if roles == "" && (modelOverride != "" || reasoningOverride != "") {
				return "Model overrides apply only to roles mode"
			}
			return nil
		}(),
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
