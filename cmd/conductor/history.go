package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

type RunRecord struct {
	ID         string   `json:"id"`
	Agent      string   `json:"agent,omitempty"`
	Role       string   `json:"role,omitempty"`
	Model      string   `json:"model,omitempty"`
	Cmd        string   `json:"cmd"`
	Args       []string `json:"args,omitempty"`
	Status     string   `json:"status"`
	ExitCode   int      `json:"exit_code"`
	StartedAt  string   `json:"started_at"`
	EndedAt    string   `json:"ended_at"`
	DurationMs int64    `json:"duration_ms"`
	PromptHash string   `json:"prompt_hash,omitempty"`
	PromptLen  int      `json:"prompt_len,omitempty"`
	Prompt     string   `json:"prompt,omitempty"`
	Error      string   `json:"error,omitempty"`
}

var runLogMu sync.Mutex

func runLogPath() string {
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	return filepath.Join(baseDir, "runs", "run-history.jsonl")
}

func appendRunRecord(record RunRecord, logPrompt bool) error {
	if !logPrompt {
		record.Prompt = ""
	}
	line, err := json.Marshal(record)
	if err != nil {
		return err
	}

	path := runLogPath()
	runLogMu.Lock()
	defer runLogMu.Unlock()

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = f.Write(append(line, '\n'))
	return err
}

func readRunHistory(limit int, status, role, agent string) ([]RunRecord, error) {
	data, err := os.ReadFile(runLogPath())
	if err != nil {
		if os.IsNotExist(err) {
			return []RunRecord{}, nil
		}
		return nil, err
	}
	lines := strings.Split(string(data), "\n")
	records := []RunRecord{}
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var rec RunRecord
		if err := json.Unmarshal([]byte(line), &rec); err != nil {
			continue
		}
		if status != "" && rec.Status != status {
			continue
		}
		if role != "" && rec.Role != role {
			continue
		}
		if agent != "" && rec.Agent != agent {
			continue
		}
		records = append(records, rec)
	}

	if limit > 0 && len(records) > limit {
		records = records[len(records)-limit:]
	}
	reverseRecords(records)
	return records, nil
}

func findRunRecord(id string) (RunRecord, bool, error) {
	records, err := readRunHistory(0, "", "", "")
	if err != nil {
		return RunRecord{}, false, err
	}
	for _, rec := range records {
		if rec.ID == id {
			return rec, true, nil
		}
	}
	return RunRecord{}, false, nil
}

func reverseRecords(records []RunRecord) {
	for i, j := 0, len(records)-1; i < j; i, j = i+1, j-1 {
		records[i], records[j] = records[j], records[i]
	}
}

func parseRFC3339(value string) time.Time {
	t, _ := time.Parse(time.RFC3339, value)
	return t
}
