package config

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func template(t *testing.T) string {
	t.Helper()
	directory, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		data, err := os.ReadFile(filepath.Join(directory, "config.example.toml"))
		if err == nil {
			return string(data)
		}
		parent := filepath.Dir(directory)
		if parent == directory {
			t.Fatal("config.example.toml not found")
		}
		directory = parent
	}
}

func fixture(t *testing.T) (string, string) {
	t.Helper()
	directory := t.TempDir()
	for _, name := range []string{"secrets", "keys", "textures"} {
		if err := os.Mkdir(filepath.Join(directory, name), 0700); err != nil {
			t.Fatal(err)
		}
	}
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	public, err := x509.MarshalPKIXPublicKey(&key.PublicKey)
	if err != nil {
		t.Fatal(err)
	}
	files := map[string][]byte{
		"keys/yggdrasil-private-key.pem": pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(key)}),
		"keys/yggdrasil-public-key.pem":  pem.EncodeToMemory(&pem.Block{Type: "PUBLIC KEY", Bytes: public}),
		"secrets/database-password":      []byte("  private-db-value  \r\n"),
		"secrets/ucenter-secret":         []byte("private-ucenter-value\n"),
		"secrets/admin-super-key":        []byte("private-admin-value"),
	}
	for name, data := range files {
		if err := os.WriteFile(filepath.Join(directory, name), data, 0600); err != nil {
			t.Fatal(err)
		}
	}
	return filepath.Join(directory, "config.toml"), template(t)
}

func writeConfig(t *testing.T, path, text string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(text), 0600); err != nil {
		t.Fatal(err)
	}
}

func TestLoadResolvesPathsAndPreservesSecretWhitespace(t *testing.T) {
	path, text := fixture(t)
	writeConfig(t, path, text)
	cfg, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.DatabasePassword() != "  private-db-value  " {
		t.Fatal("secret whitespace was changed")
	}
	if cfg.UCenterSecret() != "private-ucenter-value" || cfg.SuperKey() != "private-admin-value" {
		t.Fatal("secret references were not loaded")
	}
	if cfg.Database.ConnectTimeout.Value() != 5*time.Second {
		t.Fatal("duration was not decoded")
	}
	if cfg.Textures.Directory != filepath.Join(filepath.Dir(path), "textures") {
		t.Fatal("resource path was not resolved relative to configuration file")
	}
	if cfg.PrivateKey() == nil || !strings.Contains(cfg.PublicKeyPEM(), "PUBLIC KEY") {
		t.Fatal("RSA key pair was not loaded")
	}
	entries, err := os.ReadDir(cfg.Textures.Directory)
	if err != nil || len(entries) != 0 {
		t.Fatal("directory write probe was not cleaned up")
	}
}

func TestInvalidConfigurationFailsWithoutLeakingValues(t *testing.T) {
	cases := []struct {
		name string
		edit func(string) string
	}{
		{"unknown field", func(s string) string { return s + "\nsecret_typo = \"private-secret-marker\"\n" }},
		{"invalid duration", func(s string) string {
			return strings.Replace(s, `connect_timeout = "5s"`, `connect_timeout = "private-secret-marker"`, 1)
		}},
		{"zero duration", func(s string) string {
			return strings.Replace(s, `connect_timeout = "5s"`, `connect_timeout = "0s"`, 1)
		}},
		{"missing field", func(s string) string { return strings.Replace(s, `host = "127.0.0.1"`, `host = ""`, 1) }},
		{"invalid port", func(s string) string { return strings.Replace(s, "port = 3306", "port = 0", 1) }},
		{"invalid pool", func(s string) string {
			return strings.Replace(s, "max_idle_connections = 5", "max_idle_connections = 15", 1)
		}},
		{"wrong type", func(s string) string { return strings.Replace(s, "port = 3306", `port = "private-secret-marker"`, 1) }},
		{"invalid URL", func(s string) string {
			return strings.Replace(s, "https://ucenter.example.com/api", "file:///private-secret-marker", 1)
		}},
		{"duplicate field", func(s string) string { return strings.Replace(s, "[database]", "[database]\nport = 1234", 1) }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			path, text := fixture(t)
			writeConfig(t, path, test.edit(text))
			_, err := Load(path)
			if err == nil {
				t.Fatal("invalid configuration was accepted")
			}
			if strings.Contains(err.Error(), "private-secret-marker") {
				t.Fatal("configuration error leaked a source value")
			}
		})
	}
}

func TestMissingRuntimeConfigurationDoesNotUseTemplate(t *testing.T) {
	path, text := fixture(t)
	example := filepath.Join(filepath.Dir(path), "config.example.toml")
	writeConfig(t, example, text)
	if _, err := Load(path); err == nil {
		t.Fatal("missing config.toml must fail")
	}
	if _, err := Load(example); err == nil {
		t.Fatal("template must not be accepted as runtime configuration")
	}
	if _, err := Load(filepath.Join(filepath.Dir(path), "CONFIG.EXAMPLE.TOML")); err == nil || !strings.Contains(err.Error(), "template") {
		t.Fatal("template rejection must respect case-insensitive Windows filenames")
	}
}

func TestMismatchedKeysFail(t *testing.T) {
	path, text := fixture(t)
	writeConfig(t, path, text)
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	data := pem.EncodeToMemory(&pem.Block{Type: "RSA PUBLIC KEY", Bytes: x509.MarshalPKCS1PublicKey(&key.PublicKey)})
	if err := os.WriteFile(filepath.Join(filepath.Dir(path), "keys/yggdrasil-public-key.pem"), data, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(path); err == nil || !strings.Contains(err.Error(), "do not match") {
		t.Fatal("mismatched key pair was accepted")
	}
}

func TestMissingSecretFails(t *testing.T) {
	path, text := fixture(t)
	writeConfig(t, path, text)
	if err := os.Remove(filepath.Join(filepath.Dir(path), "secrets/database-password")); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(path); err == nil || !strings.Contains(err.Error(), "database.password_file") {
		t.Fatal("missing secret was accepted")
	}
}
