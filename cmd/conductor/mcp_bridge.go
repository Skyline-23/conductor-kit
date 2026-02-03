package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"

	"github.com/google/jsonschema-go/jsonschema"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

const mcpBridgeConnectTimeout = 15 * time.Second

type mcpBridgeMode struct {
	Codex  bool
	Claude bool
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
		case "codex":
			mode.Codex = true
		case "claude", "claude-code", "claude_code":
			mode.Claude = true
		}
	}
	return mode
}

func registerMCPBridges(server *mcp.Server, mode mcpBridgeMode) error {
	if !mode.Codex && !mode.Claude {
		return nil
	}

	if mode.Codex {
		bridge := newMcpBridgeClient("codex", "codex", []string{"mcp-server"})
		if err := registerBridgeTools(server, bridge, "codex", map[string]bool{
			"codex":       true,
			"codex-reply": true,
		}); err != nil {
			return err
		}
	}

	if mode.Claude {
		bridge := newMcpBridgeClient("claude", "claude", []string{"mcp", "serve"})
		if err := registerBridgeTools(server, bridge, "claude", map[string]bool{
			"claude":       true,
			"claude-reply": true,
		}); err != nil {
			return err
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

func registerBridgeTools(server *mcp.Server, bridge *mcpBridgeClient, prefix string, overrides map[string]bool) error {
	ctx := context.Background()
	tools, err := bridge.ensureTools(ctx)
	if err != nil {
		return err
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
