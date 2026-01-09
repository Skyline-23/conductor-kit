package main

import "strings"

type DelegatedTask struct {
	Role   string `json:"role"`
	Prompt string `json:"prompt"`
}

func tasksFromRoles(roles []string, prompt string) []DelegatedTask {
	tasks := make([]DelegatedTask, 0, len(roles))
	for _, role := range roles {
		role = strings.TrimSpace(role)
		if role == "" {
			continue
		}
		tasks = append(tasks, DelegatedTask{Role: role, Prompt: prompt})
	}
	return tasks
}
