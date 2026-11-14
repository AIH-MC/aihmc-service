// Package store provides access to the existing MySQL-compatible schema.
package store

import (
	"context"
	"database/sql"
	"errors"
	"net"
	"strconv"

	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/go-sql-driver/mysql"
)

func Open(ctx context.Context, cfg *config.Config) (*sql.DB, error) {
	driver := mysql.NewConfig()
	driver.Net = "tcp"
	driver.Addr = net.JoinHostPort(cfg.Database.Host, strconv.Itoa(cfg.Database.Port))
	driver.User = cfg.Database.User
	driver.Passwd = cfg.DatabasePassword()
	driver.DBName = cfg.Database.Name
	driver.Collation = cfg.Database.Collation
	driver.Params = map[string]string{"charset": cfg.Database.Charset}
	driver.Timeout = cfg.Database.ConnectTimeout.Value()
	driver.ReadTimeout = cfg.Database.QueryTimeout.Value()
	driver.WriteTimeout = cfg.Database.QueryTimeout.Value()
	db, err := sql.Open("mysql", driver.FormatDSN())
	if err != nil {
		return nil, errors.New("invalid database connection configuration")
	}
	db.SetMaxOpenConns(cfg.Database.MaxOpenConnections)
	db.SetMaxIdleConns(cfg.Database.MaxIdleConnections)
	pingContext, cancel := context.WithTimeout(ctx, cfg.Database.ConnectTimeout.Value())
	defer cancel()
	if err := db.PingContext(pingContext); err != nil {
		db.Close()
		return nil, errors.New("cannot connect to configured MySQL or MariaDB instance")
	}
	return db, nil
}
