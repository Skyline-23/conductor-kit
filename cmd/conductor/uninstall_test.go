package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestRemovePath(t *testing.T) {
	tmpDir := t.TempDir()

	// Create a file
	testFile := filepath.Join(tmpDir, "test.txt")
	if err := os.WriteFile(testFile, []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}

	// Dry run should not remove
	removePath(testFile, true)
	if !pathExists(testFile) {
		t.Error("removePath dry-run: should not remove file")
	}

	// Real run should remove
	removePath(testFile, false)
	if pathExists(testFile) {
		t.Error("removePath: should remove file")
	}

	// Remove non-existent file should not error
	removePath("/nonexistent/file", false)
}

func TestRemoveBin(t *testing.T) {
	tmpDir := t.TempDir()

	// Create a symlink
	srcFile := filepath.Join(tmpDir, "source")
	if err := os.WriteFile(srcFile, []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}
	linkFile := filepath.Join(tmpDir, "link")
	if err := os.Symlink(srcFile, linkFile); err != nil {
		t.Fatal(err)
	}

	// Dry run should not remove
	removeBin(linkFile, false, true)
	if !pathExists(linkFile) {
		t.Error("removeBin dry-run: should not remove symlink")
	}

	// Real run should remove symlink
	removeBin(linkFile, false, false)
	if pathExists(linkFile) {
		t.Error("removeBin: should remove symlink")
	}

	// Create a regular file (not symlink)
	regularFile := filepath.Join(tmpDir, "regular")
	if err := os.WriteFile(regularFile, []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}

	// Without force, should not remove regular file
	removeBin(regularFile, false, false)
	if !pathExists(regularFile) {
		t.Error("removeBin: should not remove regular file without force")
	}

	// With force, should remove regular file
	removeBin(regularFile, true, false)
	if pathExists(regularFile) {
		t.Error("removeBin with force: should remove regular file")
	}

	// Remove non-existent file should not error
	removeBin("/nonexistent/file", false, false)
}

func TestRemoveIfEmpty(t *testing.T) {
	tmpDir := t.TempDir()

	// Create an empty directory
	emptyDir := filepath.Join(tmpDir, "empty")
	if err := os.MkdirAll(emptyDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Create a non-empty directory
	nonEmptyDir := filepath.Join(tmpDir, "nonempty")
	if err := os.MkdirAll(nonEmptyDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(nonEmptyDir, "file.txt"), []byte("test"), 0644); err != nil {
		t.Fatal(err)
	}

	// Dry run should not remove
	removeIfEmpty(emptyDir, true)
	if !pathExists(emptyDir) {
		t.Error("removeIfEmpty dry-run: should not remove directory")
	}

	// Should remove empty directory
	removeIfEmpty(emptyDir, false)
	if pathExists(emptyDir) {
		t.Error("removeIfEmpty: should remove empty directory")
	}

	// Should not remove non-empty directory
	removeIfEmpty(nonEmptyDir, false)
	if !pathExists(nonEmptyDir) {
		t.Error("removeIfEmpty: should not remove non-empty directory")
	}

	// Non-existent directory should not error
	removeIfEmpty("/nonexistent/dir", false)
}

func TestUninstallHelp(t *testing.T) {
	help := uninstallHelp()
	if help == "" {
		t.Error("uninstallHelp: expected non-empty help text")
	}
}
