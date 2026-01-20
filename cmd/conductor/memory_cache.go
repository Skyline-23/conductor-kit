package main

import (
	"compress/gzip"
	"crypto/sha256"
	"encoding/gob"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

const (
	memoryCacheVersion         = 1
	memoryCacheTTL             = 24 * time.Hour
	memoryCachePersistInterval = 2 * time.Second
)

type memorySnapshot struct {
	Version     int
	ProjectRoot string
	GitHead     string
	UpdatedAt   time.Time
	Entries     map[string]memoryEntry
}

type memoryCache struct {
	mu          sync.Mutex
	enabled     bool
	path        string
	projectRoot string
	gitHead     string
	lastPersist time.Time
}

var sharedMemoryCache = newMemoryCache()

func init() {
	if sharedMemoryCache != nil {
		sharedMemoryCache.load(sharedMemory)
	}
}

func newMemoryCache() *memoryCache {
	root := resolveProjectRoot()
	if root == "" {
		return &memoryCache{enabled: false}
	}
	return &memoryCache{
		enabled:     true,
		path:        resolveMemoryCachePath(root),
		projectRoot: root,
		gitHead:     resolveGitHead(root),
	}
}

func (c *memoryCache) load(store *memoryStore) {
	if c == nil || !c.enabled || c.path == "" {
		return
	}

	snapshot, err := loadMemorySnapshot(c.path)
	if err != nil {
		return
	}
	if snapshot.Version != memoryCacheVersion {
		_ = os.Remove(c.path)
		return
	}
	if snapshot.ProjectRoot != c.projectRoot {
		_ = os.Remove(c.path)
		return
	}
	if c.gitHead != "" && snapshot.GitHead != "" && snapshot.GitHead != c.gitHead {
		_ = os.Remove(c.path)
		return
	}
	if time.Since(snapshot.UpdatedAt) > memoryCacheTTL {
		_ = os.Remove(c.path)
		return
	}

	trimmed := make(map[string]memoryEntry, len(snapshot.Entries))
	for key, entry := range snapshot.Entries {
		value, _ := trimMemoryValue(entry.Value, memoryMaxBytes)
		entry.Value = value
		trimmed[key] = entry
	}
	store.mu.Lock()
	store.items = trimmed
	store.mu.Unlock()
}

func (c *memoryCache) persist(entries map[string]memoryEntry, force bool) {
	if c == nil || !c.enabled || c.path == "" {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	if len(entries) == 0 {
		_ = os.Remove(c.path)
		c.lastPersist = time.Now().UTC()
		return
	}
	if !force && time.Since(c.lastPersist) < memoryCachePersistInterval {
		return
	}
	if err := os.MkdirAll(filepath.Dir(c.path), 0o755); err != nil {
		return
	}
	updatedAt := time.Now().UTC()
	snapshot := memorySnapshot{
		Version:     memoryCacheVersion,
		ProjectRoot: c.projectRoot,
		GitHead:     c.gitHead,
		UpdatedAt:   updatedAt,
		Entries:     entries,
	}
	if err := writeMemorySnapshot(c.path, snapshot); err != nil {
		return
	}
	c.lastPersist = updatedAt
}

func loadMemorySnapshot(path string) (memorySnapshot, error) {
	snapshot := memorySnapshot{}
	file, err := os.Open(path)
	if err != nil {
		return snapshot, err
	}
	defer file.Close()
	gz, err := gzip.NewReader(file)
	if err != nil {
		return snapshot, err
	}
	defer gz.Close()
	decoder := gob.NewDecoder(gz)
	if err := decoder.Decode(&snapshot); err != nil {
		return snapshot, err
	}
	return snapshot, nil
}

func writeMemorySnapshot(path string, snapshot memorySnapshot) error {
	tmpPath := path + ".tmp"
	file, err := os.Create(tmpPath)
	if err != nil {
		return err
	}
	gz := gzip.NewWriter(file)
	encoder := gob.NewEncoder(gz)
	if err := encoder.Encode(snapshot); err != nil {
		_ = gz.Close()
		_ = file.Close()
		_ = os.Remove(tmpPath)
		return err
	}
	if err := gz.Close(); err != nil {
		_ = file.Close()
		_ = os.Remove(tmpPath)
		return err
	}
	if err := file.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	return os.Rename(tmpPath, path)
}

func resolveProjectRoot() string {
	cwd, err := os.Getwd()
	if err != nil {
		return ""
	}
	dir := cwd
	for {
		if pathExists(filepath.Join(dir, ".git")) || pathExists(filepath.Join(dir, ".conductor-kit")) {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return cwd
		}
		dir = parent
	}
}

func resolveMemoryCachePath(root string) string {
	baseDir := getenv("CONDUCTOR_HOME", filepath.Join(os.Getenv("HOME"), ".conductor-kit"))
	sum := sha256.Sum256([]byte(root))
	filename := hex.EncodeToString(sum[:]) + ".gob.gz"
	return filepath.Join(baseDir, "memory", filename)
}

func resolveGitHead(root string) string {
	gitDir := resolveGitDir(root)
	if gitDir == "" {
		return ""
	}
	headPath := filepath.Join(gitDir, "HEAD")
	data, err := os.ReadFile(headPath)
	if err != nil {
		return ""
	}
	head := strings.TrimSpace(string(data))
	if strings.HasPrefix(head, "ref:") {
		ref := strings.TrimSpace(strings.TrimPrefix(head, "ref:"))
		if ref == "" {
			return ""
		}
		return resolveGitRef(gitDir, ref)
	}
	return head
}

func resolveGitDir(root string) string {
	gitPath := filepath.Join(root, ".git")
	info, err := os.Stat(gitPath)
	if err != nil {
		return ""
	}
	if info.IsDir() {
		return gitPath
	}
	data, err := os.ReadFile(gitPath)
	if err != nil {
		return ""
	}
	line := strings.TrimSpace(string(data))
	if !strings.HasPrefix(line, "gitdir:") {
		return ""
	}
	gitDir := strings.TrimSpace(strings.TrimPrefix(line, "gitdir:"))
	if gitDir == "" {
		return ""
	}
	if !filepath.IsAbs(gitDir) {
		gitDir = filepath.Join(root, gitDir)
	}
	return gitDir
}

func resolveGitRef(gitDir, ref string) string {
	refPath := filepath.Join(gitDir, filepath.FromSlash(ref))
	if data, err := os.ReadFile(refPath); err == nil {
		return strings.TrimSpace(string(data))
	}
	packedRefs := filepath.Join(gitDir, "packed-refs")
	data, err := os.ReadFile(packedRefs)
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "^") {
			continue
		}
		parts := strings.Fields(line)
		if len(parts) < 2 {
			continue
		}
		if parts[1] == ref {
			return parts[0]
		}
	}
	return ""
}
