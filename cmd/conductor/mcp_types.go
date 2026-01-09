package main

type BatchInput struct {
	Prompt          string `json:"prompt"`
	Roles           string `json:"roles,omitempty"`
	Agents          string `json:"agents,omitempty"`
	Model           string `json:"model,omitempty"`
	Reasoning       string `json:"reasoning,omitempty"`
	Config          string `json:"config,omitempty"`
	TimeoutMs       int    `json:"timeout_ms,omitempty"`
	RequireApproval bool   `json:"require_approval,omitempty"`
	Mode            string `json:"mode,omitempty"`
	NoDaemon        bool   `json:"no_daemon,omitempty"`
}

type RunInput struct {
	Prompt          string `json:"prompt"`
	Role            string `json:"role,omitempty"`
	Agent           string `json:"agent,omitempty"`
	Model           string `json:"model,omitempty"`
	Reasoning       string `json:"reasoning,omitempty"`
	Config          string `json:"config,omitempty"`
	TimeoutMs       int    `json:"timeout_ms,omitempty"`
	RequireApproval bool   `json:"require_approval,omitempty"`
	Mode            string `json:"mode,omitempty"`
	NoDaemon        bool   `json:"no_daemon,omitempty"`
}

type StatusInput struct {
	RunID string `json:"run_id"`
	Tail  int    `json:"tail,omitempty"`
}

type WaitInput struct {
	RunID     string `json:"run_id"`
	TimeoutMs int    `json:"timeout_ms,omitempty"`
	Tail      int    `json:"tail,omitempty"`
}

type CancelInput struct {
	RunID string `json:"run_id"`
	Force bool   `json:"force,omitempty"`
}

type HistoryInput struct {
	Limit  int    `json:"limit,omitempty"`
	Status string `json:"status,omitempty"`
	Role   string `json:"role,omitempty"`
	Agent  string `json:"agent,omitempty"`
}

type InfoInput struct {
	RunID string `json:"run_id"`
}

type QueueListInput struct {
	Status string `json:"status,omitempty"`
	Limit  int    `json:"limit,omitempty"`
}

type ApprovalInput struct {
	RunID string `json:"run_id"`
}
