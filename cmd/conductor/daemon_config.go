package main

import (
	"fmt"
	"os"
	"strconv"
)

const (
	defaultDaemonHost = "127.0.0.1"
	defaultDaemonPort = 51842
)

func normalizeDaemon(cfg Config) DaemonConfig {
	d := cfg.Daemon
	if env := os.Getenv("CONDUCTOR_DAEMON_HOST"); env != "" {
		d.Host = env
	}
	if env := os.Getenv("CONDUCTOR_DAEMON_PORT"); env != "" {
		if port, err := strconv.Atoi(env); err == nil && port > 0 {
			d.Port = port
		}
	}
	if d.Host == "" {
		d.Host = defaultDaemonHost
	}
	if d.Port <= 0 {
		d.Port = defaultDaemonPort
	}
	if d.MaxParallel <= 0 {
		d.MaxParallel = normalizeDefaults(cfg.Defaults).MaxParallel
	}
	if d.Queue.OnModeChange == "" {
		d.Queue.OnModeChange = "none"
	}
	return d
}

func daemonBaseURL(cfg DaemonConfig) string {
	return fmt.Sprintf("http://%s:%d", cfg.Host, cfg.Port)
}
