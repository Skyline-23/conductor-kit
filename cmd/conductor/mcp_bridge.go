package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/google/jsonschema-go/jsonschema"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

const (
	mcpBridgeConnectTimeout = 15 * time.Second
	defaultBridgeCacheTTL   = 30 * time.Second
)

type mcpBridgeMode struct {
	Codex  bool
	Claude bool
}

var (
	mcpBridgeCodex       *mcpBridgeClient
	mcpBridgeClaude      *mcpBridgeClient
	mcpBridgeStatusCache = struct {
		mu       sync.Mutex
		expires  time.Time
		statuses []map[string]interface{}
		ok       bool
		modeKey  string
	}{}
)

func bridgeCacheTTL() time.Duration {
	if val := strings.TrimSpace(os.Getenv("CONDUCTOR_BRIDGE_CACHE_TTL")); val != "" {
		if parsed, err := time.ParseDuration(val); err == nil && parsed > 0 {
			return parsed
		}
	}
	return defaultBridgeCacheTTL
}

func resolveBridgeMode() mcpBridgeMode {
	mode := mcpBridgeMode{Codex: true, Claude: true}
	if list := strings.TrimSpace(os.Getenv("CONDUCTOR_BRIDGE")); list != "" {
		mode = parseMCPBridgeMode(list, false, false)
	}
	return mode
}

func bridgeModeTargets(mode mcpBridgeMode) []string {
	targets := []string{}
	if mode.Codex {
		targets = append(targets, "codex")
	}
	if mode.Claude {
		targets = append(targets, "claude")
	}
	return targets
}

func bridgeModeLabel(mode mcpBridgeMode) string {
	targets := bridgeModeTargets(mode)
	if len(targets) == 0 {
		return "none"
	}
	return strings.Join(targets, ",")
}

func parseMCPBridgeMode(list string, codexFlag, claudeFlag bool) mcpBridgeMode {
	mode := mcpBridgeMode{Codex: codexFlag, Claude: claudeFlag}
	if strings.TrimSpace(list) == "" {
		return mode
	}
	for _, part := range strings.Split(list, ",") {
		key := strings.TrimSpace(strings.ToLower(part))
		switch key {
		case "all":
			mode.Codex = true
			mode.Claude = true
		case "none", "false", "0":
			mode.Codex = false
			mode.Claude = false
		case "codex":
			mode.Codex = true
		case "claude", "claude-code", "claude_code":
			mode.Claude = true
		}
	}
	return mode
}

func registerMCPBridges(server *mcp.Server, mode mcpBridgeMode, strict bool) error {
	if !mode.Codex && !mode.Claude {
		return nil
	}

	if mode.Codex {
		if !isCommandAvailable("codex") {
			if strict {
				return fmt.Errorf("%s CLI not found for MCP bridge", bridgeTitle("codex"))
			}
		} else {
			bridge := newMcpBridgeClient("codex", "codex", []string{"mcp-server"})
			if err := registerBridgeTools(server, bridge, "codex", map[string]bool{
				"codex":       true,
				"codex-reply": true,
			}); err != nil {
				if strict {
					return err
				}
				fmt.Fprintf(os.Stderr, "Warning: Codex MCP bridge unavailable: %v\n", err)
			} else {
				if !bridgeHasTool(bridge, "codex") || !bridgeHasTool(bridge, "codex-reply") {
					return fmt.Errorf("Codex MCP bridge missing codex/codex-reply tool")
				}
				mcpBridgeCodex = bridge
			}
		}
	}

	if mode.Claude {
		if !isCommandAvailable("claude") {
			if strict {
				return fmt.Errorf("%s CLI not found for MCP bridge", bridgeTitle("claude"))
			}
		} else {
			bridge := newMcpBridgeClient("claude", "claude", []string{"mcp", "serve"})
			if err := registerBridgeTools(server, bridge, "claude", map[string]bool{}); err != nil {
				if strict {
					return err
				}
				fmt.Fprintf(os.Stderr, "Warning: Claude MCP bridge unavailable: %v\n", err)
			} else {
				mcpBridgeClaude = bridge
			}
		}
	}

	return nil
}

type mcpBridgeClient struct {
	name    string
	cmd     string
	args    []string
	mu      sync.Mutex
	client  *mcp.Client
	session *mcp.ClientSession
	tools   map[string]*mcp.Tool
}

func newMcpBridgeClient(name, cmd string, args []string) *mcpBridgeClient {
	return &mcpBridgeClient{name: name, cmd: cmd, args: args}
}

func mcpBridgeClientForCLI(cli string) (*mcpBridgeClient, error) {
	switch strings.ToLower(strings.TrimSpace(cli)) {
	case "codex":
		if mcpBridgeCodex == nil {
			return nil, fmt.Errorf("Codex MCP bridge is not available")
		}
		return mcpBridgeCodex, nil
	case "claude":
		if mcpBridgeClaude == nil {
			return nil, fmt.Errorf("Claude MCP bridge is not available")
		}
		return mcpBridgeClaude, nil
	default:
		return nil, fmt.Errorf("unsupported MCP bridge CLI: %s", cli)
	}
}

func (b *mcpBridgeClient) ensureSession(ctx context.Context) (*mcp.ClientSession, error) {
	b.mu.Lock()
	if b.session != nil {
		sess := b.session
		b.mu.Unlock()
		return sess, nil
	}
	b.mu.Unlock()

	if !isCommandAvailable(b.cmd) {
		return nil, fmt.Errorf("%s CLI not found for MCP bridge", bridgeTitle(b.name))
	}

	ctx, cancel := context.WithTimeout(ctx, mcpBridgeConnectTimeout)
	defer cancel()

	client := mcp.NewClient(&mcp.Implementation{Name: "conductor-mcp-bridge", Version: Version}, nil)
	cmd := exec.Command(b.cmd, b.args...)
	cmd.Env = append(os.Environ(), "CI=1", "NO_COLOR=1")
	cmd.Stderr = os.Stderr
	transport := &mcp.CommandTransport{Command: cmd}
	session, err := client.Connect(ctx, transport, nil)
	if err != nil {
		return nil, err
	}

	b.mu.Lock()
	b.client = client
	b.session = session
	b.mu.Unlock()

	return session, nil
}

func (b *mcpBridgeClient) ensureTools(ctx context.Context) (map[string]*mcp.Tool, error) {
	b.mu.Lock()
	if b.tools != nil {
		tools := b.tools
		b.mu.Unlock()
		return tools, nil
	}
	b.mu.Unlock()

	session, err := b.ensureSession(ctx)
	if err != nil {
		return nil, err
	}

	all := make(map[string]*mcp.Tool)
	cursor := ""
	for {
		res, err := session.ListTools(ctx, &mcp.ListToolsParams{Cursor: cursor})
		if err != nil {
			return nil, err
		}
		for _, tool := range res.Tools {
			if tool == nil || tool.Name == "" {
				continue
			}
			all[tool.Name] = tool
		}
		if res.NextCursor == "" {
			break
		}
		cursor = res.NextCursor
	}

	b.mu.Lock()
	b.tools = all
	b.mu.Unlock()

	return all, nil
}

func (b *mcpBridgeClient) CallTool(ctx context.Context, name string, args map[string]any) (*mcp.CallToolResult, error) {
	session, err := b.ensureSession(ctx)
	if err != nil {
		return nil, err
	}
	if args == nil {
		args = map[string]any{}
	}
	return session.CallTool(ctx, &mcp.CallToolParams{Name: name, Arguments: args})
}

func (b *mcpBridgeClient) CallToolAny(ctx context.Context, name string, args any) (*mcp.CallToolResult, error) {
	session, err := b.ensureSession(ctx)
	if err != nil {
		return nil, err
	}
	return session.CallTool(ctx, &mcp.CallToolParams{Name: name, Arguments: args})
}

func registerBridgeTools(server *mcp.Server, bridge *mcpBridgeClient, prefix string, overrides map[string]bool) error {
	ctx := context.Background()
	tools, err := bridge.ensureTools(ctx)
	if err != nil {
		return err
	}
	if len(tools) == 0 {
		return fmt.Errorf("%s MCP bridge returned no tools", bridgeTitle(bridge.name))
	}

	for name, tool := range tools {
		localName := fmt.Sprintf("%s__%s", prefix, name)
		if overrides[name] {
			localName = name
		}

		proxyTool := *tool
		proxyTool.Name = localName
		proxyTool.Description = bridgeToolDescription(tool.Description, bridge.name)
		proxyTool.InputSchema = normalizeBridgeSchema(tool.InputSchema)
		if proxyTool.OutputSchema != nil && proxyTool.OutputSchema.Type != "object" {
			proxyTool.OutputSchema = nil
		}

		upstreamName := name
		server.AddTool(&proxyTool, func(ctx context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			args, err := bridgeDecodeArgs(req)
			if err != nil {
				return nil, err
			}
			return bridge.CallTool(ctx, upstreamName, args)
		})
	}

	return nil
}

func bridgeHasTool(bridge *mcpBridgeClient, name string) bool {
	if bridge == nil {
		return false
	}
	bridge.mu.Lock()
	tools := bridge.tools
	bridge.mu.Unlock()
	if tools == nil {
		return false
	}
	_, ok := tools[name]
	return ok
}

func bridgeStatusPayload() ([]map[string]interface{}, bool) {
	mode := resolveBridgeMode()
	modeKey := fmt.Sprintf("codex=%t;claude=%t", mode.Codex, mode.Claude)

	now := time.Now()
	mcpBridgeStatusCache.mu.Lock()
	if len(mcpBridgeStatusCache.statuses) > 0 && now.Before(mcpBridgeStatusCache.expires) && mcpBridgeStatusCache.modeKey == modeKey {
		cached := cloneBridgeStatuses(mcpBridgeStatusCache.statuses)
		ok := mcpBridgeStatusCache.ok
		mcpBridgeStatusCache.mu.Unlock()
		return cached, ok
	}
	mcpBridgeStatusCache.mu.Unlock()

	statuses := []map[string]interface{}{}
	ok := true

	if mode.Codex {
		codexStatus, codexOK := probeMCPBridge("codex", "codex", []string{"mcp-server"}, []string{"codex", "codex-reply"})
		statuses = append(statuses, codexStatus)
		if !codexOK {
			ok = false
		}
	}

	if mode.Claude {
		claudeStatus, claudeOK := probeMCPBridge("claude", "claude", []string{"mcp", "serve"}, nil)
		statuses = append(statuses, claudeStatus)
		if !claudeOK {
			ok = false
		}
	}

	mcpBridgeStatusCache.mu.Lock()
	mcpBridgeStatusCache.statuses = cloneBridgeStatuses(statuses)
	mcpBridgeStatusCache.ok = ok
	mcpBridgeStatusCache.modeKey = modeKey
	mcpBridgeStatusCache.expires = time.Now().Add(bridgeCacheTTL())
	mcpBridgeStatusCache.mu.Unlock()

	return statuses, ok
}

func probeMCPBridge(name, cmd string, args []string, requiredTools []string) (map[string]interface{}, bool) {
	entry := map[string]interface{}{
		"name": name,
	}
	if !isCommandAvailable(cmd) {
		entry["status"] = "missing"
		entry["error"] = "CLI not found: " + cmd
		return entry, false
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	tools, err := listBridgeTools(ctx, cmd, args)
	if err != nil {
		entry["status"] = "error"
		entry["error"] = err.Error()
		return entry, false
	}
	if len(tools) == 0 {
		entry["status"] = "error"
		entry["error"] = "no tools returned"
		return entry, false
	}

	toolNames := make([]string, 0, len(tools))
	for name := range tools {
		toolNames = append(toolNames, name)
	}
	sort.Strings(toolNames)

	missing := []string{}
	for _, req := range requiredTools {
		if _, ok := tools[req]; !ok {
			missing = append(missing, req)
		}
	}
	if len(missing) > 0 {
		entry["status"] = "error"
		entry["missing_tools"] = missing
		entry["tool_count"] = len(toolNames)
		entry["tools"] = toolNames
		return entry, false
	}

	entry["status"] = "ready"
	entry["tool_count"] = len(toolNames)
	entry["tools"] = toolNames
	return entry, true
}

func listBridgeTools(ctx context.Context, cmd string, args []string) (map[string]*mcp.Tool, error) {
	client := mcp.NewClient(&mcp.Implementation{Name: "conductor-bridge-probe", Version: Version}, nil)
	command := exec.Command(cmd, args...)
	command.Env = append(os.Environ(), "CI=1", "NO_COLOR=1")
	command.Stderr = io.Discard
	transport := &mcp.CommandTransport{Command: command}

	session, err := client.Connect(ctx, transport, nil)
	if err != nil {
		return nil, err
	}
	defer session.Close()

	all := make(map[string]*mcp.Tool)
	cursor := ""
	for {
		res, err := session.ListTools(ctx, &mcp.ListToolsParams{Cursor: cursor})
		if err != nil {
			return nil, err
		}
		for _, tool := range res.Tools {
			if tool == nil || tool.Name == "" {
				continue
			}
			all[tool.Name] = tool
		}
		if res.NextCursor == "" {
			break
		}
		cursor = res.NextCursor
	}
	return all, nil
}

func cloneBridgeStatuses(statuses []map[string]interface{}) []map[string]interface{} {
	if len(statuses) == 0 {
		return nil
	}
	cloned := make([]map[string]interface{}, 0, len(statuses))
	for _, entry := range statuses {
		if entry == nil {
			cloned = append(cloned, nil)
			continue
		}
		next := map[string]interface{}{}
		for key, value := range entry {
			switch v := value.(type) {
			case []string:
				next[key] = append([]string{}, v...)
			default:
				next[key] = v
			}
		}
		cloned = append(cloned, next)
	}
	return cloned
}

func bridgeDecodeArgs(req *mcp.CallToolRequest) (map[string]any, error) {
	args := map[string]any{}
	if req == nil || req.Params == nil {
		return args, nil
	}
	if len(req.Params.Arguments) == 0 {
		return args, nil
	}
	if err := json.Unmarshal(req.Params.Arguments, &args); err != nil {
		return nil, err
	}
	if args == nil {
		args = map[string]any{}
	}
	return args, nil
}

func normalizeBridgeSchema(schema *jsonschema.Schema) *jsonschema.Schema {
	if schema == nil {
		return &jsonschema.Schema{Type: "object"}
	}
	if schema.Type == "" {
		schema.Type = "object"
	}
	if schema.Type != "object" {
		return &jsonschema.Schema{Type: "object"}
	}
	return schema
}

func bridgeToolDescription(desc, bridge string) string {
	if strings.TrimSpace(desc) == "" {
		return fmt.Sprintf("Bridged from %s MCP server.", bridgeTitle(bridge))
	}
	return fmt.Sprintf("%s (via %s MCP bridge)", desc, bridgeTitle(bridge))
}

func bridgeTitle(name string) string {
	trimmed := strings.TrimSpace(name)
	if trimmed == "" {
		return ""
	}
	return strings.ToUpper(trimmed[:1]) + trimmed[1:]
}
