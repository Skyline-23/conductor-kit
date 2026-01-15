package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestEnsureDir(t *testing.T) {
	tmpDir := t.TempDir()
	newDir := filepath.Join(tmpDir, "subdir", "nested")

	// Dry run should not create
	err := ensureDir(newDir, true)
	if err != nil {
		t.Errorf("ensureDir dry-run: unexpected error: %v", err)
	}
	if pathExists(newDir) {
		t.Error("ensureDir dry-run: should not create directory")
	}

	// Real run should create
	err = ensureDir(newDir, false)
	if err != nil {
		t.Errorf("ensureDir: unexpected error: %v", err)
	}
	if !pathExists(newDir) {
		t.Error("ensureDir: should create directory")
	}
}

func TestDoLinkOrCopy(t *testing.T) {
	tmpDir := t.TempDir()

	// Create source file
	srcFile := filepath.Join(tmpDir, "source.txt")
	if err := os.WriteFile(srcFile, []byte("test content"), 0644); err != nil {
		t.Fatal(err)
	}

	// Test copy
	destFile := filepath.Join(tmpDir, "dest-copy.txt")
	err := doLinkOrCopy(srcFile, destFile, "copy", false, false)
	if err != nil {
		t.Errorf("doLinkOrCopy copy: unexpected error: %v", err)
	}
	if !pathExists(destFile) {
		t.Error("doLinkOrCopy copy: destination should exist")
	}
	content, _ := os.ReadFile(destFile)
	if string(content) != "test content" {
		t.Errorf("doLinkOrCopy copy: expected 'test content', got %q", string(content))
	}

	// Test link
	destLink := filepath.Join(tmpDir, "dest-link.txt")
	err = doLinkOrCopy(srcFile, destLink, "link", false, false)
	if err != nil {
		t.Errorf("doLinkOrCopy link: unexpected error: %v", err)
	}
	if !pathExists(destLink) {
		t.Error("doLinkOrCopy link: destination should exist")
	}
	info, _ := os.Lstat(destLink)
	if info.Mode()&os.ModeSymlink == 0 {
		t.Error("doLinkOrCopy link: should be a symlink")
	}

	// Test skip existing (without force)
	err = doLinkOrCopy(srcFile, destFile, "copy", false, false)
	if err != nil {
		t.Errorf("doLinkOrCopy skip: unexpected error: %v", err)
	}

	// Test force overwrite
	newSrc := filepath.Join(tmpDir, "new-source.txt")
	if err := os.WriteFile(newSrc, []byte("new content"), 0644); err != nil {
		t.Fatal(err)
	}
	err = doLinkOrCopy(newSrc, destFile, "copy", true, false)
	if err != nil {
		t.Errorf("doLinkOrCopy force: unexpected error: %v", err)
	}
	content, _ = os.ReadFile(destFile)
	if string(content) != "new content" {
		t.Errorf("doLinkOrCopy force: expected 'new content', got %q", string(content))
	}
}

func TestLinkMatches(t *testing.T) {
	tmpDir := t.TempDir()

	// Create source file
	srcFile := filepath.Join(tmpDir, "source.txt")
	if err := os.WriteFile(srcFile, []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}

	// Create symlink
	linkFile := filepath.Join(tmpDir, "link.txt")
	if err := os.Symlink(srcFile, linkFile); err != nil {
		t.Fatal(err)
	}

	// Test matching link
	if !linkMatches(linkFile, srcFile) {
		t.Error("linkMatches: should return true for matching link")
	}

	// Test non-matching link
	otherSrc := filepath.Join(tmpDir, "other.txt")
	if linkMatches(linkFile, otherSrc) {
		t.Error("linkMatches: should return false for non-matching link")
	}

	// Test regular file
	if linkMatches(srcFile, srcFile) {
		t.Error("linkMatches: should return false for regular file")
	}

	// Test non-existent file
	if linkMatches("/nonexistent", srcFile) {
		t.Error("linkMatches: should return false for non-existent file")
	}
}

func TestCopyFile(t *testing.T) {
	tmpDir := t.TempDir()

	srcFile := filepath.Join(tmpDir, "source.txt")
	if err := os.WriteFile(srcFile, []byte("test content"), 0644); err != nil {
		t.Fatal(err)
	}

	destFile := filepath.Join(tmpDir, "dest.txt")
	err := copyFile(srcFile, destFile)
	if err != nil {
		t.Errorf("copyFile: unexpected error: %v", err)
	}

	content, _ := os.ReadFile(destFile)
	if string(content) != "test content" {
		t.Errorf("copyFile: expected 'test content', got %q", string(content))
	}

	// Test copy non-existent file
	err = copyFile("/nonexistent", filepath.Join(tmpDir, "fail.txt"))
	if err == nil {
		t.Error("copyFile: expected error for non-existent source")
	}
}

func TestCopyDir(t *testing.T) {
	tmpDir := t.TempDir()

	// Create source directory with files
	srcDir := filepath.Join(tmpDir, "src")
	if err := os.MkdirAll(filepath.Join(srcDir, "subdir"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "file1.txt"), []byte("file1"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "subdir", "file2.txt"), []byte("file2"), 0644); err != nil {
		t.Fatal(err)
	}

	destDir := filepath.Join(tmpDir, "dest")
	err := copyDir(srcDir, destDir)
	if err != nil {
		t.Errorf("copyDir: unexpected error: %v", err)
	}

	// Check files were copied
	if !pathExists(filepath.Join(destDir, "file1.txt")) {
		t.Error("copyDir: file1.txt should exist")
	}
	if !pathExists(filepath.Join(destDir, "subdir", "file2.txt")) {
		t.Error("copyDir: subdir/file2.txt should exist")
	}

	content, _ := os.ReadFile(filepath.Join(destDir, "subdir", "file2.txt"))
	if string(content) != "file2" {
		t.Errorf("copyDir: expected 'file2', got %q", string(content))
	}
}

func TestDetectRepoRoot(t *testing.T) {
	// Just ensure it doesn't panic
	root := detectRepoRoot()
	_ = root // may be empty if running outside repo
}

func TestInstallHelp(t *testing.T) {
	help := installHelp()
	if help == "" {
		t.Error("installHelp: expected non-empty help text")
	}
	if !strings.Contains(help, "conductor install") {
		t.Error("installHelp: should contain 'conductor install'")
	}
}
