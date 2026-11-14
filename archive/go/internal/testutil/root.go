// Package testutil locates migration fixtures before and after the module moves to the repository root.
package testutil

import (
	"os"
	"path/filepath"
	"testing"
)

func Root(t testing.TB) string {
	t.Helper()
	directory, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		if _, err := os.Stat(filepath.Join(directory, "GO_MIGRATION_PLAN.md")); err == nil {
			return directory
		}
		parent := filepath.Dir(directory)
		if parent == directory {
			t.Fatal("migration repository root not found")
		}
		directory = parent
	}
}
