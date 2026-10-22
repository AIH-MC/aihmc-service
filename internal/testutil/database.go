package testutil

import (
	"context"
	"database/sql"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/config"
)

type OpenDatabase func(context.Context, *config.Config) (*sql.DB, error)

// Database uses the existing configured server and creates only the fixed test schema.
// The production application never imports this package or executes this DDL.
func Database(t testing.TB, cfg *config.Config, open OpenDatabase) *sql.DB {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	serverConfig := *cfg
	serverConfig.Database.Name = ""
	server, err := open(ctx, &serverConfig)
	if err != nil {
		t.Fatal(err)
	}
	_, err = server.ExecContext(ctx, "CREATE DATABASE IF NOT EXISTS aihmc_migration_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci")
	server.Close()
	if err != nil {
		t.Fatal("cannot create isolated database within the existing server:", err)
	}
	cfg.Database.Name = "aihmc_migration_test"
	db, err := open(ctx, cfg)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	var database string
	if err := db.QueryRowContext(ctx, "SELECT DATABASE()").Scan(&database); err != nil || database != "aihmc_migration_test" {
		t.Fatal("isolated database guard failed")
	}
	schema, err := os.ReadFile(filepath.Join(Root(t), "tests/schema.sql"))
	if err != nil {
		t.Fatal(err)
	}
	for _, statement := range strings.Split(string(schema), ";") {
		if strings.TrimSpace(statement) == "" {
			continue
		}
		statement = strings.Replace(statement, "CREATE TABLE `", "CREATE TABLE IF NOT EXISTS `", 1)
		if _, err := db.ExecContext(ctx, statement); err != nil {
			t.Fatal("cannot prepare isolated schema:", err)
		}
	}
	return db
}
