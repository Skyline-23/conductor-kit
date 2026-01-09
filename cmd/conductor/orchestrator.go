package main

import (
	"encoding/json"
	"regexp"
	"sort"
	"strings"
)

var (
	rolesJSONPattern = regexp.MustCompile(`(?s)\\[(.*?)\\]`)
)

func autoSelectRoles(prompt string, cfg Config) []string {
	routing := normalizeRouting(cfg.Routing)
	available := availableRoles(cfg)
	add := func(out []string, role string) []string {
		if !available[role] {
			return out
		}
		for _, existing := range out {
			if existing == role {
				return out
			}
		}
		return append(out, role)
	}

	out := []string{}
	text := strings.ToLower(prompt)

	for _, role := range routing.Always {
		out = add(out, role)
	}

	if strings.EqualFold(routing.Strategy, "oracle") {
		if roles, ok := selectRolesWithOracle(prompt, cfg, routing, available); ok {
			for _, role := range roles {
				out = add(out, role)
			}
			if len(out) > 0 {
				return out
			}
		}
	}

	for _, role := range selectRolesByHints(prompt, cfg, routing, available) {
		out = add(out, role)
	}

	for name := range cfg.Roles {
		if name != "" && strings.Contains(text, strings.ToLower(name)) {
			out = add(out, name)
		}
	}

	if len(out) == 0 {
		out = append(out, sortedRoles(cfg)...)
	}
	return out
}

func selectRolesWithOracle(prompt string, cfg Config, routing RoutingConfig, available map[string]bool) ([]string, bool) {
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
	roleHints := buildRoleHints(roleNames, routing.Hints)
	routerPrompt := buildRouterPrompt(prompt, roleNames, roleHints)

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
	roles := parseRoleList(stdout, available)
	if len(roles) == 0 {
		return nil, false
	}
	return roles, true
}

func selectRolesByHints(prompt string, cfg Config, routing RoutingConfig, available map[string]bool) []string {
	lower := strings.ToLower(prompt)
	out := []string{}
	add := func(role string) {
		if !available[role] {
			return
		}
		for _, existing := range out {
			if existing == role {
				return
			}
		}
		out = append(out, role)
	}
	for role, hints := range routing.Hints {
		for _, hint := range hints {
			h := strings.TrimSpace(strings.ToLower(hint))
			if h == "" {
				continue
			}
			if strings.Contains(lower, h) {
				add(role)
				break
			}
		}
	}
	return out
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
	if r.Hints == nil {
		r.Hints = map[string][]string{}
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

func buildRoleHints(roles []string, hints map[string][]string) []string {
	lines := make([]string, 0, len(roles))
	for _, role := range roles {
		if len(hints[role]) == 0 {
			continue
		}
		lines = append(lines, role+": "+strings.Join(hints[role], ", "))
	}
	return lines
}

func buildRouterPrompt(prompt string, roles, hints []string) string {
	builder := strings.Builder{}
	builder.WriteString("You are routing tasks to roles.\n")
	builder.WriteString("Return ONLY a JSON array of role names.\n")
	builder.WriteString("Rules:\n")
	builder.WriteString("- Use only the provided roles.\n")
	builder.WriteString("- Include multiple roles when useful.\n")
	builder.WriteString("- If none fit, return an empty array.\n\n")
	builder.WriteString("Available roles:\n")
	builder.WriteString(strings.Join(roles, ", "))
	builder.WriteString("\n\n")
	if len(hints) > 0 {
		builder.WriteString("Role hints:\n")
		builder.WriteString(strings.Join(hints, "\n"))
		builder.WriteString("\n\n")
	}
	builder.WriteString("Task:\n")
	builder.WriteString(prompt)
	return builder.String()
}

func parseRoleList(output string, available map[string]bool) []string {
	trimmed := strings.TrimSpace(output)
	if trimmed == "" {
		return nil
	}
	candidates := []string{}
	if roles := parseRoleJSONArray(trimmed); len(roles) > 0 {
		candidates = roles
	} else if match := rolesJSONPattern.FindString(trimmed); match != "" {
		if roles := parseRoleJSONArray(match); len(roles) > 0 {
			candidates = roles
		}
	}
	if len(candidates) == 0 {
		for _, part := range strings.FieldsFunc(trimmed, func(r rune) bool {
			return r == ',' || r == '\n' || r == '\r' || r == '\t'
		}) {
			val := strings.TrimSpace(strings.Trim(part, "\"'"))
			if val != "" {
				candidates = append(candidates, val)
			}
		}
	}
	out := []string{}
	seen := map[string]bool{}
	for _, role := range candidates {
		if !available[role] || seen[role] {
			continue
		}
		seen[role] = true
		out = append(out, role)
	}
	return out
}

func parseRoleJSONArray(raw string) []string {
	var roles []string
	if err := json.Unmarshal([]byte(raw), &roles); err == nil {
		return roles
	}
	return nil
}
