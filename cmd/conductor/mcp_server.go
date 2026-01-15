package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"sync"

	"github.com/google/uuid"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// Session management for multi-turn conversations (matches OpenAI Codex MCP pattern)
var (
	mcpSessionStore   = make(map[string]*MCPSession)
	mcpSessionStoreMu sync.RWMutex
)

// MCPSession represents a conversation session
type MCPSession struct {
	ID       string
	CLI      string // codex, claude, gemini
	Messages []MCPMessage
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
	IdleTimeoutMs      int    `json:"idle_timeout_ms,omitempty"`
}

// MCPGeminiInput for gemini tool
type MCPGeminiInput struct {
	Prompt        string `json:"prompt"`
	Model         string `json:"model,omitempty"`
	Sandbox       string `json:"sandbox,omitempty"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
}

// MCPReplyInput for *-reply tools
type MCPReplyInput struct {
	Prompt         string `json:"prompt"`
	ThreadID       string `json:"threadId"`
	ConversationID string `json:"conversationId,omitempty"` // deprecated alias
}

// MCPConductorInput for role-based routing
type MCPConductorInput struct {
	Prompt        string `json:"prompt"`
	Role          string `json:"role"`
	IdleTimeoutMs int    `json:"idle_timeout_ms,omitempty"`
}

func runMCPServer(args []string) int {
	_ = args
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
- include-plan-tool: Include plan tool in conversation`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPCodexInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		result, err := mcpRunSession(ctx, "codex", input.Prompt, mcpBuildCodexArgs(input), input.IdleTimeoutMs)
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
- threadId (required): Thread ID from previous response`,
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
		Description: `Run a Claude session. Returns structuredContent.threadId for continuation.

Parameters:
- prompt (required): The user prompt
- model: Model override
- permission-mode: "default", "acceptEdits", "bypassPermissions", "dontAsk"
- allowed-tools: Comma-separated tool names
- disallowed-tools: Comma-separated tool names
- system-prompt: Custom system prompt
- append-system-prompt: Append to system prompt`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPClaudeInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		result, err := mcpRunSession(ctx, "claude", input.Prompt, mcpBuildClaudeArgs(input), input.IdleTimeoutMs)
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
- threadId (required): Thread ID from previous response`,
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
- model: Model override
- sandbox: Sandbox mode`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPGeminiInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		result, err := mcpRunSession(ctx, "gemini", input.Prompt, mcpBuildGeminiArgs(input), input.IdleTimeoutMs)
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
- threadId (required): Thread ID from previous response`,
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

Available roles are defined in ~/.conductor-kit/conductor.json`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPConductorInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		if err := ValidatePrompt(input.Prompt); err != nil {
			return nil, nil, err
		}
		if input.Role == "" {
			return nil, nil, fmt.Errorf("role is required")
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
- threadId (required): Thread ID from previous response`,
	}, func(ctx context.Context, req *mcp.CallToolRequest, input MCPReplyInput) (*mcp.CallToolResult, map[string]interface{}, error) {
		result, err := mcpRunReply(ctx, input)
		if err != nil {
			return nil, nil, err
		}
		return nil, result, nil
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

// mcpRunSession runs a new CLI session and creates a thread
func mcpRunSession(ctx context.Context, cli, prompt string, args []string, idleTimeoutMs int) (map[string]interface{}, error) {
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

	// Create session
	threadID := uuid.New().String()
	sess := &MCPSession{
		ID:  threadID,
		CLI: cli,
		Messages: []MCPMessage{
			{Role: "user", Content: prompt},
			{Role: "assistant", Content: output},
		},
	}

	mcpSessionStoreMu.Lock()
	mcpSessionStore[threadID] = sess
	mcpSessionStoreMu.Unlock()

	return mcpBuildResponse(output, threadID), nil
}

// mcpRunReply continues an existing session
func mcpRunReply(ctx context.Context, input MCPReplyInput) (map[string]interface{}, error) {
	if err := ValidatePrompt(input.Prompt); err != nil {
		return nil, err
	}

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

	// Build context prompt from history
	contextPrompt := mcpBuildContextPrompt(sess.Messages, input.Prompt)
	args := mcpBuildReplyArgs(sess.CLI, contextPrompt)

	output, err := adapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: defaultCLIIdleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}

	// Update session
	mcpSessionStoreMu.Lock()
	sess.Messages = append(sess.Messages,
		MCPMessage{Role: "user", Content: input.Prompt},
		MCPMessage{Role: "assistant", Content: output},
	)
	mcpSessionStoreMu.Unlock()

	return mcpBuildResponse(output, threadID), nil
}

// mcpRunRoleSession runs a role-based session
func mcpRunRoleSession(ctx context.Context, input MCPConductorInput) (map[string]interface{}, error) {
	configPath := resolveConfigPath("")
	cfg, err := loadConfig(configPath)
	if err != nil {
		return nil, fmt.Errorf("failed to load config: %w", err)
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

	args := mcpBuildRoleArgs(cli, input.Prompt, role.Model, role.Reasoning)

	output, err := adapter.Run(ctx, CLIRunOptions{
		Args:          args,
		IdleTimeoutMs: input.IdleTimeoutMs,
	})
	if err != nil {
		return nil, err
	}

	// Create session
	threadID := uuid.New().String()
	sess := &MCPSession{
		ID:  threadID,
		CLI: cli,
		Messages: []MCPMessage{
			{Role: "user", Content: input.Prompt},
			{Role: "assistant", Content: output},
		},
	}

	mcpSessionStoreMu.Lock()
	mcpSessionStore[threadID] = sess
	mcpSessionStoreMu.Unlock()

	return mcpBuildResponse(output, threadID), nil
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

	args = append(args, input.Prompt)
	return args
}

func mcpBuildClaudeArgs(input MCPClaudeInput) []string {
	permissionMode := strings.TrimSpace(input.PermissionMode)
	if permissionMode == "" {
		permissionMode = "dontAsk"
	}

	args := []string{"-p", input.Prompt, "--output-format", "stream-json", "--permission-mode", permissionMode, "--verbose"}

	if input.Model != "" {
		args = append(args, "--model", input.Model)
	}
	if input.AllowedTools != "" {
		args = append(args, "--allowed-tools", input.AllowedTools)
	}
	if input.DisallowedTools != "" {
		args = append(args, "--disallowed-tools", input.DisallowedTools)
	}
	if input.SystemPrompt != "" {
		args = append(args, "--system-prompt", input.SystemPrompt)
	}
	if input.AppendSystemPrompt != "" {
		args = append(args, "--append-system-prompt", input.AppendSystemPrompt)
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

	return args
}

func mcpBuildReplyArgs(cli, contextPrompt string) []string {
	switch cli {
	case "codex":
		return []string{"exec", "--json", contextPrompt}
	case "claude":
		return []string{"-p", contextPrompt, "--output-format", "stream-json", "--permission-mode", "dontAsk", "--verbose"}
	case "gemini":
		return []string{"-p", contextPrompt, "--output-format", "stream-json"}
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
		if reasoning != "" {
			args = append(args, "-c", fmt.Sprintf("model_reasoning_effort=\"%s\"", reasoning))
		}
		args = append(args, prompt)
		return args
	case "claude":
		args := []string{"-p", prompt, "--output-format", "stream-json", "--permission-mode", "dontAsk", "--verbose"}
		if model != "" {
			args = append(args, "--model", model)
		}
		return args
	case "gemini":
		args := []string{"-p", prompt, "--output-format", "stream-json"}
		if model != "" {
			args = append(args, "-m", model)
		}
		return args
	}
	return []string{prompt}
}

func mcpBuildContextPrompt(messages []MCPMessage, newPrompt string) string {
	var sb strings.Builder
	sb.WriteString("Previous conversation:\n")
	for _, msg := range messages {
		sb.WriteString(fmt.Sprintf("[%s]: %s\n", msg.Role, msg.Content))
	}
	sb.WriteString("\nCurrent request:\n")
	sb.WriteString(newPrompt)
	return sb.String()
}

func mcpBuildResponse(output, threadID string) map[string]interface{} {
	textContent := mcpExtractText(output)

	return map[string]interface{}{
		"content": []map[string]interface{}{
			{"type": "text", "text": textContent},
		},
		"structuredContent": map[string]interface{}{
			"threadId": threadID,
		},
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
