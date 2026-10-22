// Package config loads the service's private TOML configuration.
package config

import (
	"bytes"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"errors"
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/pelletier/go-toml/v2"
)

type Duration time.Duration

func (d *Duration) UnmarshalText(text []byte) error {
	value, err := time.ParseDuration(string(text))
	if err != nil || value <= 0 {
		return errors.New("expected a positive duration")
	}
	*d = Duration(value)
	return nil
}

func (d Duration) Value() time.Duration { return time.Duration(d) }

type Config struct {
	Server   Server   `toml:"server"`
	Site     Site     `toml:"site"`
	Database Database `toml:"database"`
	UCenter  UCenter  `toml:"ucenter"`
	Auth     Auth     `toml:"auth"`
	Textures Textures `toml:"textures"`
	Admin    Admin    `toml:"admin"`
	Logging  Logging  `toml:"logging"`

	password string
	secret   string
	superKey string
	key      *rsa.PrivateKey
	public   string
}

type Server struct {
	Listen            string   `toml:"listen"`
	PublicBaseURL     string   `toml:"public_base_url"`
	ReadHeaderTimeout Duration `toml:"read_header_timeout"`
	ReadTimeout       Duration `toml:"read_timeout"`
	WriteTimeout      Duration `toml:"write_timeout"`
	ShutdownTimeout   Duration `toml:"shutdown_timeout"`
}

type Site struct {
	Name                  string `toml:"name"`
	ImplementationName    string `toml:"implementation_name"`
	ImplementationVersion string `toml:"implementation_version"`
	Homepage              string `toml:"homepage"`
	RegisterURL           string `toml:"register_url"`
	AllowNonEmailLogin    bool   `toml:"allow_non_email_login"`
}

type Database struct {
	Host               string   `toml:"host"`
	Port               int      `toml:"port"`
	Name               string   `toml:"name"`
	User               string   `toml:"user"`
	PasswordFile       string   `toml:"password_file"`
	Charset            string   `toml:"charset"`
	Collation          string   `toml:"collation"`
	MaxOpenConnections int      `toml:"max_open_connections"`
	MaxIdleConnections int      `toml:"max_idle_connections"`
	ConnectTimeout     Duration `toml:"connect_timeout"`
	QueryTimeout       Duration `toml:"query_timeout"`
}

type UCenter struct {
	Endpoint       string   `toml:"endpoint"`
	AppID          string   `toml:"app_id"`
	SecretFile     string   `toml:"secret_file"`
	Nonce          string   `toml:"nonce"`
	RequestTimeout Duration `toml:"request_timeout"`
}

type Auth struct {
	PrivateKeyFile       string   `toml:"private_key_file"`
	PublicKeyFile        string   `toml:"public_key_file"`
	LoginRecordRetention Duration `toml:"login_record_retention"`
}

type Textures struct {
	Directory        string   `toml:"directory"`
	PublicBaseURL    string   `toml:"public_base_url"`
	RequestTimeout   Duration `toml:"request_timeout"`
	MojangBaseURL    string   `toml:"mojang_base_url"`
	CrafatarBaseURL  string   `toml:"crafatar_base_url"`
	CraftheadBaseURL string   `toml:"crafthead_base_url"`
}

type Admin struct {
	SuperKeyFile string `toml:"super_key_file"`
}

type Logging struct {
	Level  string `toml:"level"`
	Format string `toml:"format"`
}

func Load(path string) (*Config, error) {
	if strings.EqualFold(filepath.Base(path), "config.example.toml") {
		return nil, errors.New("config.example.toml is a template; provide config.toml")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, errors.New("cannot read runtime configuration file")
	}
	var cfg Config
	decoder := toml.NewDecoder(bytes.NewReader(data)).DisallowUnknownFields()
	if err := decoder.Decode(&cfg); err != nil {
		// Decoder context may contain secret values. Never return the source excerpt.
		var unknown *toml.StrictMissingError
		if errors.As(err, &unknown) && len(unknown.Errors) > 0 {
			return nil, fmt.Errorf("unknown configuration field %s", strings.Join(unknown.Errors[0].Key(), "."))
		}
		var invalid *toml.DecodeError
		if errors.As(err, &invalid) {
			row, column := invalid.Position()
			return nil, fmt.Errorf("invalid configuration field %s at line %d column %d: check TOML syntax and value type", strings.Join(invalid.Key(), "."), row, column)
		}
		return nil, errors.New("invalid TOML: check syntax, field names and value types")
	}
	if err := cfg.validate(); err != nil {
		return nil, err
	}
	absolute, err := filepath.Abs(path)
	if err != nil {
		return nil, errors.New("cannot resolve configuration directory")
	}
	base := filepath.Dir(absolute)
	for _, value := range []*string{
		&cfg.Database.PasswordFile, &cfg.UCenter.SecretFile, &cfg.Admin.SuperKeyFile,
		&cfg.Auth.PrivateKeyFile, &cfg.Auth.PublicKeyFile, &cfg.Textures.Directory,
	} {
		if !filepath.IsAbs(*value) {
			*value = filepath.Join(base, *value)
		}
	}
	if cfg.password, err = readSecret(cfg.Database.PasswordFile); err != nil {
		return nil, errors.New("database.password_file must reference a readable nonempty secret")
	}
	if cfg.secret, err = readSecret(cfg.UCenter.SecretFile); err != nil {
		return nil, errors.New("ucenter.secret_file must reference a readable nonempty secret")
	}
	if cfg.superKey, err = readSecret(cfg.Admin.SuperKeyFile); err != nil {
		return nil, errors.New("admin.super_key_file must reference a readable nonempty secret")
	}
	if err := cfg.loadKeys(); err != nil {
		return nil, err
	}
	directory, err := os.Stat(cfg.Textures.Directory)
	if err != nil || !directory.IsDir() {
		return nil, errors.New("textures.directory must reference an existing directory")
	}
	probe, err := os.CreateTemp(cfg.Textures.Directory, ".write-check-*")
	if err != nil {
		return nil, errors.New("textures.directory must be writable")
	}
	name := probe.Name()
	closeErr := probe.Close()
	removeErr := os.Remove(name)
	if closeErr != nil || removeErr != nil {
		return nil, errors.New("cannot complete textures.directory access check")
	}
	return &cfg, nil
}

func (c *Config) validate() error {
	required := []struct{ name, value string }{
		{"site.name", c.Site.Name},
		{"site.implementation_name", c.Site.ImplementationName},
		{"site.implementation_version", c.Site.ImplementationVersion},
		{"database.host", c.Database.Host}, {"database.name", c.Database.Name},
		{"database.user", c.Database.User}, {"database.password_file", c.Database.PasswordFile},
		{"database.charset", c.Database.Charset}, {"database.collation", c.Database.Collation},
		{"ucenter.app_id", c.UCenter.AppID}, {"ucenter.nonce", c.UCenter.Nonce},
		{"ucenter.secret_file", c.UCenter.SecretFile},
		{"auth.private_key_file", c.Auth.PrivateKeyFile}, {"auth.public_key_file", c.Auth.PublicKeyFile},
		{"textures.directory", c.Textures.Directory}, {"admin.super_key_file", c.Admin.SuperKeyFile},
	}
	for _, field := range required {
		if field.value == "" {
			return fmt.Errorf("%s is required", field.name)
		}
	}
	if _, _, err := net.SplitHostPort(c.Server.Listen); err != nil {
		return errors.New("server.listen must be a host:port address")
	}
	urls := []struct{ name, value string }{
		{"server.public_base_url", c.Server.PublicBaseURL},
		{"site.homepage", c.Site.Homepage}, {"site.register_url", c.Site.RegisterURL},
		{"ucenter.endpoint", c.UCenter.Endpoint},
		{"textures.public_base_url", c.Textures.PublicBaseURL},
		{"textures.mojang_base_url", c.Textures.MojangBaseURL},
		{"textures.crafatar_base_url", c.Textures.CrafatarBaseURL},
		{"textures.crafthead_base_url", c.Textures.CraftheadBaseURL},
	}
	for _, field := range urls {
		u, err := url.Parse(field.value)
		if err != nil || u.Host == "" || (u.Scheme != "https" && u.Scheme != "http") || u.User != nil {
			return fmt.Errorf("%s must be an HTTP or HTTPS URL without credentials", field.name)
		}
	}
	if c.Database.Port < 1 || c.Database.Port > 65535 {
		return errors.New("database.port must be between 1 and 65535")
	}
	if c.Database.MaxOpenConnections < 1 || c.Database.MaxIdleConnections < 0 || c.Database.MaxIdleConnections > c.Database.MaxOpenConnections {
		return errors.New("database connection pool sizes are invalid")
	}
	durations := []struct {
		name  string
		value Duration
	}{
		{"server.read_header_timeout", c.Server.ReadHeaderTimeout},
		{"server.read_timeout", c.Server.ReadTimeout},
		{"server.write_timeout", c.Server.WriteTimeout},
		{"server.shutdown_timeout", c.Server.ShutdownTimeout},
		{"database.connect_timeout", c.Database.ConnectTimeout},
		{"database.query_timeout", c.Database.QueryTimeout},
		{"ucenter.request_timeout", c.UCenter.RequestTimeout},
		{"auth.login_record_retention", c.Auth.LoginRecordRetention},
		{"textures.request_timeout", c.Textures.RequestTimeout},
	}
	for _, field := range durations {
		if field.value <= 0 {
			return fmt.Errorf("%s must be a positive duration", field.name)
		}
	}
	switch c.Logging.Level {
	case "debug", "info", "warn", "error":
	default:
		return errors.New("logging.level must be debug, info, warn or error")
	}
	if c.Logging.Format != "json" && c.Logging.Format != "text" {
		return errors.New("logging.format must be json or text")
	}
	return nil
}

func readSecret(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	value := strings.TrimRight(string(data), "\r\n")
	if value == "" {
		return "", errors.New("empty secret")
	}
	return value, nil
}

func (c *Config) loadKeys() error {
	privateData, err := os.ReadFile(c.Auth.PrivateKeyFile)
	if err != nil {
		return errors.New("cannot read auth.private_key_file")
	}
	block, _ := pem.Decode(privateData)
	if block == nil {
		return errors.New("auth.private_key_file must contain a PEM RSA private key")
	}
	if key, err := x509.ParsePKCS1PrivateKey(block.Bytes); err == nil {
		c.key = key
	} else {
		key, err := x509.ParsePKCS8PrivateKey(block.Bytes)
		if err != nil {
			return errors.New("auth.private_key_file must contain a PKCS1 or PKCS8 RSA private key")
		}
		var ok bool
		c.key, ok = key.(*rsa.PrivateKey)
		if !ok {
			return errors.New("auth.private_key_file must contain an RSA key")
		}
	}
	if err := c.key.Validate(); err != nil {
		return errors.New("auth.private_key_file contains an invalid RSA key")
	}
	publicData, err := os.ReadFile(c.Auth.PublicKeyFile)
	if err != nil {
		return errors.New("cannot read auth.public_key_file")
	}
	block, _ = pem.Decode(publicData)
	if block == nil {
		return errors.New("auth.public_key_file must contain a PEM public key")
	}
	var public *rsa.PublicKey
	if key, err := x509.ParsePKCS1PublicKey(block.Bytes); err == nil {
		public = key
	} else {
		key, err := x509.ParsePKIXPublicKey(block.Bytes)
		if err != nil {
			return errors.New("auth.public_key_file must contain a PKCS1 or PKIX RSA public key")
		}
		var ok bool
		public, ok = key.(*rsa.PublicKey)
		if !ok {
			return errors.New("auth.public_key_file must contain an RSA key")
		}
	}
	if !public.Equal(&c.key.PublicKey) {
		return errors.New("auth public and private keys do not match")
	}
	c.public = string(publicData)
	return nil
}

func (c *Config) DatabasePassword() string    { return c.password }
func (c *Config) UCenterSecret() string       { return c.secret }
func (c *Config) SuperKey() string            { return c.superKey }
func (c *Config) PrivateKey() *rsa.PrivateKey { return c.key }
func (c *Config) PublicKeyPEM() string        { return c.public }
