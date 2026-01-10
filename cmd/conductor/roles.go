package main

import (
	"fmt"
	"sort"
	"strings"
)

func roleNames(cfg Config) []string {
	names := make([]string, 0, len(cfg.Roles))
	for name := range cfg.Roles {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func listRolesPayload(cfg Config, configPath string) map[string]interface{} {
	names := roleNames(cfg)
	roles := make([]map[string]interface{}, 0, len(names))
	for _, name := range names {
		roleCfg := cfg.Roles[name]
		entry := map[string]interface{}{"role": name}
		if roleCfg.CLI != "" {
			entry["cli"] = roleCfg.CLI
		}
		if roleCfg.Model != "" {
			entry["model"] = roleCfg.Model
		}
		if roleCfg.Reasoning != "" {
			entry["reasoning"] = roleCfg.Reasoning
		}
		roles = append(roles, entry)
	}
	return map[string]interface{}{
		"count":  len(roles),
		"roles":  roles,
		"config": configPath,
	}
}

func statusPayload(cfg Config, configPath string) (map[string]interface{}, bool) {
	names := roleNames(cfg)
	roles := make([]map[string]interface{}, 0, len(names))
	ok := true
	for _, name := range names {
		roleCfg := cfg.Roles[name]
		entry := map[string]interface{}{
			"role": name,
		}
		if roleCfg.CLI != "" {
			entry["cli"] = roleCfg.CLI
		}
		if roleCfg.Model != "" {
			entry["model"] = roleCfg.Model
		}
		if roleCfg.Reasoning != "" {
			entry["reasoning"] = roleCfg.Reasoning
		}
		status := "unknown"
		if roleCfg.CLI == "" {
			status = "invalid"
			entry["error"] = "missing cli"
			ok = false
		} else if !isCommandAvailable(roleCfg.CLI) {
			status = "missing_cli"
			entry["error"] = "missing CLI on PATH: " + roleCfg.CLI
			ok = false
		} else {
			authStatus, authErr := checkAuthForCLI(roleCfg.CLI)
			status = authStatus
			if authErr != "" {
				entry["error"] = authErr
				ok = false
			}
		}
		entry["status"] = status
		roles = append(roles, entry)
	}
	return map[string]interface{}{
		"count":  len(roles),
		"roles":  roles,
		"config": configPath,
	}, ok
}

func unknownRolePayload(cfg Config, role, configPath string) map[string]interface{} {
	return map[string]interface{}{
		"status":  "unknown_role",
		"role":    role,
		"roles":   roleNames(cfg),
		"config":  configPath,
		"message": unknownRoleMessage(cfg, role),
	}
}

func unknownRoleMessage(cfg Config, role string) string {
	available := roleNames(cfg)
	if len(available) == 0 {
		return fmt.Sprintf("Unknown role: %s (no roles configured)", role)
	}
	return fmt.Sprintf("Unknown role: %s (available: %s)", role, strings.Join(available, ", "))
}

func unknownRoleResult(cfg Config, role string) map[string]interface{} {
	return map[string]interface{}{
		"agent":  role,
		"status": "error",
		"error":  unknownRoleMessage(cfg, role),
		"roles":  roleNames(cfg),
	}
}
