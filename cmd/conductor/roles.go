package main

import "sort"

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

func unknownRolePayload(cfg Config, role, configPath string) map[string]interface{} {
	return map[string]interface{}{
		"status": "unknown_role",
		"role":   role,
		"roles":  roleNames(cfg),
		"config": configPath,
	}
}
