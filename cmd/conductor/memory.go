package main

import (
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"
	"unicode/utf8"
)

const (
	memoryDefaultSeparator = "\n\n"
	memoryMaxBytes         = 200000
	memoryGlobalKey        = "shared"
	memoryRolePrefix       = "role:"
)

type memoryEntry struct {
	Value     string
	UpdatedAt time.Time
}

type memoryItem struct {
	Key       string
	Size      int
	UpdatedAt time.Time
}

type memoryStore struct {
	mu    sync.RWMutex
	items map[string]memoryEntry
}

var sharedMemory = newMemoryStore()

func newMemoryStore() *memoryStore {
	return &memoryStore{items: make(map[string]memoryEntry)}
}

func normalizeMemoryKey(key string) (string, error) {
	key = strings.TrimSpace(key)
	if key == "" {
		return "", fmt.Errorf("memory key is required")
	}
	return key, nil
}

func normalizeMemoryLabel(label string) string {
	label = strings.TrimSpace(label)
	if label == "" {
		return "unknown"
	}
	return label
}

func memoryLabel(agent, role string) string {
	if strings.TrimSpace(role) != "" {
		return normalizeMemoryLabel(role)
	}
	if strings.TrimSpace(agent) != "" {
		return normalizeMemoryLabel(agent)
	}
	return "unknown"
}

func memoryRoleKey(label string) string {
	return memoryRolePrefix + normalizeMemoryLabel(label)
}

func memoryRoleBlock(label, value string) string {
	label = normalizeMemoryLabel(label)
	return fmt.Sprintf("[role:%s]\n%s\n[/role]", label, value)
}

func trimMemoryValue(value string, maxBytes int) (string, bool) {
	if len(value) <= maxBytes {
		return value, false
	}
	trimmed := value[len(value)-maxBytes:]
	if utf8.ValidString(trimmed) {
		return trimmed, true
	}
	for i := 1; i < len(trimmed) && i < 4; i++ {
		if utf8.ValidString(trimmed[i:]) {
			return trimmed[i:], true
		}
	}
	return strings.ToValidUTF8(trimmed, ""), true
}

func (m *memoryStore) get(key string) (memoryEntry, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	entry, ok := m.items[key]
	return entry, ok
}

func (m *memoryStore) set(key, value string) (memoryEntry, error) {
	return m.setInternal(key, value, false)
}

func (m *memoryStore) setAuto(key, value string) memoryEntry {
	entry, _ := m.setInternal(key, value, true)
	return entry
}

func (m *memoryStore) setInternal(key, value string, truncate bool) (memoryEntry, error) {
	trimmed, trimmedDown := trimMemoryValue(value, memoryMaxBytes)
	if trimmedDown && !truncate {
		return memoryEntry{}, fmt.Errorf("memory value exceeds %d bytes", memoryMaxBytes)
	}
	entry := memoryEntry{Value: trimmed, UpdatedAt: time.Now().UTC()}
	m.mu.Lock()
	m.items[key] = entry
	snapshot := cloneMemoryEntries(m.items)
	m.mu.Unlock()
	sharedMemoryCache.persist(snapshot, false)
	return entry, nil
}

func (m *memoryStore) append(key, value, separator string) (memoryEntry, error) {
	return m.appendInternal(key, value, separator, false)
}

func (m *memoryStore) appendAuto(key, value, separator string) memoryEntry {
	entry, _ := m.appendInternal(key, value, separator, true)
	return entry
}

func (m *memoryStore) appendInternal(key, value, separator string, truncate bool) (memoryEntry, error) {
	if separator == "" {
		separator = memoryDefaultSeparator
	}
	entry, _ := m.get(key)
	newValue := value
	if entry.Value != "" {
		newValue = entry.Value + separator + value
	}
	return m.setInternal(key, newValue, truncate)
}

func (m *memoryStore) clear(key string) bool {
	m.mu.Lock()
	if _, ok := m.items[key]; ok {
		delete(m.items, key)
		snapshot := cloneMemoryEntries(m.items)
		m.mu.Unlock()
		sharedMemoryCache.persist(snapshot, false)
		return true
	}
	m.mu.Unlock()
	return false
}

func (m *memoryStore) clearAll() int {
	m.mu.Lock()
	count := len(m.items)
	m.items = make(map[string]memoryEntry)
	snapshot := cloneMemoryEntries(m.items)
	m.mu.Unlock()
	sharedMemoryCache.persist(snapshot, true)
	return count
}

func (m *memoryStore) list() []memoryItem {
	m.mu.RLock()
	defer m.mu.RUnlock()
	items := make([]memoryItem, 0, len(m.items))
	for key, entry := range m.items {
		items = append(items, memoryItem{Key: key, Size: len(entry.Value), UpdatedAt: entry.UpdatedAt})
	}
	sort.Slice(items, func(i, j int) bool {
		return items[i].Key < items[j].Key
	})
	return items
}

func cloneMemoryEntries(entries map[string]memoryEntry) map[string]memoryEntry {
	clone := make(map[string]memoryEntry, len(entries))
	for key, entry := range entries {
		clone[key] = entry
	}
	return clone
}

func memoryBlock(key, value string) string {
	return fmt.Sprintf("[shared-memory:%s]\n%s\n[/shared-memory]", key, value)
}

func applySharedMemory(prompt string) string {
	entry, ok := sharedMemory.get(memoryGlobalKey)
	if !ok {
		return prompt
	}
	value := strings.TrimSpace(entry.Value)
	if value == "" {
		return prompt
	}
	return memoryBlock(memoryGlobalKey, value) + "\n\n" + prompt
}

func rememberSharedMemory(agent, role, output string) {
	output = strings.TrimSpace(output)
	if output == "" {
		return
	}
	label := memoryLabel(agent, role)
	sharedMemory.appendAuto(memoryRoleKey(label), output, memoryDefaultSeparator)
	sharedMemory.appendAuto(memoryGlobalKey, memoryRoleBlock(label, output), memoryDefaultSeparator)
}

func applyMemoryToPrompt(prompt, key, mode string) (string, error) {
	if key == "" {
		return prompt, nil
	}
	normalized, err := normalizeMemoryKey(key)
	if err != nil {
		return "", err
	}
	entry, ok := sharedMemory.get(normalized)
	if !ok {
		return "", fmt.Errorf("memory not found: %s", normalized)
	}
	block := memoryBlock(normalized, entry.Value)
	mode = strings.ToLower(strings.TrimSpace(mode))
	switch mode {
	case "", "prepend", "prefix":
		return block + "\n\n" + prompt, nil
	case "append", "suffix":
		return prompt + "\n\n" + block, nil
	default:
		return "", fmt.Errorf("unknown memory mode: %s", mode)
	}
}

func mcpHandleMemory(input MCPMemoryInput) (map[string]interface{}, error) {
	action := strings.ToLower(strings.TrimSpace(input.Action))
	switch action {
	case "set":
		key, err := normalizeMemoryKey(input.Key)
		if err != nil {
			return nil, err
		}
		entry, err := sharedMemory.set(key, input.Value)
		if err != nil {
			return nil, err
		}
		return memoryEntryPayload("set", key, entry, false), nil
	case "append":
		key, err := normalizeMemoryKey(input.Key)
		if err != nil {
			return nil, err
		}
		entry, err := sharedMemory.append(key, input.Value, input.Separator)
		if err != nil {
			return nil, err
		}
		return memoryEntryPayload("append", key, entry, false), nil
	case "get":
		key, err := normalizeMemoryKey(input.Key)
		if err != nil {
			return nil, err
		}
		entry, ok := sharedMemory.get(key)
		if !ok {
			return nil, fmt.Errorf("memory not found: %s", key)
		}
		return memoryEntryPayload("get", key, entry, true), nil
	case "list":
		items := sharedMemory.list()
		return map[string]interface{}{
			"status":    "ok",
			"count":     len(items),
			"items":     memoryListPayload(items),
			"max_bytes": memoryMaxBytes,
		}, nil
	case "clear":
		key, err := normalizeMemoryKey(input.Key)
		if err != nil {
			return nil, err
		}
		removed := sharedMemory.clear(key)
		return map[string]interface{}{
			"status":  "cleared",
			"key":     key,
			"removed": removed,
		}, nil
	case "clear_all":
		count := sharedMemory.clearAll()
		return map[string]interface{}{
			"status": "cleared_all",
			"count":  count,
		}, nil
	case "":
		return nil, fmt.Errorf("memory action is required")
	default:
		return nil, fmt.Errorf("unknown memory action: %s", input.Action)
	}
}

func memoryEntryPayload(status, key string, entry memoryEntry, includeValue bool) map[string]interface{} {
	payload := map[string]interface{}{
		"status":     status,
		"key":        key,
		"size":       len(entry.Value),
		"updated_at": entry.UpdatedAt.Format(time.RFC3339),
	}
	if includeValue {
		payload["value"] = entry.Value
	}
	return payload
}

func memoryListPayload(items []memoryItem) []map[string]interface{} {
	result := make([]map[string]interface{}, 0, len(items))
	for _, item := range items {
		result = append(result, map[string]interface{}{
			"key":        item.Key,
			"size":       item.Size,
			"updated_at": item.UpdatedAt.Format(time.RFC3339),
		})
	}
	return result
}
