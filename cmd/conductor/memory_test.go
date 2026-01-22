package main

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func withIsolatedMemoryStore(t *testing.T, fn func()) {
	t.Helper()
	originalStore := sharedMemory
	originalCache := sharedMemoryCache
	sharedMemory = newMemoryStore()
	sharedMemoryCache = &memoryCache{enabled: false}
	t.Cleanup(func() {
		sharedMemory = originalStore
		sharedMemoryCache = originalCache
	})
	fn()
}

func TestApplyMemoryToPromptModes(t *testing.T) {
	withIsolatedMemoryStore(t, func() {
		if _, err := sharedMemory.set("alpha", "first"); err != nil {
			t.Fatalf("set memory: %v", err)
		}

		prompt := "hello"
		got, err := applyMemoryToPrompt(prompt, "alpha", "")
		if err != nil {
			t.Fatalf("applyMemoryToPrompt default: %v", err)
		}
		want := memoryBlock("alpha", "first") + "\n\n" + prompt
		if got != want {
			t.Errorf("default mode: expected %q, got %q", want, got)
		}

		got, err = applyMemoryToPrompt(prompt, "alpha", "append")
		if err != nil {
			t.Fatalf("applyMemoryToPrompt append: %v", err)
		}
		want = prompt + "\n\n" + memoryBlock("alpha", "first")
		if got != want {
			t.Errorf("append mode: expected %q, got %q", want, got)
		}

		if _, err := applyMemoryToPrompt(prompt, "alpha", "unknown"); err == nil {
			t.Error("expected error for unknown memory mode")
		}

		if _, err := applyMemoryToPrompt(prompt, "missing", ""); err == nil {
			t.Error("expected error for missing memory key")
		}
	})
}

func TestMcpHandleMemoryLifecycle(t *testing.T) {
	withIsolatedMemoryStore(t, func() {
		payload, err := mcpHandleMemory(MCPMemoryInput{
			Action: "set",
			Key:    "alpha",
			Value:  "one",
		})
		if err != nil {
			t.Fatalf("set memory: %v", err)
		}
		if payload["status"] != "set" {
			t.Errorf("expected status set, got %v", payload["status"])
		}

		_, err = mcpHandleMemory(MCPMemoryInput{
			Action: "append",
			Key:    "alpha",
			Value:  "two",
		})
		if err != nil {
			t.Fatalf("append memory: %v", err)
		}

		payload, err = mcpHandleMemory(MCPMemoryInput{
			Action: "get",
			Key:    "alpha",
		})
		if err != nil {
			t.Fatalf("get memory: %v", err)
		}
		value, ok := payload["value"].(string)
		if !ok {
			t.Fatalf("expected value string, got %T", payload["value"])
		}
		wantValue := "one" + memoryDefaultSeparator + "two"
		if value != wantValue {
			t.Errorf("expected value %q, got %q", wantValue, value)
		}

		payload, err = mcpHandleMemory(MCPMemoryInput{Action: "list"})
		if err != nil {
			t.Fatalf("list memory: %v", err)
		}
		count, ok := payload["count"].(int)
		if !ok {
			t.Fatalf("expected count int, got %T", payload["count"])
		}
		if count != 1 {
			t.Errorf("expected count 1, got %d", count)
		}
		items, ok := payload["items"].([]map[string]interface{})
		if !ok || len(items) != 1 {
			t.Fatalf("expected single item list, got %v", payload["items"])
		}
		if items[0]["key"] != "alpha" {
			t.Errorf("expected key alpha, got %v", items[0]["key"])
		}

		payload, err = mcpHandleMemory(MCPMemoryInput{
			Action: "clear",
			Key:    "alpha",
		})
		if err != nil {
			t.Fatalf("clear memory: %v", err)
		}
		removed, ok := payload["removed"].(bool)
		if !ok || !removed {
			t.Errorf("expected removed true, got %v", payload["removed"])
		}

		_, err = mcpHandleMemory(MCPMemoryInput{
			Action: "set",
			Key:    "beta",
			Value:  "value",
		})
		if err != nil {
			t.Fatalf("set memory for clear_all: %v", err)
		}

		payload, err = mcpHandleMemory(MCPMemoryInput{Action: "clear_all"})
		if err != nil {
			t.Fatalf("clear_all memory: %v", err)
		}
		cleared, ok := payload["count"].(int)
		if !ok || cleared != 1 {
			t.Errorf("expected count 1, got %v", payload["count"])
		}

		if _, err := mcpHandleMemory(MCPMemoryInput{Action: "unknown"}); err == nil {
			t.Error("expected error for unknown action")
		}
	})
}

func TestMemoryCachePersistAndLoad(t *testing.T) {
	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "memory.gob.gz")
	cache := &memoryCache{
		enabled:     true,
		path:        path,
		projectRoot: "root",
		gitHead:     "head",
	}
	entries := map[string]memoryEntry{
		"alpha": {Value: "value", UpdatedAt: time.Now().UTC()},
	}
	cache.persist(entries, true)
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected cache file, got error: %v", err)
	}

	store := newMemoryStore()
	cache.load(store)
	entry, ok := store.get("alpha")
	if !ok {
		t.Fatal("expected cached entry")
	}
	if entry.Value != "value" {
		t.Errorf("expected value 'value', got %q", entry.Value)
	}
}

func TestMemoryCacheLoadInvalidatesExpired(t *testing.T) {
	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "memory.gob.gz")
	old := time.Now().Add(-memoryCacheTTL - time.Minute)
	snapshot := memorySnapshot{
		Version:     memoryCacheVersion,
		ProjectRoot: "root",
		GitHead:     "head",
		UpdatedAt:   old,
		Entries: map[string]memoryEntry{
			"alpha": {Value: "value", UpdatedAt: old},
		},
	}
	if err := writeMemorySnapshot(path, snapshot); err != nil {
		t.Fatalf("write snapshot: %v", err)
	}

	cache := &memoryCache{
		enabled:     true,
		path:        path,
		projectRoot: "root",
		gitHead:     "head",
	}
	store := newMemoryStore()
	cache.load(store)

	if _, ok := store.get("alpha"); ok {
		t.Error("expected expired cache to be ignored")
	}
	if _, err := os.Stat(path); err == nil {
		t.Error("expected expired cache file to be removed")
	} else if !os.IsNotExist(err) {
		t.Errorf("unexpected stat error: %v", err)
	}
}
