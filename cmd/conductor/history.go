package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"
)

type RunRecord struct {
	ID           string   `json:"id"`
	Agent        string   `json:"agent,omitempty"`
	Role         string   `json:"role,omitempty"`
	Model        string   `json:"model,omitempty"`
	Cmd          string   `json:"cmd"`
	Args         []string `json:"args,omitempty"`
	Status       string   `json:"status"`
	ExitCode     int      `json:"exit_code"`
	StartedAt    string   `json:"started_at"`
	EndedAt      string   `json:"ended_at"`
	DurationMs   int64    `json:"duration_ms"`
	PromptHash   string   `json:"prompt_hash,omitempty"`
	PromptLen    int      `json:"prompt_len,omitempty"`
	Prompt       string   `json:"prompt,omitempty"`
	ReadFiles    []string `json:"read_files,omitempty"`
	ChangedFiles []string `json:"changed_files,omitempty"`
	Error        string   `json:"error,omitempty"`
}

var runLogMu sync.Mutex

const runHistoryDefaultMaxBytes = 10 * 1024 * 1024

func runHistoryMaxBytes() int64 {
	if val := strings.TrimSpace(os.Getenv("CONDUCTOR_RUN_HISTORY_MAX_BYTES")); val != "" {
		if parsed, err := strconv.ParseInt(val, 10, 64); err == nil {
			return parsed
		}
	}
	return runHistoryDefaultMaxBytes
}

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
	if err != nil {
		return err
	}
	if maxBytes := runHistoryMaxBytes(); maxBytes > 0 {
		if err := trimRunHistory(path, maxBytes); err != nil {
			return err
		}
	}
	return nil
}

func trimRunHistory(path string, maxBytes int64) error {
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if info.Size() <= maxBytes {
		return nil
	}
	start := info.Size() - maxBytes
	if start < 0 {
		start = 0
	}
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	if _, err := file.Seek(start, 0); err != nil {
		return err
	}
	data, err := io.ReadAll(file)
	if err != nil {
		return err
	}
	if start > 0 {
		if idx := bytes.IndexByte(data, '\n'); idx >= 0 {
			data = data[idx+1:]
		} else {
			data = nil
		}
	}
	return os.WriteFile(path, data, 0o644)
}

func readRunHistory(limit int, status, role, agent string) ([]RunRecord, error) {
	path := runLogPath()
	file, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return []RunRecord{}, nil
		}
		return nil, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 0, 64*1024), 10*1024*1024)
	records := []RunRecord{}
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
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
		if limit > 0 && len(records) > limit {
			records = records[len(records)-limit:]
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, err
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
