package store

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"testing"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/testutil"
)

func integrationStore(t *testing.T) *Store {
	t.Helper()
	path := os.Getenv("AIHMC_TEST_CONFIG")
	if path == "" {
		t.Skip("set AIHMC_TEST_CONFIG to run against the existing database instance")
	}
	cfg, err := config.Load(path)
	if err != nil {
		t.Fatal(err)
	}
	db := testutil.Database(t, cfg, Open)
	clean := func() {
		if _, err := db.Exec("DELETE FROM texture_apis WHERE id IN (950001, 950002)"); err != nil {
			t.Error(err)
		}
		if _, err := db.Exec("DELETE FROM mcauth_logins WHERE server_id = ?", "go-migration-store-test"); err != nil {
			t.Error(err)
		}
		for _, table := range []string{"mcauth_tokens", "mcauth_profiles", "mcauth_textures"} {
			if _, err := db.Exec("DELETE FROM " + table + " WHERE uid IN (910001, 910002, 910003)"); err != nil {
				t.Error(err)
			}
		}
	}
	clean()
	t.Cleanup(clean)
	return New(db, 5*time.Second)
}

func TestSkinAPIPriorityUsesLegacyRecordOrder(t *testing.T) {
	s := integrationStore(t)
	ctx := context.Background()
	first := "http://z-go-migration.invalid"
	second := "http://a-go-migration.invalid"
	if _, err := s.db.Exec("INSERT INTO texture_apis(id, api_url, added_by, added_date) VALUES(950001, ?, 'fixture', 0), (950002, ?, 'fixture', 0)", first, second); err != nil {
		t.Fatal(err)
	}
	apis, err := s.SkinAPIs(ctx)
	if err != nil {
		t.Fatal(err)
	}
	positions := map[string]int{}
	for index, address := range apis {
		positions[address] = index
	}
	firstIndex, firstFound := positions[first]
	secondIndex, secondFound := positions[second]
	if !firstFound || !secondFound || firstIndex >= secondIndex {
		t.Fatal("source priority follows the URL index instead of legacy record order")
	}
}

func seedAccount(ctx context.Context, q *Queries, uid int64, name, uuid, access, client string) error {
	if err := q.SaveProfile(ctx, Profile{UID: uid, Name: name, ProfileID: "profile-" + uuid, UUID: uuid, TextureData: "e30="}); err != nil {
		return err
	}
	if err := q.SaveTexture(ctx, Texture{UID: uid, UUID: uuid, Name: name, Skin: "fixture-skin", Model: "slim", Timestamp: 1700000000}); err != nil {
		return err
	}
	return q.BindToken(ctx, Token{UID: uid, Name: name, AccessToken: access, ClientToken: client, CreatedAt: 1700000000})
}

func TestExistingDatabaseIdentityBindingAndParameterizedQueries(t *testing.T) {
	s := integrationStore(t)
	ctx := context.Background()
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		return seedAccount(ctx, q, 910001, "迁移测试", "11111111111141118111111111111112", "store-access", "store-client")
	}); err != nil {
		t.Fatal(err)
	}
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		if err := q.BindToken(ctx, Token{UID: 910001, Name: "ignored-name", AccessToken: "ignored-new-token", ClientToken: "new-client", CreatedAt: 1700000001}); err != nil {
			return err
		}
		return q.SaveProfile(ctx, Profile{UID: 910001, Name: "ignored-name", UUID: "ignored-uuid", TextureData: "bmV3"})
	}); err != nil {
		t.Fatal(err)
	}
	token, err := s.Token(ctx, "store-access", "new-client")
	if err != nil || token.Name != "迁移测试" || token.CreatedAt != 1700000001 {
		t.Fatal("existing token was not retained during rebinding")
	}
	profile, err := s.ProfileByUID(ctx, 910001)
	if err != nil || profile.Name != "迁移测试" || profile.UUID != "11111111111141118111111111111112" || profile.TextureData != "bmV3" {
		t.Fatal("stored identity changed during profile update")
	}
	if _, err := s.ProfileByName(ctx, "' OR '1'='1"); !errors.Is(err, sql.ErrNoRows) {
		t.Fatal("SQL input was not treated as a parameter")
	}
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		return q.SaveProfile(ctx, Profile{UID: 910002, Name: "迁移测试", UUID: "different-uuid", TextureData: "must-not-overwrite"})
	}); err == nil {
		t.Fatal("conflicting identity insertion unexpectedly succeeded")
	}
	profile, err = s.ProfileByUID(ctx, 910001)
	if err != nil || profile.TextureData != "bmV3" {
		t.Fatal("unique-key conflict mutated a different user's record")
	}
}

func TestDatabaseTransactionRollbackAndDeletion(t *testing.T) {
	s := integrationStore(t)
	ctx := context.Background()
	failure := errors.New("fixture failure")
	err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		if err := seedAccount(ctx, q, 910001, "RollbackPlayer", "11111111111141118111111111111112", "rollback-access", "rollback-client"); err != nil {
			return err
		}
		return failure
	})
	if !errors.Is(err, failure) {
		t.Fatal("transaction did not return the original failure")
	}
	if _, err := s.ProfileByUID(ctx, 910001); !errors.Is(err, sql.ErrNoRows) {
		t.Fatal("failed transaction retained a profile")
	}
	if _, err := s.TokenByUID(ctx, 910001); !errors.Is(err, sql.ErrNoRows) {
		t.Fatal("failed transaction retained a token")
	}
	if _, err := s.TextureByUID(ctx, 910001); !errors.Is(err, sql.ErrNoRows) {
		t.Fatal("failed transaction retained a texture")
	}
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		if err := seedAccount(ctx, q, 910001, "RollbackPlayer", "11111111111141118111111111111112", "rollback-access", "rollback-client"); err != nil {
			return err
		}
		return q.RecordJoin(ctx, "go-migration-store-test", "rollback-access", "RollbackPlayer", "127.0.0.1", time.Now().Unix())
	}); err != nil {
		t.Fatal(err)
	}
	err = s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		if ok, err := q.DeleteUser(ctx, 910001); err != nil || !ok {
			t.Fatal("fixture delete failed", err)
		}
		return failure
	})
	if !errors.Is(err, failure) {
		t.Fatal(err)
	}
	if joined, err := s.HasJoined(ctx, "RollbackPlayer", "go-migration-store-test"); err != nil || !joined {
		t.Fatal("deletion rollback did not restore the join record")
	}
	if _, err := s.TokenByUID(ctx, 910001); err != nil {
		t.Fatal("deletion rollback did not restore token")
	}
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		_, err := q.DeleteUser(ctx, 910001)
		return err
	}); err != nil {
		t.Fatal(err)
	}
	for _, read := range []func() error{
		func() error { _, err := s.ProfileByUID(ctx, 910001); return err },
		func() error { _, err := s.TokenByUID(ctx, 910001); return err },
		func() error { _, err := s.TextureByUID(ctx, 910001); return err },
	} {
		if !errors.Is(read(), sql.ErrNoRows) {
			t.Fatal("successful deletion retained user data")
		}
	}
	if joined, err := s.HasJoined(ctx, "RollbackPlayer", "go-migration-store-test"); err != nil || joined {
		t.Fatal("successful deletion retained the join record")
	}
}

func TestSharedClientRefreshFailsAtomicallyUnderExistingUniqueConstraint(t *testing.T) {
	s := integrationStore(t)
	ctx := context.Background()
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		if err := seedAccount(ctx, q, 910001, "SharedClientOne", "11111111111141118111111111111112", "shared-access-one", "shared-client"); err != nil {
			return err
		}
		return seedAccount(ctx, q, 910002, "SharedClientTwo", "11111111111141118111111111111113", "shared-access-two", "shared-client")
	}); err != nil {
		t.Fatal(err)
	}
	if err := s.Transaction(ctx, func(ctx context.Context, q *Queries) error {
		_, err := q.Refresh(ctx, "shared-client", "new-shared-access")
		return err
	}); err == nil {
		t.Fatal("ambiguous refresh should fail under the existing unique access_token constraint")
	}
	for _, access := range []string{"shared-access-one", "shared-access-two"} {
		if _, err := s.Token(ctx, access, "shared-client"); err != nil {
			t.Fatal("failed refresh changed an existing token")
		}
	}
	if _, err := s.SkinDomains(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := s.SkinAPIs(ctx); err != nil {
		t.Fatal(err)
	}
}
