package main

import (
	"testing"
)

func TestTasksFromRoles(t *testing.T) {
	tests := []struct {
		name   string
		roles  []string
		prompt string
		want   int
	}{
		{
			name:   "single role",
			roles:  []string{"oracle"},
			prompt: "test prompt",
			want:   1,
		},
		{
			name:   "multiple roles",
			roles:  []string{"oracle", "explorer", "librarian"},
			prompt: "test prompt",
			want:   3,
		},
		{
			name:   "empty roles",
			roles:  []string{},
			prompt: "test prompt",
			want:   0,
		},
		{
			name:   "roles with whitespace",
			roles:  []string{"  oracle  ", "explorer", "  "},
			prompt: "test prompt",
			want:   2,
		},
		{
			name:   "all empty strings",
			roles:  []string{"", "  ", "\t"},
			prompt: "test prompt",
			want:   0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tasks := tasksFromRoles(tt.roles, tt.prompt)
			if len(tasks) != tt.want {
				t.Errorf("tasksFromRoles: expected %d tasks, got %d", tt.want, len(tasks))
			}
			for _, task := range tasks {
				if task.Prompt != tt.prompt {
					t.Errorf("tasksFromRoles: expected prompt %q, got %q", tt.prompt, task.Prompt)
				}
				if task.Role == "" {
					t.Error("tasksFromRoles: role should not be empty")
				}
			}
		})
	}
}
