package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"time"
)

type DaemonState struct {
	PID       int    `json:"pid"`
	Host      string `json:"host"`
	Port      int    `json:"port"`
	StartedAt string `json:"started_at"`
	Version   string `json:"version"`
}

func daemonStateDir() string {
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	return filepath.Join(baseDir, "daemon")
}

func daemonStatePath() string {
	return filepath.Join(daemonStateDir(), "daemon.json")
}

func daemonLogPath() string {
	return filepath.Join(daemonStateDir(), "daemon.log")
}

func writeDaemonState(state DaemonState) error {
	if err := os.MkdirAll(daemonStateDir(), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(daemonStatePath(), data, 0o644)
}

func readDaemonState() (DaemonState, error) {
	var state DaemonState
	data, err := os.ReadFile(daemonStatePath())
	if err != nil {
		return state, err
	}
	if err := json.Unmarshal(data, &state); err != nil {
		return state, err
	}
	return state, nil
}

func newDaemonState(cfg DaemonConfig) DaemonState {
	return DaemonState{
		PID:       os.Getpid(),
		Host:      cfg.Host,
		Port:      cfg.Port,
		StartedAt: time.Now().UTC().Format(time.RFC3339),
		Version:   "0.1.0",
	}
}
