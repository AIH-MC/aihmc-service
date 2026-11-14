package ucenter

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strconv"
	"sync/atomic"
	"testing"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/testutil"
)

func TestSignatureMatchesPHP(t *testing.T) {
	data, err := os.ReadFile(filepath.Join(testutil.Root(t), "docs/legacy-protocol-vectors.json"))
	if err != nil {
		t.Fatal(err)
	}
	var vector struct {
		Timestamp int64
		UCenter   struct{ Nonce, Secret, Signature string }
	}
	if err := json.Unmarshal(data, &vector); err != nil {
		t.Fatal(err)
	}
	if Signature(vector.UCenter.Nonce, vector.Timestamp, vector.UCenter.Secret) != vector.UCenter.Signature {
		t.Fatal("UCenter signature differs from original PHP function")
	}
}

func TestConfiguredClientDoesNotFollowRedirects(t *testing.T) {
	var redirected atomic.Int64
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/redirected" {
			redirected.Add(1)
			json.NewEncoder(w).Encode(map[string]string{"token": "unexpected"})
			return
		}
		http.Redirect(w, r, "/redirected", http.StatusFound)
	}))
	defer server.Close()
	cfg := &config.Config{UCenter: config.UCenter{Endpoint: server.URL, RequestTimeout: config.Duration(time.Second)}}
	client := New(cfg)
	if _, err := client.Login(context.Background(), "fixture", "password"); !errors.Is(err, ErrUpstream) {
		t.Fatal("redirect response was accepted")
	}
	if redirected.Load() != 0 {
		t.Fatal("client followed a redirect unlike the legacy cURL client")
	}
}

func TestLoginThroughQueryStyleEndpoint(t *testing.T) {
	for _, test := range []struct {
		name     string
		password string
		want     error
	}{
		{"email", "valid-password", nil},
		{"rejected", "invalid-password", ErrInvalidCredentials},
	} {
		t.Run(test.name, func(t *testing.T) {
			requests := 0
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				requests++
				timestamp, err := strconv.ParseInt(r.Header.Get("t"), 10, 64)
				if err != nil || r.Header.Get("sign") != Signature("nonce", timestamp, "secret") || r.Header.Get("appid") != "app" {
					t.Error("missing or invalid signed headers")
					w.WriteHeader(403)
					return
				}
				if r.Method != http.MethodPost || r.URL.Path != "/api/" {
					t.Error("endpoint concatenation changed")
				}
				var body map[string]any
				if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
					t.Error(err)
				}
				w.Header().Set("Content-Type", "application/json")
				switch r.URL.RawQuery {
				case "/token":
					json.NewEncoder(w).Encode(map[string]string{"token": "upstream-token"})
				case "/user/login":
					if body["isuid"] != float64(2) || r.Header.Get("token") != "upstream-token" {
						t.Error("email login or token header differs")
					}
					status := 1
					if body["password"] != "valid-password" {
						status = -1
					}
					json.NewEncoder(w).Encode(map[string]any{"data": map[string]any{"status": status, "username": "迁移测试"}})
				case "/user/get_user":
					if body["username"] != "迁移测试" || body["isuid"] != float64(0) {
						t.Error("user lookup request differs")
					}
					json.NewEncoder(w).Encode(map[string]any{"data": map[string]any{"uid": "910001", "username": "迁移测试", "email": "test@example.com"}})
				default:
					t.Error("unexpected upstream path")
					w.WriteHeader(404)
				}
			}))
			defer server.Close()
			client := &Client{endpoint: server.URL + "/api/?", appID: "app", nonce: "nonce", secret: "secret", http: &http.Client{Timeout: time.Second}}
			user, err := client.Login(context.Background(), "test@example.com", test.password)
			if !errors.Is(err, test.want) {
				t.Fatalf("want %v, got %v", test.want, err)
			}
			if test.want == nil && (user.UID != 910001 || user.Name != "迁移测试" || requests != 4) {
				t.Fatal("user result or request sequence differs")
			}
		})
	}
}

func TestUpstreamFailuresAndCancellation(t *testing.T) {
	for _, status := range []int{200, 403, 500} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(status)
			w.Write([]byte("not-json"))
		}))
		client := &Client{endpoint: server.URL, http: &http.Client{Timeout: time.Second}}
		if _, err := client.Login(context.Background(), "name", "password"); !errors.Is(err, ErrUpstream) {
			t.Fatal("invalid upstream response was accepted")
		}
		ctx, cancel := context.WithCancel(context.Background())
		cancel()
		if _, err := client.Login(ctx, "name", "password"); !errors.Is(err, ErrUpstream) {
			t.Fatal("canceled request was accepted")
		}
		server.Close()
	}
}
