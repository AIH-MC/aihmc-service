// Command protocol-bridge verifies Rust signatures with the unchanged Go implementation.
// It accepts only synthetic test inputs on stdin and never opens a database.
package main

import (
	"crypto"
	"crypto/rsa"
	"crypto/sha1"
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"os"

	"github.com/AIH-MC/aihmc-service/internal/auth"
	"github.com/AIH-MC/aihmc-service/internal/config"
)

func main() {
	path := flag.String("config", "", "synthetic runtime config")
	flag.Parse()
	if err := verify(*path); err != nil {
		fmt.Fprintln(os.Stderr, "protocol bridge failed")
		os.Exit(1)
	}
}

func verify(path string) error {
	cfg, err := config.Load(path)
	if err != nil {
		return err
	}
	var input struct{ Value, Signature string }
	if err := json.NewDecoder(os.Stdin).Decode(&input); err != nil {
		return err
	}
	signature, err := base64.StdEncoding.DecodeString(input.Signature)
	if err != nil {
		return err
	}
	digest := sha1.Sum([]byte(input.Value))
	if err := rsa.VerifyPKCS1v15(&cfg.PrivateKey().PublicKey, crypto.SHA1, digest[:], signature); err != nil {
		return err
	}
	generated, err := auth.Sign(input.Value, cfg.PrivateKey())
	if err != nil {
		return err
	}
	return json.NewEncoder(os.Stdout).Encode(map[string]any{"verified": true, "signature": generated})
}
