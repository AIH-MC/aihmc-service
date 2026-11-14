package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"

	"github.com/AIH-MC/aihmc-service/internal/auth"
	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/httpapi"
	"github.com/AIH-MC/aihmc-service/internal/store"
	"github.com/AIH-MC/aihmc-service/internal/texture"
	"github.com/AIH-MC/aihmc-service/internal/ucenter"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "startup failed:", err)
		os.Exit(1)
	}
}

func run() error {
	path := flag.String("config", "config.toml", "path to runtime TOML configuration")
	check := flag.Bool("check-config", false, "validate configuration, files and database connectivity, then exit")
	flag.Parse()
	cfg, err := config.Load(*path)
	if err != nil {
		return err
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	db, err := store.Open(ctx, cfg)
	if err != nil {
		return err
	}
	defer db.Close()
	if *check {
		fmt.Println("configuration, key pair, texture directory and database connection validated")
		return nil
	}
	var level slog.Level
	if err := level.UnmarshalText([]byte(cfg.Logging.Level)); err != nil {
		return errors.New("invalid logging.level")
	}
	options := &slog.HandlerOptions{Level: level}
	var handler slog.Handler = slog.NewJSONHandler(os.Stdout, options)
	if cfg.Logging.Format == "text" {
		handler = slog.NewTextHandler(os.Stdout, options)
	}
	logger := slog.New(handler)
	databaseStore := store.New(db, cfg.Database.QueryTimeout.Value())
	materials := texture.NewService(cfg, databaseStore)
	service := auth.NewService(cfg, databaseStore, ucenter.New(cfg), materials)
	root := httpapi.New(service, materials, logger)
	server := &http.Server{
		Addr:              cfg.Server.Listen,
		Handler:           root,
		ReadHeaderTimeout: cfg.Server.ReadHeaderTimeout.Value(),
		ReadTimeout:       cfg.Server.ReadTimeout.Value(),
		WriteTimeout:      cfg.Server.WriteTimeout.Value(),
	}
	listener, err := net.Listen("tcp", server.Addr)
	if err != nil {
		return errors.New("cannot listen on server.listen")
	}
	finished := make(chan error, 1)
	go func() { finished <- server.Serve(listener) }()
	logger.Info("server started", "address", listener.Addr().String())
	select {
	case err := <-finished:
		if !errors.Is(err, http.ErrServerClosed) {
			return err
		}
		return nil
	case <-ctx.Done():
		shutdownContext, cancel := context.WithTimeout(context.Background(), cfg.Server.ShutdownTimeout.Value())
		defer cancel()
		if err := server.Shutdown(shutdownContext); err != nil {
			server.Close()
			return errors.New("graceful shutdown timed out")
		}
		logger.Info("server stopped")
		return nil
	}
}
