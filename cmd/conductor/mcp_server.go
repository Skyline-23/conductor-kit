package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

const (
	// Session TTL - sessions expire after 1 hour of inactivity
	mcpSessionTTL = 1 * time.Hour
	// Cleanup interval - check for expired sessions every 10 minutes
	mcpSessionCleanupInterval = 10 * time.Minute
	// Max sessions to prevent memory exhaustion
	mcpMaxSessions = 100
)

// Session management for multi-turn conversations (matches OpenAI Codex MCP pattern)
var (
	mcpSessionStore   = make(map[string]*MCPSession)
	mcpSessionStoreMu sync.RWMutex
)

// MCPSession represents a conversation session
// For Codex/Claude/Gemini: uses native session resume (no history re-transmission)
type MCPSession struct {
	ID             string           // Our session ID (maps to native thread/session ID)
	NativeThreadID string           // Native CLI thread ID (for Codex: from structuredContent.threadId)
	CLI            string           // codex, claude, gemini
	Role           string           // role name if created via conductor tool
	Model          string           // model used
	Config         MCPSessionConfig // original session configuration
	CreatedAt      time.Time
	UpdatedAt      time.Time
}

// MCPSessionConfig stores original session settings for reply
type MCPSessionConfig struct {
	// Codex settings
	ApprovalPolicy string
	Sandbox        string
	Cwd            string
	Profile        string
	// Claude settings
	PermissionMode     string
	AllowedTools       string
	DisallowedTools    string
	SystemPrompt       string
	AppendSystemPrompt string
	// Gemini settings
	Yolo               bool
	ApprovalMode       string
	IncludeDirectories string
}

// MCPMessage represents a message in a session
type MCPMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// CLI Adapters
var (
	mcpCodexAdapter  = &CLIAdapter{Name: "Codex", Cmd: "codex"}
	mcpClaudeAdapter = &CLIAdapter{Name: "Claude", Cmd: "claude"}
	mcpGeminiAdapter = &CLIAdapter{Name: "Gemini", Cmd: "gemini"}
)

// Input types matching OpenAI Codex MCP server pattern

// MCPCodexInput for codex tool
type MCPCodexInput struct {
	Prompt           string                 `json:"prompt"`
	ApprovalPolicy   string                 `json:"approval-policy,omitempty"`
	BaseInstructions string                 `json:"base-instructions,omitempty"`
	Config           map[string]interface{} `json:"config,omitempty"`
	Cwd              string                 `json:"cwd,omitempty"`
	IncludePlanTool  *bool                  `json:"include-plan-tool,omitempty"`
	Model            string                 `json:"model,omitempty"`
	Profile          string                 `json:"profile,omitempty"`
	Sandbox          string                 `json:"sandbox,omitempty"`
	IdleTimeoutMs    int                    `json:"idle_timeout_ms,omitempty"`
	MemoryKey        string                 `json:"memory_key,omitempty"`
	MemoryMode       string                 `json:"memory_mode,omitempty"`
	// Reasoning effort for o-series models (low, medium, high)
	ReasoningEffort string `json:"reasoning-effort,omitempty"`
}

// MCPClaudeInput for claude tool
type MCPClaudeInput struct {
	Prompt             string `json:"prompt"`
	Model              string `json:"model,omitempty"`
	PermissionMode     string `json:"permission-mode,omitempty"`
	AllowedTools       string `json:"allowed-tools,omitempty"`
	DisallowedTools    string `json:"disallowed-tools,omitempty"`
	SystemPrompt       string `json:"system-prompt,omitempty"`
	AppendSystemPrompt string `json:"append-system-prompt,omitempty"`
	MaxTurns           int    `json:"max-turns,omitempty"`
	Cwd                string `json:"cwd,omitempty"`
	AddDir             string `json:"add-dir,omitempty"`
	McpConfig          string `json:"mcp-config,omitempty"`
	Agents             string `json:"agents,omitempty"`
	Debug              bool   `json:"debug,omitempty"`
	IdleTimeoutMs      int    `json:"idle_timeout_ms,omitempty"`
	MemoryKey          string `json:"memory_key,omitempty"`
	MemoryMode         string `json:"memory_mode,omitempty"`
}

// MCPGeminiInput for gemini tool
type MCPGeminiInput struct {
	Prompt             string `json:"prompt"`
	Model              string `json:"model,omitempty"`
	Sandbox            string `json:"sandbox,omitempty"`
	Yolo               bool   `json:"yolo,omitempty"`
	ApprovalMode       string `json:"approval-mode,omitempty"`
	IncludeDirectories string `json:"include-directories,omitempty"`
	Cwd                string `json:"cwd,omitempty"`
	Debug              bool   `json:"debug,omitempty"`
	IdleTimeoutMs      int    `json:"idle_timeout_ms,omitempty"`
	MemoryKey          string `json:"memory_key,omitempty"`
	MemoryMode         string `json:"memory_mode,omitempty"`
}

// MCPReplyInput for *-reply tools
type MCPReplyInput struct {
	Prompt         string `json:"prompt"`
	ThreadID       string `json:"threadId"`
	ConversationID string `json:"conversationId,omitempty"` // deprecated alias
	MemoryKey      string `json:"memory_key,omitempty"`
	MemoryMode     string `json:"memory_mode,omitempty"`
}

// MCPConductorInput for role-based routing
type MCPConductorInput struct {
	Prompt        string `json:"prompt"`
	Role          string `json:"role"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
	MemoryKey     string `json:"memory_key,omitempty"`
	MemoryMode    string `json:"memory_mode,omitempty"`
}

type MCPMemoryInput struct {
	Action    string `json:"action"`
	Key       string `json:"key,omitempty"`
	Value     string `json:"value,omitempty"`
	Separator string `json:"separator,omitempty"`
}

func runMCPServer(args []string) int {
	_ = args

	// Start session cleanup goroutine
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go mcpSessionCleanupLoop(ctx)

	server := mcp.NewServer(&mcp.Implementation{
		Name:    "conductor-mcp-server",
		Version: "1.0.0",
	}, nil)

	// ===== Codex Tools =====
	mcp.AddTool(server, &mcp.Tool{
		Name: "codex",
		Description: `Run a Codex session. Returns structuredContent.threadId for continuation.

Parameters:
- prompt (required): The user prompt
- approval-policy: "untrusted", "on-request", "on-failure", "never"
- sandbox: "read-only", "workspace-write", "danger-full-access"
- model: Model override (e.g., "o3", "o4-mini")
- profile: Configuration profile from config.toml
- cwd: Working directory
- config: Individual config overrides
- base-instructions: Custom base instructions
- include-plan-tool: Include plan tool in conversation
- reasoning-effort: Reasoning effort for o-series models ("low", "medium", "high")
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPCodexInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		prompt := input.Prompt
		if input.MemoryKey != "" {
			var err error
			prompt, err = applyMemoryToPrompt(prompt, input.MemoryKey, input.MemoryMode)
			if err != nil {
				return nil, nil, err
			}
		}
		prompt = applySharedMemory(prompt)
		config := MCPSessionConfig{
			ApprovalPolicy: input.ApprovalPolicy,
			Sandbox:        input.Sandbox,
			Cwd:            input.Cwd,
			Profile:        input.Profile,
		}
		result, err := mcpRunSessionWithConfig(ctx, "codex", "", input.Model, prompt, mcpBuildCodexArgs(input), input.IdleTimeoutMs, config)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name: "codex-reply",
		Description: `Continue a Codex session.

Parameters:
- prompt (required): The next user prompt
- threadId (required): Thread ID from previous response
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPReplyInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		result, err := mcpRunReply(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	// ===== Claude Tools =====
	mcp.AddTool(server, &mcp.Tool{
		Name: "claude",
		Description: `Run a Claude Code session. Returns structuredContent.threadId for continuation.

Parameters:
- prompt (required): The user prompt
- model: Model alias (sonnet, opus) or full name
- permission-mode: "default", "acceptEdits", "bypassPermissions", "plan", "dontAsk"
- allowed-tools: Comma-separated tool names (e.g., "Bash,Edit,Read")
- disallowed-tools: Comma-separated tool names to disable
- system-prompt: Replace entire system prompt
- append-system-prompt: Append to default system prompt
- max-turns: Limit number of agentic turns
- cwd: Working directory for the session
- add-dir: Additional directories to include (space-separated)
- mcp-config: Path to MCP server config JSON
- agents: JSON object defining custom subagents
- debug: Enable debug mode
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPClaudeInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		prompt := input.Prompt
		if input.MemoryKey != "" {
			var err error
			prompt, err = applyMemoryToPrompt(prompt, input.MemoryKey, input.MemoryMode)
			if err != nil {
				return nil, nil, err
			}
		}
		prompt = applySharedMemory(prompt)
		config := MCPSessionConfig{
			PermissionMode:     input.PermissionMode,
			AllowedTools:       input.AllowedTools,
			DisallowedTools:    input.DisallowedTools,
			SystemPrompt:       input.SystemPrompt,
			AppendSystemPrompt: input.AppendSystemPrompt,
		}
		result, err := mcpRunSessionWithConfig(ctx, "claude", "", input.Model, prompt, mcpBuildClaudeArgs(input), input.IdleTimeoutMs, config)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name: "claude-reply",
		Description: `Continue a Claude session.

Parameters:
- prompt (required): The next user prompt
- threadId (required): Thread ID from previous response
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPReplyInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		result, err := mcpRunReply(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	// ===== Gemini Tools =====
	mcp.AddTool(server, &mcp.Tool{
		Name: "gemini",
		Description: `Run a Gemini session. Returns structuredContent.threadId for continuation.

Parameters:
- prompt (required): The user prompt
- model: Model override (e.g., "gemini-2.5-pro", "gemini-2.5-flash")
- sandbox: Sandbox mode
- yolo: Auto-approve all actions (equivalent to -y flag)
- approval-mode: Approval policy ("auto_edit", etc.)
- include-directories: Comma-separated additional directories to include
- cwd: Working directory for the session
- debug: Enable debug mode
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPGeminiInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		prompt := input.Prompt
		if input.MemoryKey != "" {
			var err error
			prompt, err = applyMemoryToPrompt(prompt, input.MemoryKey, input.MemoryMode)
			if err != nil {
				return nil, nil, err
			}
		}
		prompt = applySharedMemory(prompt)
		config := MCPSessionConfig{
			Sandbox:            input.Sandbox,
			Yolo:               input.Yolo,
			ApprovalMode:       input.ApprovalMode,
			IncludeDirectories: input.IncludeDirectories,
			Cwd:                input.Cwd,
		}
		result, err := mcpRunSessionWithConfig(ctx, "gemini", "", input.Model, prompt, mcpBuildGeminiArgs(input), input.IdleTimeoutMs, config)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name: "gemini-reply",
		Description: `Continue a Gemini session.

Parameters:
- prompt (required): The next user prompt
- threadId (required): Thread ID from previous response
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPReplyInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		result, err := mcpRunReply(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	// ===== Conductor Role-based Routing =====
	mcp.AddTool(server, &mcp.Tool{
		Name: "conductor",
		Description: `Run a session with role-based CLI routing. Uses conductor.json to map roles to CLIs.

Parameters:
- prompt (required): The user prompt
- role (required): Role name (e.g., "oracle", "explore", "librarian")
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"

Available roles are defined in ~/.conductor-kit/conductor.json`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPConductorInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		if input.Role == "" {
			return nil, nil, fmt.Errorf("role is required")
		}
		if input.MemoryKey != "" {
			prompt, err := applyMemoryToPrompt(input.Prompt, input.MemoryKey, input.MemoryMode)
			if err != nil {
				return nil, nil, err
			}
			input.Prompt = prompt
		}
		result, err := mcpRunRoleSession(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	mcp.AddTool(server, &mcp.Tool{
		Name: "conductor-reply",
		Description: `Continue a conductor session.

Parameters:
- prompt (required): The next user prompt
- threadId (required): Thread ID from previous response
- memory_key: Shared memory key to inject
- memory_mode: "prepend" (default) or "append"`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPReplyInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		result, err := mcpRunReply(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
	})

	// ===== Shared Memory Tool =====
	mcp.AddTool(server, &mcp.Tool{
		Name: "memory",
		Description: `Manage shared memory for this MCP server.

Cached per project (TTL + git HEAD invalidation).

Actions:
- set: store value at key
- append: append value with separator
- get: fetch value by key
- list: list keys
- clear: delete a key
- clear_all: clear everything`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPMemoryInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		payload, err := mcpHandleMemory(input)
		if err != nil {
			return nil, nil, err
		}
		return nil, payload, nil
	})

	// ===== Status Tool =====
	mcp.AddTool(server, &mcp.Tool{
		Name: "status",
		Description: `Check CLI availability and session status.

Returns:
- cli: availability status for codex, claude, gemini
- sessions: active session count and info`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input struct{}) (*mcp.CallToolResult, map[string]interface{}, error) {
		return nil, mcpGetStatus(), nil
	})

	transport := mcp.NewStdioTransport()
	session, err := server.Connect(context.Background(), transport, nil)
	if err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		return 1
	}
	if err := session.Wait(); err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		return 1
	}
	return 0
}

// mcpSessionCleanupLoop periodically removes expired sessions
func mcpSessionCleanupLoop(ctx context.Context) {
	ticker := time.NewTicker(mcpSessionCleanupInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			mcpCleanupExpiredSessions()
		}
	}
}

// mcpCleanupExpiredSessions removes sessions that have exceeded TTL
func mcpCleanupExpiredSessions() {
	now := time.Now()
	mcpSessionStoreMu.Lock()
	defer mcpSessionStoreMu.Unlock()

	for id, sess := range mcpSessionStore {
		if now.Sub(sess.UpdatedAt) > mcpSessionTTL {
			delete(mcpSessionStore, id)
		}
	}
}

// mcpEvictOldestSession removes the oldest session if at capacity
func mcpEvictOldestSession() {
	if len(mcpSessionStore) < mcpMaxSessions {
		return
	}

	var oldestID string
	var oldestTime time.Time

	for id, sess := range mcpSessionStore {
		if oldestID == "" || sess.UpdatedAt.Before(oldestTime) {
			oldestID = id
			oldestTime = sess.UpdatedAt
		}
	}

	if oldestID != "" {
		delete(mcpSessionStore, oldestID)
	}
}

// mcpRunQuickCommand runs a simple command with timeout and returns output
func mcpRunQuickCommand(cmd string, args []string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	c := exec.CommandContext(ctx, cmd, args...)
	out, err := c.CombinedOutput()
	return string(out), err
}

// mcpCheckCodexAuth checks Codex CLI authentication status
func mcpCheckCodexAuth() (bool, string) {
	if !isCommandAvailable("codex") {
		return false, "not installed"
	}

	// Get version first
	version := ""
	if out, err := mcpRunQuickCommand("codex", []string{"--version"}); err == nil {
		version = strings.TrimSpace(out)
	}

	// Check login status
	output, err := mcpRunQuickCommand("codex", []string{"login", "status"})
	if err != nil {
		if version != "" {
			return false, fmt.Sprintf("%s, no auth", version)
		}
		return false, "no auth"
	}
	output = strings.TrimSpace(output)
	if strings.Contains(strings.ToLower(output), "logged in") {
		// Simplify auth message
		authMsg := "OAuth"
		if strings.Contains(output, "API") {
			authMsg = "API key"
		}
		if version != "" {
			return true, fmt.Sprintf("%s, %s", version, authMsg)
		}
		return true, authMsg
	}
	if version != "" {
		return false, fmt.Sprintf("%s, no auth", version)
	}
	return false, "no auth"
}

// mcpCheckClaudeAuth checks Claude CLI authentication status
func mcpCheckClaudeAuth() (bool, string) {
	if !isCommandAvailable("claude") {
		return false, "not installed"
	}

	// Get version first
	version := ""
	if out, err := mcpRunQuickCommand("claude", []string{"--version"}); err == nil {
		version = strings.TrimSpace(out)
	}

	// Check authentication methods:
	// 1. ANTHROPIC_API_KEY environment variable
	if apiKey := os.Getenv("ANTHROPIC_API_KEY"); apiKey != "" {
		return true, fmt.Sprintf("%s, API key", version)
	}

	// 2. Check for Claude CLI session data (~/.claude/)
	homeDir, _ := os.UserHomeDir()
	claudeDir := filepath.Join(homeDir, ".claude")
	if pathExists(claudeDir) {
		// Check for session indicators (statsig = logged in user)
		statsigDir := filepath.Join(claudeDir, "statsig")
		if pathExists(statsigDir) {
			return true, fmt.Sprintf("%s, OAuth", version)
		}
		// Check for settings.json (indicates CLI has been configured)
		settingsFile := filepath.Join(claudeDir, "settings.json")
		if pathExists(settingsFile) {
			return true, fmt.Sprintf("%s, configured", version)
		}
	}

	// No authentication found
	if version != "" {
		return false, fmt.Sprintf("%s, no auth", version)
	}
	return false, "no auth"
}

// mcpCheckGeminiAuth checks Gemini CLI authentication status
func mcpCheckGeminiAuth() (bool, string) {
	if !isCommandAvailable("gemini") {
		return false, "not installed"
	}

	// Get version first
	version := ""
	if out, err := mcpRunQuickCommand("gemini", []string{"-v"}); err == nil {
		version = strings.TrimSpace(out)
	}

	// Check authentication methods in order of priority:
	// 1. GEMINI_API_KEY environment variable
	if apiKey := os.Getenv("GEMINI_API_KEY"); apiKey != "" {
		return true, fmt.Sprintf("%s, API key", version)
	}

	// 2. GOOGLE_API_KEY environment variable (Vertex AI)
	if apiKey := os.Getenv("GOOGLE_API_KEY"); apiKey != "" {
		return true, fmt.Sprintf("%s, Vertex AI", version)
	}

	// 3. Check for cached OAuth credentials (~/.gemini/)
	homeDir, _ := os.UserHomeDir()
	geminiDir := filepath.Join(homeDir, ".gemini")
	if pathExists(geminiDir) {
		// Check for .env file with credentials
		envFile := filepath.Join(geminiDir, ".env")
		if pathExists(envFile) {
			return true, fmt.Sprintf("%s, .env", version)
		}
		// Check for cached OAuth tokens (credentials directory or files)
		credFiles := []string{"credentials", "oauth_credentials.json", "oauth_creds.json", "auth.json"}
		for _, f := range credFiles {
			if pathExists(filepath.Join(geminiDir, f)) {
				return true, fmt.Sprintf("%s, OAuth", version)
			}
		}
	}

	// 4. Check for Google Cloud ADC (gcloud auth application-default)
	if os.Getenv("GOOGLE_APPLICATION_CREDENTIALS") != "" {
		return true, fmt.Sprintf("%s, ADC", version)
	}

	// 5. Check default ADC path
	adcPath := filepath.Join(homeDir, ".config", "gcloud", "application_default_credentials.json")
	if pathExists(adcPath) {
		return true, fmt.Sprintf("%s, gcloud ADC", version)
	}

	// No authentication found
	if version != "" {
		return false, fmt.Sprintf("%s, no auth", version)
	}
	return false, "no auth"
}

// mcpGetStatus returns CLI availability and session status
func mcpGetStatus() map[string]interface{} {
	// Check Codex auth
	codexAuth, codexMsg := mcpCheckCodexAuth()
	codexStatus := map[string]interface{}{
		"available":     isCommandAvailable("codex"),
		"authenticated": codexAuth,
		"status":        codexMsg,
	}

	// Check Claude auth
	claudeAuth, claudeMsg := mcpCheckClaudeAuth()
	claudeStatus := map[string]interface{}{
		"available":     isCommandAvailable("claude"),
		"authenticated": claudeAuth,
		"status":        claudeMsg,
	}

	// Check Gemini auth
	geminiAuth, geminiMsg := mcpCheckGeminiAuth()
	geminiStatus := map[string]interface{}{
		"available":     isCommandAvailable("gemini"),
		"authenticated": geminiAuth,
		"status":        geminiMsg,
	}

	clis := map[string]interface{}{
		"codex":  codexStatus,
		"claude": claudeStatus,
		"gemini": geminiStatus,
	}

	mcpSessionStoreMu.RLock()
	sessionCount := len(mcpSessionStore)
	sessions := make([]map[string]interface{}, 0, sessionCount)
	for _, sess := range mcpSessionStore {
		sessions = append(sessions, map[string]interface{}{
			"threadId":       sess.ID,
			"nativeThreadId": sess.NativeThreadID,
			"cli":            sess.CLI,
			"role":           sess.Role,
			"model":          sess.Model,
			"createdAt":      sess.CreatedAt.Format(time.RFC3339),
			"updatedAt":      sess.UpdatedAt.Format(time.RFC3339),
		})
	}
	mcpSessionStoreMu.RUnlock()

	return map[string]interface{}{
		"cli": clis,
		"sessions": map[string]interface{}{
			"count":  sessionCount,
			"max":    mcpMaxSessions,
			"ttl":    mcpSessionTTL.String(),
			"active": sessions,
		},
	}
}

// mcpRunSession runs a new CLI session and creates a thread
func mcpRunSession(ctx context.Context, cli, prompt string, args []string, idleTimeoutMs int) (map[string]interface{}, error) {
	return mcpRunSessionWithConfig(ctx, cli, "", "", prompt, args, idleTimeoutMs, MCPSessionConfig{})
}

// mcpRunSessionWithConfig runs a new CLI session with full configuration
// Uses native session/resume support - no history re-transmission needed
func mcpRunSessionWithConfig(ctx context.Context, cli, role, model, prompt string, args []string, idleTimeoutMs int, config MCPSessionConfig) (map[string]interface{}, error) {
	adapter := mcpGetAdapter(cli)
	if adapter == nil {
		return nil, fmt.Errorf("unknown CLI: %s", cli)
	}

	output, err := adapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: idleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}

	// Extract native thread ID from output (for Codex JSON output)
	nativeThreadID := mcpExtractNativeThreadID(cli, output)

	// Create session - use native thread ID if available, otherwise generate one
	now := time.Now()
	threadID := nativeThreadID
	if threadID == "" {
		threadID = uuid.New().String()
	}

	sess := &MCPSession{
		ID:             threadID,
		NativeThreadID: nativeThreadID,
		CLI:            cli,
		Role:           role,
		Model:          model,
		Config:         config,
		CreatedAt:      now,
		UpdatedAt:      now,
	}

	mcpSessionStoreMu.Lock()
	mcpEvictOldestSession()
	mcpSessionStore[threadID] = sess
	mcpSessionStoreMu.Unlock()

	// Extract text content for response
	textContent := mcpExtractTextContent(cli, output)
	rememberSharedMemory(cli, role, textContent)
	return mcpBuildResponseWithMeta(textContent, threadID, cli, role, model), nil
}

// mcpRunReply continues an existing session using native CLI resume
// NO HISTORY RE-TRANSMISSION - uses native session/resume support
func mcpRunReply(ctx context.Context, input MCPReplyInput) (map[string]interface{}, error) {
	if err := ValidatePrompt(input.Prompt); err != nil {
		return nil, err
	}

	prompt := input.Prompt
	if input.MemoryKey != "" {
		var err error
		prompt, err = applyMemoryToPrompt(prompt, input.MemoryKey, input.MemoryMode)
		if err != nil {
			return nil, err
		}
	}
	prompt = applySharedMemory(prompt)

	threadID := input.ThreadID
	if threadID == "" {
		threadID = input.ConversationID // deprecated fallback
	}
	if threadID == "" {
		return nil, fmt.Errorf("threadId is required")
	}

	mcpSessionStoreMu.RLock()
	sess, exists := mcpSessionStore[threadID]
	mcpSessionStoreMu.RUnlock()

	if !exists {
		return nil, fmt.Errorf("thread not found: %s", threadID)
	}

	adapter := mcpGetAdapter(sess.CLI)
	if adapter == nil {
		return nil, fmt.Errorf("unknown CLI: %s", sess.CLI)
	}

	// Build args using native resume - NO history re-transmission
	args := mcpBuildResumeArgs(sess.CLI, sess.NativeThreadID, prompt, sess.Config)

	output, err := adapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: defaultCLIIdleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}

	// Update session timestamp only (no message storage)
	mcpSessionStoreMu.Lock()
	sess.UpdatedAt = time.Now()
	mcpSessionStoreMu.Unlock()

	textContent := mcpExtractTextContent(sess.CLI, output)
	rememberSharedMemory(sess.CLI, sess.Role, textContent)
	return mcpBuildResponseWithMeta(textContent, threadID, sess.CLI, sess.Role, sess.Model), nil
}

// mcpRunRoleSession runs a role-based session
func mcpRunRoleSession(ctx context.Context, input MCPConductorInput) (map[string]interface{}, error) {
	configPath := resolveConfigPath("")
	cfg, err := loadConfig(configPath)
	if err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
	}
	if cfg.Disabled {
		return nil, fmt.Errorf("conductor is disabled (run `conductor enable` to resume)")
	}

	role, ok := cfg.Roles[input.Role]
	if !ok {
		return nil, fmt.Errorf("unknown role: %s", input.Role)
	}

	cli := role.CLI
	adapter := mcpGetAdapter(cli)
	if adapter == nil {
		return nil, fmt.Errorf("unknown CLI for role %s: %s", input.Role, cli)
	}

	prompt := applySharedMemory(input.Prompt)
	args := mcpBuildRoleArgs(cli, prompt, role.Model, role.Reasoning)

	output, err := adapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: input.IdleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}

	// Extract native thread ID
	nativeThreadID := mcpExtractNativeThreadID(cli, output)

	// Create session with role info
	now := time.Now()
	threadID := nativeThreadID
	if threadID == "" {
		threadID = uuid.New().String()
	}

	sess := &MCPSession{
		ID:             threadID,
		NativeThreadID: nativeThreadID,
		CLI:            cli,
		Role:           input.Role,
		Model:          role.Model,
		Config:         MCPSessionConfig{},
		CreatedAt:      now,
		UpdatedAt:      now,
	}

	mcpSessionStoreMu.Lock()
	mcpEvictOldestSession()
	mcpSessionStore[threadID] = sess
	mcpSessionStoreMu.Unlock()

	textContent := mcpExtractTextContent(cli, output)
	rememberSharedMemory(cli, input.Role, textContent)
	return mcpBuildResponseWithMeta(textContent, threadID, cli, input.Role, role.Model), nil
}

// Helper functions

func mcpGetAdapter(cli string) *CLIAdapter {
	switch cli {
	case "codex":
		return mcpCodexAdapter
	case "claude":
		return mcpClaudeAdapter
	case "gemini":
		return mcpGeminiAdapter
	}
	return nil
}

func mcpBuildCodexArgs(input MCPCodexInput) []string {
	args := []string{"exec", "--json"}

	if input.ApprovalPolicy != "" {
		args = append(args, "--approval-policy", input.ApprovalPolicy)
	}
	if input.Sandbox != "" {
		args = append(args, "--sandbox", input.Sandbox)
	}
	if input.Cwd != "" {
		args = append(args, "--cwd", input.Cwd)
	}
	if input.Model != "" {
		args = append(args, "-m", input.Model)
	}
	if input.Profile != "" {
		args = append(args, "-p", input.Profile)
	}
	if input.Config != nil {
		for key, value := range input.Config {
			args = append(args, "-c", fmt.Sprintf("%s=%v", key, value))
		}
	}
	if input.BaseInstructions != "" {
		args = append(args, "-c", fmt.Sprintf("base_instructions=%q", input.BaseInstructions))
	}
	if input.IncludePlanTool != nil && *input.IncludePlanTool {
		args = append(args, "-c", "include_plan_tool=true")
	}
	// Add reasoning effort for o-series models (o3, o4-mini, etc.)
	if input.ReasoningEffort != "" {
		args = append(args, "-c", fmt.Sprintf("model_reasoning_effort=%s", input.ReasoningEffort))
	}

	args = append(args, input.Prompt)
	return args
}

func mcpBuildClaudeArgs(input MCPClaudeInput) []string {
	permissionMode := strings.TrimSpace(input.PermissionMode)
	if permissionMode == "" {
		permissionMode = "bypassPermissions"
	}

	args := []string{"-p", input.Prompt, "--output-format", "stream-json", "--permission-mode", permissionMode, "--verbose"}

	if input.Model != "" {
		args = append(args, "--model", input.Model)
	}
	if input.AllowedTools != "" {
		args = append(args, "--allowedTools", input.AllowedTools)
	}
	if input.DisallowedTools != "" {
		args = append(args, "--disallowedTools", input.DisallowedTools)
	}
	if input.SystemPrompt != "" {
		args = append(args, "--system-prompt", input.SystemPrompt)
	}
	if input.AppendSystemPrompt != "" {
		args = append(args, "--append-system-prompt", input.AppendSystemPrompt)
	}
	if input.MaxTurns > 0 {
		args = append(args, "--max-turns", fmt.Sprintf("%d", input.MaxTurns))
	}
	if input.Cwd != "" {
		// Claude uses --add-dir for additional directories, cwd is handled via working directory
	}
	if input.AddDir != "" {
		args = append(args, "--add-dir", input.AddDir)
	}
	if input.McpConfig != "" {
		args = append(args, "--mcp-config", input.McpConfig)
	}
	if input.Agents != "" {
		args = append(args, "--agents", input.Agents)
	}
	if input.Debug {
		args = append(args, "--debug")
	}

	return args
}

func mcpBuildGeminiArgs(input MCPGeminiInput) []string {
	args := []string{"-p", input.Prompt, "--output-format", "stream-json"}

	if input.Model != "" {
		args = append(args, "-m", input.Model)
	}
	if input.Sandbox != "" {
		args = append(args, "--sandbox", input.Sandbox)
	}
	if input.Yolo {
		args = append(args, "--yolo")
	}
	if input.ApprovalMode != "" {
		args = append(args, "--approval-mode", input.ApprovalMode)
	}
	if input.IncludeDirectories != "" {
		args = append(args, "--include-directories", input.IncludeDirectories)
	}
	if input.Cwd != "" {
		// Gemini uses working directory from where it's run
		// We'll handle this via the adapter's working directory
	}
	if input.Debug {
		args = append(args, "--debug")
	}

	return args
}

func mcpBuildReplyArgs(cli, contextPrompt string) []string {
	return mcpBuildReplyArgsWithConfig(cli, contextPrompt, MCPSessionConfig{})
}

func mcpBuildReplyArgsWithConfig(cli, contextPrompt string, config MCPSessionConfig) []string {
	switch cli {
	case "codex":
		args := []string{"exec", "--json"}
		// Preserve original session settings
		if config.ApprovalPolicy != "" {
			args = append(args, "--approval-policy", config.ApprovalPolicy)
		}
		if config.Sandbox != "" {
			args = append(args, "--sandbox", config.Sandbox)
		}
		if config.Cwd != "" {
			args = append(args, "--cwd", config.Cwd)
		}
		if config.Profile != "" {
			args = append(args, "-p", config.Profile)
		}
		args = append(args, contextPrompt)
		return args
	case "claude":
		permissionMode := config.PermissionMode
		if permissionMode == "" {
			permissionMode = "dontAsk"
		}
		args := []string{"-p", contextPrompt, "--output-format", "stream-json", "--permission-mode", permissionMode, "--verbose"}
		if config.AllowedTools != "" {
			args = append(args, "--allowed-tools", config.AllowedTools)
		}
		if config.DisallowedTools != "" {
			args = append(args, "--disallowed-tools", config.DisallowedTools)
		}
		if config.SystemPrompt != "" {
			args = append(args, "--system-prompt", config.SystemPrompt)
		}
		if config.AppendSystemPrompt != "" {
			args = append(args, "--append-system-prompt", config.AppendSystemPrompt)
		}
		return args
	case "gemini":
		args := []string{"-p", contextPrompt, "--output-format", "stream-json"}
		if config.Sandbox != "" {
			args = append(args, "--sandbox", config.Sandbox)
		}
		if config.Yolo {
			args = append(args, "--yolo")
		}
		if config.ApprovalMode != "" {
			args = append(args, "--approval-mode", config.ApprovalMode)
		}
		if config.IncludeDirectories != "" {
			args = append(args, "--include-directories", config.IncludeDirectories)
		}
		return args
	}
	return []string{contextPrompt}
}

func mcpBuildRoleArgs(cli, prompt, model, reasoning string) []string {
	switch cli {
	case "codex":
		args := []string{"exec", "--json"}
		if model != "" {
			args = append(args, "-m", model)
		}
		// Reasoning effort for o-series models - no quotes needed
		if reasoning != "" {
			args = append(args, "-c", fmt.Sprintf("model_reasoning_effort=%s", reasoning))
		}
		args = append(args, prompt)
		return args
	case "claude":
		args := []string{"-p", prompt, "--output-format", "stream-json", "--permission-mode", "bypassPermissions", "--verbose"}
		if model != "" {
			args = append(args, "--model", model)
		}
		return args
	case "gemini":
		args := []string{"--output-format", "stream-json", "--sandbox"}
		if model != "" {
			args = append(args, "-m", model)
		}
		args = append(args, prompt)
		return args
	}
	return []string{prompt}
}

// mcpBuildResumeArgs builds arguments for native CLI resume (no history re-transmission)
func mcpBuildResumeArgs(cli, nativeThreadID, prompt string, config MCPSessionConfig) []string {
	switch cli {
	case "codex":
		// Codex: codex exec resume <session-id> [prompt]
		args := []string{"exec", "resume"}
		if nativeThreadID != "" {
			args = append(args, nativeThreadID)
		} else {
			args = append(args, "--last")
		}
		args = append(args, "--json")
		if config.ApprovalPolicy != "" {
			args = append(args, "--approval-policy", config.ApprovalPolicy)
		}
		if config.Sandbox != "" {
			args = append(args, "--sandbox", config.Sandbox)
		}
		args = append(args, prompt)
		return args

	case "claude":
		// Claude: claude --resume <session-id> -p <prompt>
		args := []string{"--output-format", "stream-json"}
		if nativeThreadID != "" {
			args = append(args, "--resume", nativeThreadID)
		} else {
			args = append(args, "--continue")
		}
		permissionMode := config.PermissionMode
		if permissionMode == "" {
			permissionMode = "bypassPermissions"
		}
		args = append(args, "--permission-mode", permissionMode, "--verbose")
		args = append(args, "-p", prompt)
		return args

	case "gemini":
		// Gemini: supports --resume with session UUID or index
		// See: https://geminicli.com/docs/cli/session-management/
		args := []string{"--output-format", "stream-json"}
		if nativeThreadID != "" {
			args = append(args, "--resume", nativeThreadID)
		} else {
			// Fall back to latest session if no ID available
			args = append(args, "--resume")
		}
		if config.Yolo {
			args = append(args, "--yolo")
		}
		if config.ApprovalMode != "" {
			args = append(args, "--approval-mode", config.ApprovalMode)
		}
		args = append(args, prompt)
		return args
	}
	return []string{prompt}
}

// mcpExtractNativeThreadID extracts the native thread/session ID from CLI output
func mcpExtractNativeThreadID(cli, output string) string {
	if output == "" {
		return ""
	}

	lines := strings.Split(output, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		var event map[string]interface{}
		if err := json.Unmarshal([]byte(line), &event); err != nil {
			continue
		}

		switch cli {
		case "codex":
			// Codex JSON output: look for session_id or thread_id in various locations
			if sessionID, ok := event["session_id"].(string); ok && sessionID != "" {
				return sessionID
			}
			if threadID, ok := event["thread_id"].(string); ok && threadID != "" {
				return threadID
			}
			// Check structuredContent.threadId pattern
			if structured, ok := event["structuredContent"].(map[string]interface{}); ok {
				if threadID, ok := structured["threadId"].(string); ok && threadID != "" {
					return threadID
				}
			}

		case "claude":
			// Claude stream-json: look for session_id in system events
			if sessionID, ok := event["session_id"].(string); ok && sessionID != "" {
				return sessionID
			}
			if eventType, ok := event["type"].(string); ok && eventType == "system" {
				if sessionID, ok := event["session_id"].(string); ok && sessionID != "" {
					return sessionID
				}
			}

		case "gemini":
			// Gemini: look for session identifier
			if sessionID, ok := event["session_id"].(string); ok && sessionID != "" {
				return sessionID
			}
		}
	}

	return ""
}

// mcpExtractTextContent extracts user-facing text from CLI output
func mcpExtractTextContent(cli, output string) string {
	// For now, delegate to existing mcpExtractText which handles JSON parsing
	return mcpExtractText(output)
}

func mcpBuildResponse(output, threadID string) map[string]interface{} {
	return mcpBuildResponseWithMeta(output, threadID, "", "", "")
}

func mcpBuildResponseWithMeta(output, threadID, cli, role, model string) map[string]interface{} {
	textContent := mcpExtractText(output)

	structured := map[string]interface{}{
		"threadId": threadID,
	}
	if cli != "" {
		structured["cli"] = cli
	}
	if role != "" {
		structured["role"] = role
	}
	if model != "" {
		structured["model"] = model
	}

	return map[string]interface{}{
		"content": []map[string]interface{}{
			{"type": "text", "text": textContent},
		},
		"structuredContent": structured,
	}
}

func mcpExtractText(output string) string {
	if output == "" {
		return ""
	}

	lines := strings.Split(output, "\n")
	var texts []string

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		var event map[string]interface{}
		if err := json.Unmarshal([]byte(line), &event); err == nil {
			if eventType, ok := event["type"].(string); ok {
				switch eventType {
				case "message":
					// Gemini format: only extract assistant messages, skip user messages
					if role, ok := event["role"].(string); ok && role == "user" {
						continue
					}
					if content, ok := event["content"].(string); ok {
						texts = append(texts, content)
					}
				case "response.output_text.done":
					if text, ok := event["text"].(string); ok {
						texts = append(texts, text)
					}
				case "result":
					if result, ok := event["result"].(string); ok {
						texts = append(texts, result)
					}
				case "item.completed":
					// Codex JSON format: {"type":"item.completed","item":{"type":"agent_message","text":"..."}}
					if item, ok := event["item"].(map[string]interface{}); ok {
						if itemType, ok := item["type"].(string); ok && itemType == "agent_message" {
							if text, ok := item["text"].(string); ok {
								texts = append(texts, text)
							}
						}
					}
				}
			}
			continue
		}
		texts = append(texts, line)
	}

	if len(texts) > 0 {
		return strings.Join(texts, "\n")
	}
	return output
}
