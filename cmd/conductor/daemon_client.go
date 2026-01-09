package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

const daemonHealthTimeout = 300 * time.Millisecond

func resolveDaemonURL(configPath string) string {
	if env := getenv("CONDUCTOR_DAEMON_URL", ""); env != "" {
		return env
	}
	if state, err := readDaemonState(); err == nil {
		url := fmt.Sprintf("http://%s:%d", state.Host, state.Port)
		if daemonHealthy(url) {
			return url
		}
	}
	if configPath != "" {
		if cfg, err := loadConfigOrEmpty(configPath); err == nil {
			dcfg := normalizeDaemon(cfg)
			url := daemonBaseURL(dcfg)
			if daemonHealthy(url) {
				return url
			}
		}
	}
	return ""
}

func daemonHealthy(baseURL string) bool {
	client := &http.Client{Timeout: daemonHealthTimeout}
	resp, err := client.Get(baseURL + "/health")
	if err != nil {
		return false
	}
	_ = resp.Body.Close()
	return resp.StatusCode == http.StatusOK
}

func daemonPostJSON(baseURL, path string, payload any) (map[string]interface{}, error) {
	data, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}
	client := &http.Client{Timeout: 5 * time.Second}
	req, err := http.NewRequest(http.MethodPost, baseURL+path, bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 300 {
		return nil, fmt.Errorf("daemon error: %s", strings.TrimSpace(string(body)))
	}
	var out map[string]interface{}
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	return out, nil
}

func daemonGetJSON(baseURL, path string) (map[string]interface{}, error) {
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Get(baseURL + path)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 300 {
		return nil, fmt.Errorf("daemon error: %s", strings.TrimSpace(string(body)))
	}
	var out map[string]interface{}
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	return out, nil
}

func daemonRun(baseURL string, input RunInput) (map[string]interface{}, error) {
	return daemonPostJSON(baseURL, "/run", input)
}

func daemonRunBatch(baseURL string, input BatchInput) (map[string]interface{}, error) {
	return daemonPostJSON(baseURL, "/run_batch", input)
}

func daemonRunStatus(baseURL, runID string, tail int) (map[string]interface{}, error) {
	path := fmt.Sprintf("/run/%s?tail=%d", runID, tail)
	return daemonGetJSON(baseURL, path)
}

func daemonRunCancel(baseURL, runID string, force bool) (map[string]interface{}, error) {
	return daemonPostJSON(baseURL, "/cancel", map[string]interface{}{"run_id": runID, "force": force})
}

func daemonRunWait(baseURL, runID string, timeout time.Duration, tail int) (map[string]interface{}, error) {
	deadline := time.Now().Add(timeout)
	for {
		res, err := daemonRunStatus(baseURL, runID, tail)
		if err != nil {
			return nil, err
		}
		if res["status"] != "running" && res["status"] != "queued" && res["status"] != "awaiting_approval" {
			return res, nil
		}
		if time.Now().After(deadline) {
			return res, nil
		}
		time.Sleep(500 * time.Millisecond)
	}
}

func daemonQueueList(baseURL, status string, limit int) (map[string]interface{}, error) {
	path := "/runs"
	if status != "" || limit > 0 {
		path = fmt.Sprintf("/runs?status=%s&limit=%d", status, limit)
	}
	return daemonGetJSON(baseURL, path)
}

func daemonApprovalList(baseURL string) (map[string]interface{}, error) {
	return daemonGetJSON(baseURL, "/approvals")
}

func daemonApprovalApprove(baseURL, runID string) (map[string]interface{}, error) {
	return daemonPostJSON(baseURL, "/approve", map[string]interface{}{"run_id": runID})
}

func daemonApprovalReject(baseURL, runID string) (map[string]interface{}, error) {
	return daemonPostJSON(baseURL, "/reject", map[string]interface{}{"run_id": runID})
}

func daemonStatus(baseURL string) (map[string]interface{}, error) {
	return daemonGetJSON(baseURL, "/health")
}
