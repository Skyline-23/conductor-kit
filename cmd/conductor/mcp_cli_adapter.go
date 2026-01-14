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
		if outputStr != "" {
			return "", fmt.Errorf("%s CLI failed: %w\n%s", a.Name, err, outputStr)
		}
		return "", fmt.Errorf("%s CLI failed: %w", a.Name, err)
	}
	return strings.TrimSpace(output.String()), nil
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
