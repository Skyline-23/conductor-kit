package main

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

const defaultCLIIdleTimeoutMs = 120000

// CLIAdapter provides common functionality for running CLI commands with idle timeout.
type CLIAdapter struct {
	Name string
	Cmd  string
}

// CLIRunOptions contains options for running a CLI command.
type CLIRunOptions struct {
	Args          []string
	IdleTimeoutMs int
}

// Run executes a CLI command with idle timeout support.
// The idle timer resets whenever output is received.
func (a *CLIAdapter) Run(ctx context.Context, opts CLIRunOptions) (string, error) {
	if !isCommandAvailable(a.Cmd) {
		return "", fmt.Errorf("%s CLI not found", a.Name)
	}

	// Setup cancellable context
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	// Setup idle timeout
	idleTimeout := time.Duration(opts.IdleTimeoutMs) * time.Millisecond
	if idleTimeout <= 0 {
		idleTimeout = time.Duration(defaultCLIIdleTimeoutMs) * time.Millisecond
	}
	activityCh := make(chan struct{}, 1)
	var idleTimedOut atomic.Bool
	stopIdle := startIdleTimer(ctx, idleTimeout, activityCh, func() {
		idleTimedOut.Store(true)
		cancel()
	})
	defer stopIdle()

	cmd := exec.CommandContext(ctx, a.Cmd, opts.Args...)
	stdoutPipe, err := cmd.StdoutPipe()
	if err != nil {
		return "", err
	}
	stderrPipe, err := cmd.StderrPipe()
	if err != nil {
		return "", err
	}

	var output bytes.Buffer
	outputWriter := &activityWriter{w: &output, activityCh: activityCh}

	if err := cmd.Start(); err != nil {
		return "", err
	}

	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		_, _ = io.Copy(outputWriter, stdoutPipe)
		wg.Done()
	}()
	go func() {
		_, _ = io.Copy(outputWriter, stderrPipe)
		wg.Done()
	}()

	err = cmd.Wait()
	wg.Wait()

	if idleTimedOut.Load() {
		return "", fmt.Errorf("%s CLI idle timed out (no output for %v)", a.Name, idleTimeout)
	}
	if ctx.Err() == context.DeadlineExceeded {
		return "", fmt.Errorf("%s CLI timed out", a.Name)
	}
	if err != nil {
		outputStr := strings.TrimSpace(output.String())
		// Extract concise error - avoid dumping entire output to prevent token explosion
		errMsg := extractConciseError(outputStr, err)
		return "", fmt.Errorf("%s CLI failed: %s", a.Name, errMsg)
	}
	return strings.TrimSpace(output.String()), nil
}

// extractConciseError extracts a concise error message from CLI output.
// Avoids including full output to prevent token explosion on retries.
func extractConciseError(output string, err error) string {
	lowerOutput := strings.ToLower(output)

	// Check for quota errors (Google/Gemini specific)
	if strings.Contains(lowerOutput, "quota") || strings.Contains(lowerOutput, "quotaerror") ||
		strings.Contains(lowerOutput, "resource_exhausted") || strings.Contains(lowerOutput, "resourceexhausted") {
		return "quota exceeded - please wait or check your Google API quota"
	}

	// Check for rate limit errors
	if strings.Contains(lowerOutput, "rate limit") || strings.Contains(lowerOutput, "rate_limit") ||
		strings.Contains(lowerOutput, "too many requests") || strings.Contains(lowerOutput, "429") {
		return "rate limit exceeded - please wait before retrying"
	}

	// Check for auth errors
	if strings.Contains(lowerOutput, "unauthorized") || strings.Contains(lowerOutput, "authentication") ||
		strings.Contains(lowerOutput, "api key") || strings.Contains(lowerOutput, "401") {
		return "authentication failed - check API key"
	}

	// Check for common error patterns in JSON output
	lines := strings.Split(output, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		// Look for error messages in the last few lines
		if strings.Contains(strings.ToLower(line), "error") && len(line) < 200 {
			return line
		}
	}

	// Return just the underlying error, not the full output
	if err != nil {
		return err.Error()
	}

	// Truncate output if we must include it
	if len(output) > 200 {
		return output[:200] + "... (truncated)"
	}
	return output
}

// ValidatePrompt checks if prompt is non-empty.
func ValidatePrompt(prompt string) error {
	if strings.TrimSpace(prompt) == "" {
		return errors.New("prompt is required")
	}
	return nil
}

// SplitModels splits a comma-separated model list.
func SplitModels(models string) []string {
	parts := strings.Split(models, ",")
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		trimmed := strings.TrimSpace(part)
		if trimmed != "" {
			out = append(out, trimmed)
		}
	}
	return out
}
