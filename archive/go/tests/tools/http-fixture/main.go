// Command http-fixture compares the unchanged Go handler using synthetic materials.
// It refuses to use a business database and is never included in a release.
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"os"

	"github.com/AIH-MC/aihmc-service/internal/auth"
	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/httpapi"
	"github.com/AIH-MC/aihmc-service/internal/store"
	"github.com/AIH-MC/aihmc-service/internal/ucenter"
)

type fixtureMaterials struct{}

func (fixtureMaterials) Resolve(context.Context, string) (auth.Material, error) {
	return auth.Material{Skin: "fixture-skin", Model: "slim", Cape: "fixture-cape"}, nil
}

func main() {
	path := flag.String("config", "", "synthetic runtime config")
	flag.Parse()
	if err := run(*path); err != nil {
		fmt.Fprintln(os.Stderr, "HTTP fixture failed")
		os.Exit(1)
	}
}

func run(path string) error {
	cfg, err := config.Load(path)
	if err != nil {
		return err
	}
	if cfg.Database.Name != "aihmc_migration_test" {
		return fmt.Errorf("fixture refuses business database")
	}
	db, err := store.Open(context.Background(), cfg)
	if err != nil {
		return err
	}
	defer db.Close()
	database := store.New(db, cfg.Database.QueryTimeout.Value())
	service := auth.NewService(cfg, database, ucenter.New(cfg), fixtureMaterials{})
	logger := slog.New(slog.NewJSONHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelError}))
	listener, err := net.Listen("tcp", cfg.Server.Listen)
	if err != nil {
		return err
	}
	defer listener.Close()
	fmt.Println("fixture listening", listener.Addr())
	server := &http.Server{
		Handler:           httpapi.New(service, nil, logger),
		ReadHeaderTimeout: cfg.Server.ReadHeaderTimeout.Value(),
		ReadTimeout:       cfg.Server.ReadTimeout.Value(),
		WriteTimeout:      cfg.Server.WriteTimeout.Value(),
	}
	return server.Serve(listener)
}
