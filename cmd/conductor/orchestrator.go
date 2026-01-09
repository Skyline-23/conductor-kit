package main

import (
	"encoding/json"
	"regexp"
	"sort"
	"strings"
)

var (
	rolesJSONPattern = regexp.MustCompile(`(?s)\[(.*?)\]`)
)

type DelegatedTask struct {
	Role   string `json:"role"`
	Prompt string `json:"prompt"`
}

func autoSelectRoles(prompt string, cfg Config) []string {
	tasks := autoPlanTasks(prompt, cfg)
	roles := []string{}
	seen := map[string]bool{}
	for _, task := range tasks {
		if task.Role == "" || seen[task.Role] {
			continue
		}
		seen[task.Role] = true
		roles = append(roles, task.Role)
	}
	return roles
}

func autoPlanTasks(prompt string, cfg Config) []DelegatedTask {
	routing := normalizeRouting(cfg.Routing)
	available := availableRoles(cfg)

	if strings.EqualFold(routing.Strategy, "oracle") {
		if tasks, ok := planDelegatedTasks(prompt, cfg, routing, available); ok {
			return tasks
		}
	}

	tasks := tasksFromRoles(sortedRoles(cfg), prompt)
	return ensureAlwaysTasks(tasks, routing.Always, prompt, available)
}

func planDelegatedTasks(prompt string, cfg Config, routing RoutingConfig, available map[string]bool) ([]DelegatedTask, bool) {
	router := routing.RouterRole
	if router == "" {
		router = "oracle"
	}
	roleCfg, ok := cfg.Roles[router]
	if !ok || roleCfg.CLI == "" {
		return nil, false
	}
	if !isCommandAvailable(roleCfg.CLI) {
		return nil, false
	}

	roleNames := sortedRoles(cfg)
	routerPrompt := buildRouterPrompt(prompt, roleNames)

	defaults := normalizeDefaults(cfg.Defaults)
	spec, err := buildSpecFromRole(cfg, router, routerPrompt, "", "", defaults.LogPrompt)
	if err != nil {
		return nil, false
	}
	res, err := runCommand(spec)
	if err != nil {
		return nil, false
	}
	stdout, _ := res["stdout"].(string)
	tasks := parseDelegatedTasks(stdout, prompt, available)
	tasks = ensureAlwaysTasks(tasks, routing.Always, prompt, available)
	if len(tasks) == 0 {
		return nil, false
	}
	return tasks, true
}

func normalizeRouting(r RoutingConfig) RoutingConfig {
	if r.Strategy == "" {
		r.Strategy = "oracle"
	}
	if r.RouterRole == "" {
		r.RouterRole = "oracle"
	}
	if len(r.Always) == 0 {
		r.Always = []string{"oracle"}
	}
	return r
}

func availableRoles(cfg Config) map[string]bool {
	available := map[string]bool{}
	for name := range cfg.Roles {
		available[name] = true
	}
	return available
}

func sortedRoles(cfg Config) []string {
	roles := make([]string, 0, len(cfg.Roles))
	for name := range cfg.Roles {
		roles = append(roles, name)
	}
	sort.Strings(roles)
	return roles
}

func buildRouterPrompt(prompt string, roles []string) string {
	builder := strings.Builder{}
	builder.WriteString("You are routing tasks to roles.\n")
	builder.WriteString("Return ONLY a JSON array of objects: [{\"role\":\"<role>\",\"prompt\":\"<role-specific prompt>\"}].\n")
	builder.WriteString("Rules:\n")
	builder.WriteString("- Use only the provided roles.\n")
	builder.WriteString("- Include multiple roles when useful.\n")
	builder.WriteString("- Keep prompts short and role-specific.\n")
	builder.WriteString("- If none fit, return an empty array.\n\n")
	builder.WriteString("Available roles:\n")
	builder.WriteString(strings.Join(roles, ", "))
	builder.WriteString("\n\n")
	builder.WriteString("Task:\n")
	builder.WriteString(prompt)
	return builder.String()
}

func parseDelegatedTasks(output, fallbackPrompt string, available map[string]bool) []DelegatedTask {
	trimmed := strings.TrimSpace(output)
	if trimmed == "" {
		return nil
	}
	if tasks := parseTaskJSONArray(trimmed); len(tasks) > 0 {
		return filterTasks(tasks, fallbackPrompt, available)
	}
	if match := rolesJSONPattern.FindString(trimmed); match != "" {
		if tasks := parseTaskJSONArray(match); len(tasks) > 0 {
			return filterTasks(tasks, fallbackPrompt, available)
		}
	}
	if roles := parseRoleJSONArray(trimmed); len(roles) > 0 {
		return filterTasks(tasksFromRoles(roles, fallbackPrompt), fallbackPrompt, available)
	}
	if match := rolesJSONPattern.FindString(trimmed); match != "" {
		if roles := parseRoleJSONArray(match); len(roles) > 0 {
			return filterTasks(tasksFromRoles(roles, fallbackPrompt), fallbackPrompt, available)
		}
	}
	lines := strings.Split(trimmed, "\n")
	parsed := []DelegatedTask{}
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		role, prompt, ok := parseRolePromptLine(line)
		if !ok {
			continue
		}
		parsed = append(parsed, DelegatedTask{Role: role, Prompt: prompt})
	}
	return filterTasks(parsed, fallbackPrompt, available)
}

func parseRoleJSONArray(raw string) []string {
	var roles []string
	if err := json.Unmarshal([]byte(raw), &roles); err == nil {
		return roles
	}
	return nil
}

func parseTaskJSONArray(raw string) []DelegatedTask {
	var tasks []DelegatedTask
	if err := json.Unmarshal([]byte(raw), &tasks); err == nil {
		return tasks
	}
	return nil
}

func parseRolePromptLine(line string) (string, string, bool) {
	seps := []string{":", " - ", " — "}
	for _, sep := range seps {
		if idx := strings.Index(line, sep); idx > 0 {
			role := strings.TrimSpace(strings.Trim(line[:idx], "\"'"))
			prompt := strings.TrimSpace(strings.Trim(line[idx+len(sep):], "\"'"))
			if role == "" {
				return "", "", false
			}
			return role, prompt, true
		}
	}
	return "", "", false
}

func tasksFromRoles(roles []string, prompt string) []DelegatedTask {
	tasks := make([]DelegatedTask, 0, len(roles))
	for _, role := range roles {
		if role == "" {
			continue
		}
		tasks = append(tasks, DelegatedTask{Role: role, Prompt: prompt})
	}
	return tasks
}

func filterTasks(tasks []DelegatedTask, fallbackPrompt string, available map[string]bool) []DelegatedTask {
	out := []DelegatedTask{}
	seen := map[string]bool{}
	for _, task := range tasks {
		role := strings.TrimSpace(task.Role)
		if role == "" || !available[role] || seen[role] {
			continue
		}
		seen[role] = true
		prompt := strings.TrimSpace(task.Prompt)
		if prompt == "" {
			prompt = fallbackPrompt
		}
		out = append(out, DelegatedTask{Role: role, Prompt: prompt})
	}
	return out
}

func ensureAlwaysTasks(tasks []DelegatedTask, always []string, fallbackPrompt string, available map[string]bool) []DelegatedTask {
	if len(always) == 0 {
		return tasks
	}
	seen := map[string]bool{}
	for _, task := range tasks {
		seen[task.Role] = true
	}
	for _, role := range always {
		if role == "" || seen[role] || !available[role] {
			continue
		}
		tasks = append(tasks, DelegatedTask{Role: role, Prompt: fallbackPrompt})
		seen[role] = true
	}
	return tasks
}
