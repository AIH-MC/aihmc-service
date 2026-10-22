package texture

import (
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/store"
	"github.com/AIH-MC/aihmc-service/internal/testutil"
)

func TestConcurrentTexturePublication(t *testing.T) {
	data, err := os.ReadFile(filepath.Join(testutil.Root(t), "tests/testdata/skin-64.png"))
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	service := &Service{cfg: &config.Config{Textures: config.Textures{Directory: directory}}}
	var group sync.WaitGroup
	failures := make(chan error, 8)
	start := make(chan struct{})
	for iteration := 0; iteration < 8; iteration++ {
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			if err := service.save("same-hash", data); err != nil {
				failures <- err
			}
		}()
	}
	close(start)
	group.Wait()
	close(failures)
	for err := range failures {
		t.Error(err)
	}
	saved, err := os.ReadFile(filepath.Join(directory, "same-hash"))
	if err != nil || !bytes.Equal(saved, data) {
		t.Fatal("published texture does not match the complete source PNG")
	}
	entries, err := os.ReadDir(directory)
	if err != nil || len(entries) != 1 {
		t.Fatal("temporary texture files were not cleaned up")
	}
}

func TestMaterialClientDoesNotFollowRedirects(t *testing.T) {
	var redirected atomic.Int64
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/redirected" {
			redirected.Add(1)
			w.Write([]byte("unexpected"))
			return
		}
		http.Redirect(w, r, "/redirected", http.StatusFound)
	}))
	defer server.Close()
	s := fixtureService(t, server.URL, fixtureRepository{apis: []string{server.URL}})
	material, err := s.Resolve(context.Background(), "Fixture")
	if err != nil || material.Skin != "" || material.Cape != "" {
		t.Fatal("redirect was not treated as an unavailable optional source", err)
	}
	if redirected.Load() != 0 {
		t.Fatal("material client followed a redirect unlike the legacy cURL client")
	}
}

type fixtureRepository struct {
	apis    []string
	profile *store.Profile
}

func (r fixtureRepository) SkinAPIs(context.Context) ([]string, error) { return r.apis, nil }

func (r fixtureRepository) ProfileByUUID(context.Context, string) (store.Profile, error) {
	if r.profile == nil {
		return store.Profile{}, sql.ErrNoRows
	}
	return *r.profile, nil
}

func fixtureService(t *testing.T, serverURL string, repository fixtureRepository) *Service {
	t.Helper()
	cfg := &config.Config{Textures: config.Textures{
		Directory: t.TempDir(), RequestTimeout: config.Duration(time.Second),
		MojangBaseURL: serverURL, CrafatarBaseURL: serverURL, CraftheadBaseURL: serverURL,
	}}
	return NewService(cfg, repository)
}

func skinFixture(t *testing.T) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(testutil.Root(t), "tests/testdata/skin-64.png"))
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func TestOrderedSourcesAndMissingCapeFallback(t *testing.T) {
	data := skinFixture(t)
	calls := []string{}
	var callsMutex sync.Mutex
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callsMutex.Lock()
		calls = append(calls, r.RequestURI)
		callsMutex.Unlock()
		switch {
		case strings.HasPrefix(r.URL.Path, "/first/csl/"):
			w.WriteHeader(404)
		case strings.HasPrefix(r.URL.Path, "/second/csl/"):
			json.NewEncoder(w).Encode(map[string]any{"skins": map[string]string{"slim": "remote-skin"}})
		case r.URL.Path == "/second/textures/remote-skin":
			w.Write(data)
		case strings.HasPrefix(r.URL.Path, "/users/profiles/minecraft/"):
			json.NewEncoder(w).Encode(map[string]string{"id": "fallback-uuid"})
		case r.URL.Path == "/capes/fallback-uuid":
			w.Write(data)
		default:
			t.Error("unexpected upstream request", r.URL.Path)
			w.WriteHeader(404)
		}
	}))
	defer server.Close()
	s := fixtureService(t, server.URL, fixtureRepository{apis: []string{server.URL + "/first", server.URL + "/second"}})
	material, err := s.Resolve(context.Background(), "迁移用户 A")
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(data)
	wantCape := hex.EncodeToString(digest[:])
	if material.Skin != "remote-skin" || material.Model != "slim" || material.Cape != wantCape {
		t.Fatalf("unexpected material: %+v", material)
	}
	callsMutex.Lock()
	defer callsMutex.Unlock()
	if len(calls) != 5 || calls[0] != "/first/csl/"+url.QueryEscape("迁移用户 A")+".json" || !strings.HasPrefix(calls[1], "/second/csl/") {
		t.Fatal("source order or PHP-compatible username encoding changed", calls)
	}
	for _, key := range []string{material.Skin, material.Cape} {
		saved, err := os.ReadFile(filepath.Join(s.cfg.Textures.Directory, key))
		if err != nil || !bytes.Equal(saved, data) {
			t.Fatal("downloaded material was not stored intact")
		}
	}
}

func TestFallbackSkinAndMissingCape(t *testing.T) {
	data := skinFixture(t)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.HasPrefix(r.URL.Path, "/users/profiles/minecraft/"):
			json.NewEncoder(w).Encode(map[string]string{"id": "fallback-uuid"})
		case r.URL.Path == "/skins/fallback-uuid":
			w.Write(data)
		default:
			w.WriteHeader(404)
		}
	}))
	defer server.Close()
	s := fixtureService(t, server.URL, fixtureRepository{apis: []string{server.URL}})
	material, err := s.Resolve(context.Background(), "Fixture")
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(data)
	if material.Skin != hex.EncodeToString(digest[:]) || material.Cape != "" || material.Model != "default" {
		t.Fatalf("fallback hashing or model differs: %+v", material)
	}
}

func TestMissingCorruptAndInvalidKeyMaterialsDoNotPublish(t *testing.T) {
	for _, mode := range []string{"missing", "malformed-json", "corrupt-png", "invalid-key"} {
		t.Run(mode, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if strings.HasPrefix(r.URL.Path, "/csl/") {
					switch mode {
					case "malformed-json":
						w.Write([]byte("{"))
					case "corrupt-png":
						json.NewEncoder(w).Encode(map[string]any{"skins": map[string]string{"default": "broken"}})
					case "invalid-key":
						json.NewEncoder(w).Encode(map[string]any{"skins": map[string]string{"default": "../outside"}})
					default:
						w.WriteHeader(404)
					}
				} else if r.URL.Path == "/textures/broken" {
					w.Write([]byte("not-a-png"))
				} else {
					w.WriteHeader(404)
				}
			}))
			defer server.Close()
			s := fixtureService(t, server.URL, fixtureRepository{apis: []string{server.URL}})
			material, err := s.Resolve(context.Background(), "Fixture")
			if err != nil || material.Skin != "" || material.Cape != "" {
				t.Fatal("optional missing material rejected login or advertised invalid data", err)
			}
			entries, err := os.ReadDir(s.cfg.Textures.Directory)
			if err != nil || len(entries) != 0 {
				t.Fatal("invalid data was published")
			}
			ctx, cancel := context.WithCancel(context.Background())
			cancel()
			if _, err := s.Resolve(ctx, "Fixture"); !errors.Is(err, context.Canceled) {
				t.Fatal("cancellation was ignored")
			}
		})
	}
}

func TestLocalAvatarAndExternalAvatarFallback(t *testing.T) {
	data := skinFixture(t)
	expected, err := os.ReadFile(filepath.Join(testutil.Root(t), "tests/testdata/avatar-64-20.png"))
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.HasPrefix(r.URL.Path, "/csl/"):
			json.NewEncoder(w).Encode(map[string]any{"skins": map[string]string{"default": "skin"}, "cape": "cape"})
		case strings.HasPrefix(r.URL.Path, "/textures/"):
			w.Write(data)
		case r.URL.Path == "/helm/missing-uuid/20":
			w.Write(expected)
		default:
			w.WriteHeader(404)
		}
	}))
	defer server.Close()
	s := fixtureService(t, server.URL, fixtureRepository{apis: []string{server.URL}, profile: &store.Profile{Name: "Fixture"}})
	actual, err := s.Avatar(context.Background(), "local-uuid", 20)
	if err != nil {
		t.Fatal(err)
	}
	actualImage, err := DecodePNG(actual)
	if err != nil {
		t.Fatal(err)
	}
	if actualImage.Bounds().Dx() != 20 || actualImage.Bounds().Dy() != 20 {
		t.Fatal("local avatar dimensions differ")
	}
	remote := fixtureService(t, server.URL, fixtureRepository{})
	actual, err = remote.Avatar(context.Background(), "missing-uuid", 20)
	if err != nil || !bytes.Equal(actual, expected) {
		t.Fatal("external avatar was not forwarded intact", err)
	}
	if _, err := remote.Avatar(context.Background(), "missing-uuid", -1); err == nil {
		t.Fatal("negative avatar size was accepted")
	}
	if _, err := remote.Avatar(context.Background(), "upstream-missing", 20); err == nil {
		t.Fatal("missing fallback avatar was accepted")
	}
}

func TestFilesystemFailureIsReported(t *testing.T) {
	data := skinFixture(t)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/csl/") {
			json.NewEncoder(w).Encode(map[string]any{"skins": map[string]string{"default": "skin"}})
		} else {
			w.Write(data)
		}
	}))
	defer server.Close()
	s := fixtureService(t, server.URL, fixtureRepository{apis: []string{server.URL}})
	file := filepath.Join(s.cfg.Textures.Directory, "not-a-directory")
	if err := os.WriteFile(file, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	s.cfg.Textures.Directory = file
	if _, err := s.Resolve(context.Background(), "Fixture"); err == nil {
		t.Fatal("filesystem publication failure was ignored")
	}
}
