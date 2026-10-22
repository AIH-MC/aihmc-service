package auth

import (
	"crypto"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha1"
	"crypto/x509"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"encoding/pem"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/AIH-MC/aihmc-service/internal/testutil"
)

func TestLegacyPHPProtocolVectors(t *testing.T) {
	data, err := os.ReadFile(filepath.Join(testutil.Root(t), "docs/legacy-protocol-vectors.json"))
	if err != nil {
		t.Fatal(err)
	}
	var vector struct {
		Timestamp        int64
		Username         string
		Email            string
		UID              int64
		ClientToken      string
		AccessToken      string
		ProfileID        string
		RefreshTimestamp int64
		RefreshToken     string
		TextureValue     string
		RSA              struct{ Value, Signature, PublicKey string }
	}
	if err := json.Unmarshal(data, &vector); err != nil {
		t.Fatal(err)
	}
	if AccessToken(vector.Username, vector.ClientToken, vector.Timestamp) != vector.AccessToken {
		t.Fatal("access token differs from PHP reference")
	}
	if ProfileID(vector.Username, vector.Email, vector.UID) != vector.ProfileID {
		t.Fatal("profile ID differs from PHP reference")
	}
	if RefreshedToken(vector.ClientToken, vector.RefreshTimestamp) != vector.RefreshToken {
		t.Fatal("refreshed token differs from PHP reference")
	}
	texture, err := EncodeTexture(vector.Timestamp, "fixture-id", "迁移测试", "fixture-skin", "slim", "fixture-cape", "https://api.aihmc.top/textures")
	if err != nil || texture != vector.TextureValue {
		t.Fatal("new texture value differs byte-for-byte from PHP reference", err)
	}
	block, _ := pem.Decode([]byte(vector.RSA.PublicKey))
	if block == nil {
		t.Fatal("missing test public key")
	}
	key, err := x509.ParsePKIXPublicKey(block.Bytes)
	if err != nil {
		t.Fatal(err)
	}
	signature, err := base64.StdEncoding.DecodeString(vector.RSA.Signature)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha1.Sum([]byte(vector.RSA.Value))
	if err := rsa.VerifyPKCS1v15(key.(*rsa.PublicKey), crypto.SHA1, digest[:], signature); err != nil {
		t.Fatal("PHP signature failed Go verification:", err)
	}
}

func TestRandomIdentifiersAndSignature(t *testing.T) {
	uuid, err := NewUUID()
	if err != nil {
		t.Fatal(err)
	}
	data, err := hex.DecodeString(uuid)
	if err != nil || len(data) != 16 || data[6]>>4 != 4 || data[8]>>6 != 2 {
		t.Fatal("UUID does not have the legacy unhyphenated v4 format")
	}
	client, err := ClientToken()
	if err != nil || len(client) != 32 {
		t.Fatal("client token generation failed")
	}
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	value := "existing-base64-value"
	encoded, err := Sign(value, key)
	if err != nil {
		t.Fatal(err)
	}
	signature, err := base64.StdEncoding.DecodeString(encoded)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha1.Sum([]byte(value))
	if err := rsa.VerifyPKCS1v15(&key.PublicKey, crypto.SHA1, digest[:], signature); err != nil {
		t.Fatal(err)
	}
}

func TestWireShapeAndPHPTruthValues(t *testing.T) {
	for input, want := range map[string]bool{"": false, "0": false, "false": true, "true": true, "1": true} {
		if PHPTruthyQuery(input) != want {
			t.Fatalf("wrong PHP truth value for %q", input)
		}
	}
	profile := Profile{ID: "fixture-id", Name: "迁移测试"}
	encoded, err := json.Marshal(Login{
		AccessToken: "a", ClientToken: "c", SelectedProfile: &profile,
		AvailableProfiles: []Profile{profile}, User: &User{ID: "user-id", Properties: []Property{}},
	})
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(encoded, &fields); err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(fields["selectedProfile"]), "properties") || !strings.Contains(string(fields["user"]), `"properties":[]`) {
		t.Fatal("profile omission or user empty-array wire shape differs")
	}
	value, err := EncodeTexture(1700000000, "id", "迁移测试", "skin", "slim", "cape", "https://api.example.com/textures")
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := base64.StdEncoding.DecodeString(value)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(decoded), `https:\/\/api.example.com\/textures\/skin`) || !strings.Contains(string(decoded), "迁移测试") {
		t.Fatal("texture JSON does not preserve PHP slash escaping and Unicode")
	}
}
