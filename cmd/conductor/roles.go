package main

import (
	"os"
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
		if len(roleCfg.AuthEnv) > 0 {
			entry["auth_env"] = roleCfg.AuthEnv
		}
		if len(roleCfg.AuthFiles) > 0 {
			entry["auth_files"] = roleCfg.AuthFiles
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
			authStatus, authErr := checkAuthHints(roleCfg)
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

func checkAuthHints(roleCfg RoleConfig) (string, string) {
	env := roleCfg.AuthEnv
	files := roleCfg.AuthFiles
	missingEnv := []string{}
	missingFiles := []string{}
	for _, key := range env {
		key = strings.TrimSpace(key)
		if key == "" {
			continue
		}
		if os.Getenv(key) == "" {
			missingEnv = append(missingEnv, key)
		}
	}
	for _, path := range files {
		path = strings.TrimSpace(path)
		if path == "" {
			continue
		}
		if !pathExists(expandPath(path)) {
			missingFiles = append(missingFiles, path)
		}
	}
	if len(env) == 0 && len(files) == 0 {
		return "unknown", "no auth hints configured"
	}
	if len(missingEnv) == 0 && len(missingFiles) == 0 {
		return "ready", ""
	}
	errMsg := ""
	if len(missingEnv) > 0 {
		errMsg = "missing env: " + strings.Join(missingEnv, ", ")
	}
	if len(missingFiles) > 0 {
		if errMsg != "" {
			errMsg += "; "
		}
		errMsg += "missing files: " + strings.Join(missingFiles, ", ")
	}
	return "not_ready", errMsg
}

func unknownRolePayload(cfg Config, role, configPath string) map[string]interface{} {
	return map[string]interface{}{
		"status": "unknown_role",
		"role":   role,
		"roles":  roleNames(cfg),
		"config": configPath,
	}
}
