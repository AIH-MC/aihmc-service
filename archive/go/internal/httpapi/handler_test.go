package httpapi_test

import (
	"bytes"
	"context"
	"crypto"
	"crypto/rsa"
	"crypto/sha1"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/auth"
	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/httpapi"
	"github.com/AIH-MC/aihmc-service/internal/store"
	"github.com/AIH-MC/aihmc-service/internal/testutil"
	"github.com/AIH-MC/aihmc-service/internal/texture"
	"github.com/AIH-MC/aihmc-service/internal/ucenter"
)

type fixtureMaterials struct{}

func (fixtureMaterials) Resolve(context.Context, string) (auth.Material, error) {
	return auth.Material{Skin: "fixture-skin", Model: "slim", Cape: "fixture-cape"}, nil
}

type fixture struct {
	server *httptest.Server
	db     *sql.DB
	store  *store.Store
	cfg    *config.Config
}

func integrationFixture(t *testing.T) fixture {
	t.Helper()
	path := os.Getenv("AIHMC_TEST_CONFIG")
	if path == "" {
		t.Skip("set AIHMC_TEST_CONFIG for real database and HTTP integration")
	}
	cfg, err := config.Load(path)
	if err != nil {
		t.Fatal(err)
	}
	db := testutil.Database(t, cfg, store.Open)
	clean := func() {
		if _, err := db.Exec("DELETE FROM texture_apis WHERE id = 920001"); err != nil {
			t.Error(err)
		}
		if _, err := db.Exec("DELETE FROM mcauth_logins WHERE server_id LIKE 'go-http-%'"); err != nil {
			t.Error(err)
		}
		for _, table := range []string{"mcauth_tokens", "mcauth_profiles", "mcauth_textures"} {
			if _, err := db.Exec("DELETE FROM " + table + " WHERE uid BETWEEN 920001 AND 920004"); err != nil {
				t.Error(err)
			}
		}
	}
	clean()
	t.Cleanup(clean)
	users := map[string]int64{"MigrationHTTP": 920001, "迁移用户": 920002, "ConcurrentFirst": 920003}
	png, err := os.ReadFile(filepath.Join(testutil.Root(t), "tests/testdata/skin-64.png"))
	if err != nil {
		t.Fatal(err)
	}
	avatarPNG, err := os.ReadFile(filepath.Join(testutil.Root(t), "tests/testdata/avatar-64-20.png"))
	if err != nil {
		t.Fatal(err)
	}
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method == "GET" {
			if strings.HasPrefix(r.URL.Path, "/csl/") {
				w.Header().Set("Content-Type", "application/json")
				json.NewEncoder(w).Encode(map[string]any{"skins": map[string]string{"slim": "fixture-skin"}, "cape": "fixture-cape"})
			} else if strings.HasPrefix(r.URL.Path, "/textures/") {
				w.Header().Set("Content-Type", "image/png")
				w.Write(png)
			} else if strings.HasPrefix(r.URL.Path, "/helm/") {
				w.Header().Set("Content-Type", "image/png")
				w.Write(avatarPNG)
			} else {
				w.WriteHeader(404)
			}
			return
		}
		var body map[string]any
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			t.Error(err)
			w.WriteHeader(400)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		switch r.URL.Path {
		case "/token":
			json.NewEncoder(w).Encode(map[string]string{"token": "fixture-token"})
		case "/user/login":
			status := 1
			if body["password"] != "fixture-password" {
				status = -1
			}
			json.NewEncoder(w).Encode(map[string]any{"data": map[string]any{"status": status, "username": body["username"]}})
		case "/user/get_user":
			name, _ := body["username"].(string)
			json.NewEncoder(w).Encode(map[string]any{"data": map[string]any{"uid": users[name], "username": name, "email": "fixture@example.com"}})
		default:
			w.WriteHeader(404)
		}
	}))
	t.Cleanup(upstream.Close)
	cfg.UCenter.Endpoint = upstream.URL
	cfg.Textures.Directory = t.TempDir()
	cfg.Textures.MojangBaseURL = upstream.URL
	cfg.Textures.CrafatarBaseURL = upstream.URL
	cfg.Textures.CraftheadBaseURL = upstream.URL
	if _, err := db.Exec("INSERT INTO texture_apis(id, api_url, added_by, added_date) VALUES (920001, ?, 'go-http-fixture', 0)", upstream.URL); err != nil {
		t.Fatal(err)
	}
	databaseStore := store.New(db, cfg.Database.QueryTimeout.Value())
	materials := texture.NewService(cfg, databaseStore)
	service := auth.NewService(cfg, databaseStore, ucenter.New(cfg), materials)
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	server := httptest.NewServer(httpapi.New(service, materials, logger))
	t.Cleanup(server.Close)
	return fixture{server: server, db: db, store: databaseStore, cfg: cfg}
}

func request(t *testing.T, f fixture, method, path string, body any, want int) []byte {
	t.Helper()
	var data []byte
	if raw, ok := body.(json.RawMessage); ok {
		data = []byte(raw)
	} else if body != nil {
		var err error
		data, err = json.Marshal(body)
		if err != nil {
			t.Fatal(err)
		}
	}
	r, err := http.NewRequest(method, f.server.URL+path, bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	r.Header.Set("Content-Type", "application/json")
	response, err := f.server.Client().Do(r)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	result, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != want {
		t.Fatalf("%s %s: want %d, got %d: %s", method, path, want, response.StatusCode, result)
	}
	if want == 204 && len(result) != 0 {
		t.Fatal("204 response contained a body")
	}
	if strings.HasPrefix(path, "/helm/") && want == 200 && response.Header.Get("Content-Type") != "image/png" {
		t.Fatal("avatar response did not use image/png")
	}
	return result
}

func readBody[T any](t *testing.T, data []byte) T {
	t.Helper()
	var value T
	if err := json.Unmarshal(data, &value); err != nil {
		t.Fatal(err)
	}
	return value
}

func loginRequest(name, client string, requestUser any) map[string]any {
	return map[string]any{
		"username": name, "password": "fixture-password", "clientToken": client,
		"requestUser": requestUser, "agent": map[string]any{"name": "Minecraft", "version": 1},
	}
}

func tokenRequest(login auth.Login) map[string]any {
	return map[string]any{"accessToken": login.AccessToken, "clientToken": login.ClientToken}
}

func verifySignedProfile(t *testing.T, cfg *config.Config, profile auth.Profile) {
	t.Helper()
	if len(profile.Properties) != 1 || profile.Properties[0].Signature == "" {
		t.Fatal("missing signed textures property")
	}
	property := profile.Properties[0]
	signature, err := base64.StdEncoding.DecodeString(property.Signature)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha1.Sum([]byte(property.Value))
	if err := rsa.VerifyPKCS1v15(&cfg.PrivateKey().PublicKey, crypto.SHA1, digest[:], signature); err != nil {
		t.Fatal("response signature does not verify:", err)
	}
}

func TestHTTPAuthenticationAndSessionSequence(t *testing.T) {
	f := integrationFixture(t)
	request(t, f, "GET", "/", nil, 200)
	request(t, f, "POST", "/", map[string]any{}, 200)
	for _, path := range []string{"/mcauth", "/unknown", "/mcauth?x=1", "/mcauth/"} {
		body := readBody[map[string]json.RawMessage](t, request(t, f, "GET", path, nil, 200))
		if _, ok := body["signaturePublickey"]; !ok {
			t.Fatal("default route did not return metadata")
		}
	}
	request(t, f, "POST", "/mcauth", map[string]any{}, 405)
	request(t, f, "POST", "/mcauth/authserver/authenticate", json.RawMessage("{"), 400)
	request(t, f, "POST", "/mcauth/authserver/authenticate", map[string]any{}, 400)
	badAgent := loginRequest("MigrationHTTP", "go-http-client", false)
	badAgent["agent"] = map[string]any{"name": "other"}
	request(t, f, "POST", "/mcauth/authserver/authenticate", badAgent, 403)
	badAgent["agent"] = map[string]any{"name": "MİNECRAFT"}
	request(t, f, "POST", "/mcauth/authserver/authenticate", badAgent, 403)
	wrongPassword := loginRequest("MigrationHTTP", "go-http-client", false)
	wrongPassword["password"] = "wrong-password"
	request(t, f, "POST", "/mcauth/authserver/authenticate", wrongPassword, 403)
	login := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("MigrationHTTP", "go-http-client", false), 200))
	if login.SelectedProfile == nil || login.User != nil || len(login.AvailableProfiles) != 1 {
		t.Fatal("initial login response shape differs")
	}
	verifyTokenWithLegacyPHP(t, login)
	repeated := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("MigrationHTTP", "go-http-client", true), 200))
	if repeated.SelectedProfile.ID != login.SelectedProfile.ID || repeated.AccessToken != login.AccessToken || repeated.User == nil {
		t.Fatal("repeated login changed stored identity or token")
	}
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(login), 204)
	profilePath := "/mcauth/sessionserver/session/minecraft/profile/" + login.SelectedProfile.ID
	profile := readBody[auth.Profile](t, request(t, f, "GET", profilePath, nil, 200))
	verifySignedProfile(t, f.cfg, profile)
	textureData, err := base64.StdEncoding.DecodeString(profile.Properties[0].Value)
	if err != nil {
		t.Fatal(err)
	}
	texture := readBody[auth.TexturePayload](t, textureData)
	if texture.ProfileID != login.SelectedProfile.ID || texture.Textures.Skin.Metadata["model"] != "slim" {
		t.Fatal("stored texture identity or model differs")
	}
	for _, suffix := range []string{"?unsigned=0", "?unsigned=false&unsigned=0"} {
		verifySignedProfile(t, f.cfg, readBody[auth.Profile](t, request(t, f, "GET", profilePath+suffix, nil, 200)))
	}
	unsigned := readBody[auth.Profile](t, request(t, f, "GET", profilePath+"?unsigned=false", nil, 200))
	if unsigned.Properties[0].Signature != "" {
		t.Fatal("PHP truth value for unsigned=false was not preserved")
	}
	for _, prefix := range []string{"/mcauth/users/profiles/minecraft/", "/mcauth/api/users/profiles/minecraft/"} {
		name := readBody[auth.Profile](t, request(t, f, "GET", prefix+"MigrationHTTP", nil, 200))
		if name.ID != login.SelectedProfile.ID || len(name.Properties) != 0 {
			t.Fatal("name API response differs")
		}
	}
	request(t, f, "GET", "/mcauth/users/profiles/minecraft/UnknownHTTPPlayer", nil, 404)
	request(t, f, "GET", "/mcauth/sessionserver/session/minecraft/profile/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", nil, 204)
	profiles := readBody[[]auth.Profile](t, request(t, f, "POST", "/mcauth/api/profiles/minecraft", []string{"MigrationHTTP", "UnknownHTTPPlayer", "migrationhttp"}, 200))
	if len(profiles) != 2 || profiles[1].Name != "migrationhttp" {
		t.Fatal("batch order or original name spelling was changed")
	}
	request(t, f, "POST", "/mcauth/sessionserver/session/minecraft/join", map[string]any{"serverId": "go-http-main", "accessToken": login.AccessToken, "selectedProfile": "wrong"}, 403)
	request(t, f, "POST", "/mcauth/sessionserver/session/minecraft/join", map[string]any{"serverId": "go-http-main", "accessToken": login.AccessToken, "selectedProfile": login.SelectedProfile.ID}, 204)
	joined := readBody[auth.Profile](t, request(t, f, "GET", "/mcauth/sessionserver/session/minecraft/hasJoined?username=MigrationHTTP&serverId=go-http-main&ip=203.0.113.99", nil, 200))
	verifySignedProfile(t, f.cfg, joined)
	request(t, f, "GET", "/mcauth/sessionserver/session/minecraft/hasJoined?username=MigrationHTTP&serverId=go-http-missing", nil, 204)
	if err := f.store.RecordJoin(context.Background(), "go-http-expired", login.AccessToken, "MigrationHTTP", "127.0.0.1", 0); err != nil {
		t.Fatal(err)
	}
	request(t, f, "GET", "/mcauth/sessionserver/session/minecraft/hasJoined?username=MigrationHTTP&serverId=go-http-expired", nil, 200)
	request(t, f, "GET", "/mcauth/sessionserver/session/minecraft/hasJoined?username=MigrationHTTP&serverId=go-http-expired", nil, 204)
	refreshBody := tokenRequest(login)
	refreshBody["requestUser"] = "false"
	refreshed := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/refresh", refreshBody, 200))
	if refreshed.User == nil || len(refreshed.AvailableProfiles) != 0 || len(refreshed.SelectedProfile.Properties) != 1 {
		t.Fatal("refresh response fields differ")
	}
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(login), 403)
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(refreshed), 204)
	request(t, f, "POST", "/mcauth/authserver/invalidate", tokenRequest(refreshed), 204)
	request(t, f, "POST", "/mcauth/authserver/invalidate", tokenRequest(refreshed), 204)
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(refreshed), 403)
	withoutClient := loginRequest("MigrationHTTP", "", false)
	delete(withoutClient, "clientToken")
	newLogin := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", withoutClient, 200))
	if len(newLogin.ClientToken) != 32 || newLogin.SelectedProfile.ID != login.SelectedProfile.ID {
		t.Fatal("missing client token generation or identity persistence failed")
	}
	request(t, f, "POST", "/mcauth/authserver/signout", map[string]any{"username": "MigrationHTTP", "password": "wrong"}, 403)
	request(t, f, "POST", "/mcauth/authserver/signout", map[string]any{"username": "MigrationHTTP", "password": "fixture-password"}, 204)
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(newLogin), 403)
	chinese := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("迁移用户", "go-http-chinese", true), 200))
	queried := readBody[auth.Profile](t, request(t, f, "GET", "/mcauth/users/profiles/minecraft/"+url.QueryEscape("迁移用户"), nil, 200))
	if queried.ID != chinese.SelectedProfile.ID || queried.Name != "迁移用户" {
		t.Fatal("Unicode name round trip failed")
	}
}

func verifyTokenWithLegacyPHP(t *testing.T, login auth.Login) {
	t.Helper()
	php := os.Getenv("AIHMC_PHP_EXE")
	legacy := os.Getenv("AIHMC_LEGACY_ROOT")
	if php == "" || legacy == "" {
		t.Log("optional Go-to-PHP token check not run; provide AIHMC_PHP_EXE and isolated AIHMC_LEGACY_ROOT")
		return
	}
	script := `<?php
$_SERVER['DOCUMENT_ROOT'] = $argv[1];
require $_SERVER['DOCUMENT_ROOT'] . '/includes/functions.php';
$tokens = json_decode(stream_get_contents(STDIN), true);
validate_token($tokens['accessToken'], $tokens['clientToken']);
if (http_response_code() !== 204) { exit(3); }
echo "PASS original PHP accepted the Go-issued token\n";
`
	path := filepath.Join(t.TempDir(), "verify-token.php")
	if err := os.WriteFile(path, []byte(script), 0600); err != nil {
		t.Fatal(err)
	}
	command := exec.Command(php, "-n", "-d", "extension="+filepath.Join(filepath.Dir(php), "ext/php_mysqli.dll"), "-d", "display_errors=0", "-f", path, legacy)
	data, err := json.Marshal(tokenRequest(login))
	if err != nil {
		t.Fatal(err)
	}
	command.Stdin = bytes.NewReader(data)
	output, err := command.CombinedOutput()
	if err != nil || !strings.Contains(string(output), "PASS original PHP accepted") {
		t.Fatalf("Go-to-PHP compatibility failed: %v: %s", err, output)
	}
	t.Log(strings.TrimSpace(string(output)))
}

func TestPHPTokenAcceptedByGoAndReopenedService(t *testing.T) {
	f := integrationFixture(t)
	data, err := os.ReadFile(filepath.Join(testutil.Root(t), "docs/legacy-http-baseline.json"))
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Case string
		Body json.RawMessage
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	var legacy auth.Login
	for _, item := range cases {
		if item.Case == "login_new" {
			legacy = readBody[auth.Login](t, item.Body)
		}
	}
	if legacy.AccessToken == "" {
		t.Fatal("missing actual PHP-issued reference token")
	}
	if err := f.store.BindToken(context.Background(), store.Token{UID: 920001, Name: "MigrationHTTP", AccessToken: legacy.AccessToken, ClientToken: legacy.ClientToken, CreatedAt: 1}); err != nil {
		t.Fatal(err)
	}
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(legacy), 204)
	// A new handler and store use the same persisted database, with no in-memory sessions.
	reopened := store.New(f.db, time.Second)
	service := auth.NewService(f.cfg, reopened, ucenter.New(f.cfg), fixtureMaterials{})
	server := httptest.NewServer(httpapi.New(service, texture.NewService(f.cfg, reopened), slog.New(slog.NewTextHandler(io.Discard, nil))))
	defer server.Close()
	f.server = server
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(legacy), 204)
}

func TestConcurrentFirstLoginKeepsOneIdentity(t *testing.T) {
	f := integrationFixture(t)
	var group sync.WaitGroup
	errors := make(chan error, 8)
	ids := make(chan string, 8)
	for iteration := 0; iteration < 8; iteration++ {
		group.Add(1)
		go func() {
			defer group.Done()
			body, _ := json.Marshal(loginRequest("ConcurrentFirst", "go-http-concurrent", false))
			response, err := f.server.Client().Post(f.server.URL+"/mcauth/authserver/authenticate", "application/json", bytes.NewReader(body))
			if err != nil {
				errors <- err
				return
			}
			defer response.Body.Close()
			if response.StatusCode != 200 {
				errors <- fmt.Errorf("concurrent login returned HTTP %d", response.StatusCode)
				return
			}
			var login auth.Login
			if err := json.NewDecoder(response.Body).Decode(&login); err != nil || login.SelectedProfile == nil {
				errors <- fmt.Errorf("invalid concurrent response: %v", err)
				return
			}
			ids <- login.SelectedProfile.ID
		}()
	}
	group.Wait()
	close(errors)
	close(ids)
	for err := range errors {
		t.Error(err)
	}
	identity := ""
	for id := range ids {
		if identity != "" && identity != id {
			t.Error("concurrent logins produced different identities")
		}
		identity = id
	}
	if identity == "" {
		t.Fatal("no concurrent login succeeded")
	}
	for _, table := range []string{"mcauth_profiles", "mcauth_tokens", "mcauth_textures"} {
		var count int
		if err := f.db.QueryRow("SELECT COUNT(*) FROM " + table + " WHERE uid = 920003").Scan(&count); err != nil || count != 1 {
			t.Error("concurrent login did not leave exactly one row in", table)
		}
	}
}

func TestConcurrentRefreshRejectsTheConsumedToken(t *testing.T) {
	f := integrationFixture(t)
	login := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("MigrationHTTP", "go-http-refresh", false), 200))
	var group sync.WaitGroup
	statuses := make(chan int, 2)
	failures := make(chan error, 2)
	start := make(chan struct{})
	for iteration := 0; iteration < 2; iteration++ {
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			body, _ := json.Marshal(tokenRequest(login))
			response, err := f.server.Client().Post(f.server.URL+"/mcauth/authserver/refresh", "application/json", bytes.NewReader(body))
			if err != nil {
				failures <- err
				return
			}
			defer response.Body.Close()
			io.Copy(io.Discard, response.Body)
			statuses <- response.StatusCode
		}()
	}
	close(start)
	group.Wait()
	close(failures)
	close(statuses)
	for err := range failures {
		t.Error(err)
	}
	counts := map[int]int{}
	for status := range statuses {
		counts[status]++
	}
	if counts[200] != 1 || counts[403] != 1 {
		t.Fatalf("concurrent refresh returned unexpected statuses: %v", counts)
	}
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(login), 403)
}

func TestAvatarAndAdminDeletionHTTP(t *testing.T) {
	f := integrationFixture(t)
	login := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("MigrationHTTP", "go-http-delete", true), 200))
	other := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("迁移用户", "go-http-other", true), 200))
	request(t, f, "POST", "/mcauth/sessionserver/session/minecraft/join", map[string]any{
		"serverId": "go-http-delete", "accessToken": login.AccessToken, "selectedProfile": login.SelectedProfile.ID,
	}, 204)
	avatar := request(t, f, "GET", "/helm/"+login.SelectedProfile.ID+"/20", nil, 200)
	decoded, err := texture.DecodePNG(avatar)
	if err != nil || decoded.Bounds().Dx() != 20 || decoded.Bounds().Dy() != 20 {
		t.Fatal("local avatar endpoint did not return the requested PNG size", err)
	}
	remote := request(t, f, "GET", "/helm/unknown-avatar-uuid/20", nil, 200)
	decoded, err = texture.DecodePNG(remote)
	if err != nil || decoded.Bounds().Dx() != 20 {
		t.Fatal("external avatar fallback failed", err)
	}
	request(t, f, "POST", "/helm/"+login.SelectedProfile.ID+"/20", map[string]any{}, 405)
	request(t, f, "GET", "/helm/"+login.SelectedProfile.ID+"/0", nil, 400)
	request(t, f, "GET", "/helm/"+login.SelectedProfile.ID, nil, 400)
	request(t, f, "GET", "/system/deluser", nil, 405)
	missing := readBody[map[string]string](t, request(t, f, "POST", "/system/deluser", map[string]any{"super_token": "", "prarms": map[string]any{"uid": 920001}}, 400))
	if missing["msg"] != "超级操作密钥或用户UID不能为空" {
		t.Fatal("admin missing-field response differs")
	}
	denied := readBody[map[string]string](t, request(t, f, "POST", "/system/deluser", map[string]any{"super_token": "wrong", "prarms": map[string]any{"uid": 920001}}, 403))
	if denied["msg"] != "超级操作密钥错误" {
		t.Fatal("admin authorization response differs")
	}
	request(t, f, "POST", "/system/deluser", map[string]any{"super_token": f.cfg.SuperKey(), "params": map[string]any{"uid": 920001}}, 400)
	request(t, f, "POST", "/system/deluser", map[string]any{"super_token": f.cfg.SuperKey(), "prarms": map[string]any{"uid": 999999999}}, 404)
	success := readBody[map[string]string](t, request(t, f, "POST", "/system/deluser", map[string]any{"super_token": f.cfg.SuperKey(), "prarms": map[string]any{"uid": "920001"}}, 200))
	if success["msg"] != "删除成功" {
		t.Fatal("admin success response differs")
	}
	for _, table := range []string{"mcauth_tokens", "mcauth_profiles", "mcauth_textures"} {
		var count int
		if err := f.db.QueryRow("SELECT COUNT(*) FROM " + table + " WHERE uid = 920001").Scan(&count); err != nil || count != 0 {
			t.Error("deleted user remains in", table)
		}
	}
	joined, err := f.store.HasJoined(context.Background(), "MigrationHTTP", "go-http-delete")
	if err != nil || joined {
		t.Fatal("deleted user's join record remains")
	}
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(login), 403)
	request(t, f, "POST", "/mcauth/authserver/validate", tokenRequest(other), 204)
	request(t, f, "GET", "/mcauth/users/profiles/minecraft/MigrationHTTP", nil, 404)
}

func TestSameHostGatewayPreservesJoinClientIP(t *testing.T) {
	f := integrationFixture(t)
	login := readBody[auth.Login](t, request(t, f, "POST", "/mcauth/authserver/authenticate", loginRequest("MigrationHTTP", "go-http-proxy", false), 200))
	data, err := json.Marshal(map[string]string{
		"serverId": "go-http-proxy", "accessToken": login.AccessToken, "selectedProfile": login.SelectedProfile.ID,
	})
	if err != nil {
		t.Fatal(err)
	}
	r, err := http.NewRequest("POST", f.server.URL+"/mcauth/sessionserver/session/minecraft/join", bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	r.Header.Set("X-Real-IP", "198.51.100.9")
	response, err := f.server.Client().Do(r)
	if err != nil {
		t.Fatal(err)
	}
	response.Body.Close()
	if response.StatusCode != 204 {
		t.Fatal("gateway join failed", response.StatusCode)
	}
	var recorded string
	if err := f.db.QueryRow("SELECT ip FROM mcauth_logins WHERE server_id = 'go-http-proxy'").Scan(&recorded); err != nil {
		t.Fatal(err)
	}
	if recorded != "198.51.100.9" {
		t.Fatalf("same-host gateway client IP was lost: recorded %s", recorded)
	}
}
