package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"os"
	"os/exec"
	"os/user"
	"runtime"
	"strings"
)

const (
	codexAuthPath         = "~/.codex/auth.json"
	geminiOAuthCredsPath  = "~/.gemini/oauth_creds.json"
	geminiAccountsPath    = "~/.gemini/google_accounts.json"
	geminiTokenFileV2Path = "~/.gemini/mcp-oauth-tokens-v2.json"
	geminiTokenFilePath   = "~/.gemini/mcp-oauth-tokens.json"
)

func checkAuthForCLI(cli string) (string, string) {
	switch cli {
	case "codex":
		return checkCodexAuth()
	case "gemini":
		return checkGeminiAuth()
	case "claude":
		return checkClaudeAuth()
	default:
		return "unknown", "no auth check for cli: " + cli
	}
}

func checkCodexAuth() (string, string) {
	if pathExists(expandPath(codexAuthPath)) {
		return "ready", ""
	}
	return "not_ready", "missing auth file: " + codexAuthPath
}

func checkGeminiAuth() (string, string) {
	keychainDetail := ""
	if ok, detail := keychainHasEntry("gemini-cli-oauth", "main-account"); ok {
		return "ready", ""
	} else if detail != "" && detail != "not_found" {
		keychainDetail = detail
	}
	if ok, err := jsonFileHasAnyKey(geminiOAuthCredsPath, "access_token", "refresh_token"); ok {
		return "ready", ""
	} else if err != "" {
		return "unknown", err
	}
	if ok, err := geminiAccountsReady(); ok {
		return "ready", ""
	} else if err != "" {
		return "unknown", err
	}
	if pathExists(expandPath(geminiTokenFileV2Path)) {
		return "unknown", "found token store: " + geminiTokenFileV2Path
	}
	if pathExists(expandPath(geminiTokenFilePath)) {
		return "unknown", "found token store: " + geminiTokenFilePath
	}
	if keychainDetail == "keychain_locked" {
		return "unknown", "keychain locked"
	}
	if keychainDetail == "keychain_error" {
		return "unknown", "keychain unavailable"
	}
	return "not_ready", "missing auth files: " + geminiOAuthCredsPath + ", " + geminiAccountsPath
}

func checkClaudeAuth() (string, string) {
	service := claudeKeychainService()
	account := currentUsername()
	if ok, detail := keychainHasEntry(service, account); ok {
		return "ready", ""
	} else if detail == "keychain_locked" {
		return "unknown", "keychain locked"
	} else if detail == "keychain_unavailable" {
		return "unknown", "keychain unavailable"
	} else if detail != "" && detail != "not_found" {
		return "unknown", detail
	}
	return "not_ready", "missing keychain item: " + service
}

func keychainHasEntry(service, account string) (bool, string) {
	if runtime.GOOS != "darwin" {
		return false, "keychain_unavailable"
	}
	if service == "" || account == "" {
		return false, "keychain_unavailable"
	}
	if !isCommandAvailable("security") {
		return false, "keychain_unavailable"
	}
	cmd := exec.Command("security", "find-generic-password", "-a", account, "-s", service)
	cmd.Stdout = io.Discard
	cmd.Stderr = io.Discard
	if err := cmd.Run(); err == nil {
		return true, ""
	} else if exitErr, ok := err.(*exec.ExitError); ok {
		switch exitErr.ExitCode() {
		case 36:
			return false, "keychain_locked"
		case 2, 44:
			return false, "not_found"
		default:
			return false, "keychain_error"
		}
	}
	return false, "keychain_error"
}

func jsonFileHasAnyKey(path string, keys ...string) (bool, string) {
	data, err := os.ReadFile(expandPath(path))
	if err != nil {
		if os.IsNotExist(err) {
			return false, ""
		}
		return false, "failed to read " + path
	}
	if len(strings.TrimSpace(string(data))) == 0 {
		return false, ""
	}
	var obj map[string]interface{}
	if err := json.Unmarshal(data, &obj); err != nil {
		return false, "invalid json: " + path
	}
	for _, key := range keys {
		if val, ok := obj[key]; ok {
			switch v := val.(type) {
			case string:
				if strings.TrimSpace(v) != "" {
					return true, ""
				}
			default:
				return true, ""
			}
		}
	}
	return false, ""
}

func geminiAccountsReady() (bool, string) {
	data, err := os.ReadFile(expandPath(geminiAccountsPath))
	if err != nil {
		if os.IsNotExist(err) {
			return false, ""
		}
		return false, "failed to read " + geminiAccountsPath
	}
	if len(strings.TrimSpace(string(data))) == 0 {
		return false, ""
	}
	var accounts struct {
		Active string   `json:"active"`
		Old    []string `json:"old"`
	}
	if err := json.Unmarshal(data, &accounts); err != nil {
		return false, "invalid json: " + geminiAccountsPath
	}
	if strings.TrimSpace(accounts.Active) != "" || len(accounts.Old) > 0 {
		return true, ""
	}
	return false, ""
}

func claudeKeychainService() string {
	base := "Claude Code"
	suffix := ""
	if configDir := os.Getenv("CLAUDE_CONFIG_DIR"); configDir != "" {
		hash := sha256.Sum256([]byte(configDir))
		suffix = "-" + hex.EncodeToString(hash[:])[:8]
	}
	return base + "-credentials" + suffix
}

func currentUsername() string {
	if val := os.Getenv("USER"); val != "" {
		return val
	}
	if u, err := user.Current(); err == nil && u.Username != "" {
		return u.Username
	}
	return ""
}
