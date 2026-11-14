package store

import (
	"context"
	"database/sql"
	"errors"
	"time"
)

type queryer interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

type Queries struct {
	db queryer
}

type Store struct {
	*Queries
	db      *sql.DB
	timeout time.Duration
}

func New(db *sql.DB, timeout time.Duration) *Store {
	return &Store{Queries: &Queries{db: db}, db: db, timeout: timeout}
}

func (s *Store) Transaction(ctx context.Context, action func(context.Context, *Queries) error) error {
	ctx, cancel := context.WithTimeout(ctx, s.timeout)
	defer cancel()
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if err := action(ctx, &Queries{db: tx}); err != nil {
		return err
	}
	return tx.Commit()
}

type Profile struct {
	UID         int64
	Name        string
	ProfileID   string
	UUID        string
	TextureData string
}

type Token struct {
	UID         int64
	Name        string
	AccessToken string
	ClientToken string
	CreatedAt   int64
}

type Texture struct {
	UID       int64
	UUID      string
	Name      string
	Skin      string
	Model     string
	Cape      string
	Timestamp int64
}

const profileColumns = "COALESCE(uid,0), COALESCE(player_name,''), COALESCE(profile_id,''), COALESCE(player_uuid,''), COALESCE(texture_data,'')"
const tokenColumns = "uid, player_name, access_token, client_token, create_time"
const textureColumns = "uid, COALESCE(uuid,''), COALESCE(player_name,''), COALESCE(skin_hash,''), COALESCE(skin_type,''), COALESCE(cape_hash,''), COALESCE(timestamps,0)"

func scanProfile(row *sql.Row) (Profile, error) {
	var result Profile
	err := row.Scan(&result.UID, &result.Name, &result.ProfileID, &result.UUID, &result.TextureData)
	return result, err
}

func scanToken(row *sql.Row) (Token, error) {
	var result Token
	err := row.Scan(&result.UID, &result.Name, &result.AccessToken, &result.ClientToken, &result.CreatedAt)
	return result, err
}

func (q *Queries) ProfileByUID(ctx context.Context, uid int64) (Profile, error) {
	return scanProfile(q.db.QueryRowContext(ctx, "SELECT "+profileColumns+" FROM mcauth_profiles WHERE uid = ?", uid))
}

// LockProfile serializes mutations of an existing user's identity within a transaction.
func (q *Queries) LockProfile(ctx context.Context, uid int64) (Profile, error) {
	return scanProfile(q.db.QueryRowContext(ctx, "SELECT "+profileColumns+" FROM mcauth_profiles WHERE uid = ? FOR UPDATE", uid))
}

func (q *Queries) Profile(ctx context.Context, uuidOrName string) (Profile, error) {
	return scanProfile(q.db.QueryRowContext(ctx, "SELECT "+profileColumns+" FROM mcauth_profiles WHERE player_uuid = ? OR player_name = ?", uuidOrName, uuidOrName))
}

func (q *Queries) ProfileByName(ctx context.Context, name string) (Profile, error) {
	return scanProfile(q.db.QueryRowContext(ctx, "SELECT "+profileColumns+" FROM mcauth_profiles WHERE player_name = ?", name))
}

func (q *Queries) ProfileByUUID(ctx context.Context, uuid string) (Profile, error) {
	return scanProfile(q.db.QueryRowContext(ctx, "SELECT "+profileColumns+" FROM mcauth_profiles WHERE player_uuid = ?", uuid))
}

func (q *Queries) TokenByUID(ctx context.Context, uid int64) (Token, error) {
	return scanToken(q.db.QueryRowContext(ctx, "SELECT "+tokenColumns+" FROM mcauth_tokens WHERE uid = ?", uid))
}

func (q *Queries) Token(ctx context.Context, access, client string) (Token, error) {
	return scanToken(q.db.QueryRowContext(ctx, "SELECT "+tokenColumns+" FROM mcauth_tokens WHERE access_token = ? AND client_token = ?", access, client))
}

func (q *Queries) LockToken(ctx context.Context, access, client string) (Token, error) {
	return scanToken(q.db.QueryRowContext(ctx, "SELECT "+tokenColumns+" FROM mcauth_tokens WHERE access_token = ? AND client_token = ? FOR UPDATE", access, client))
}

func (q *Queries) TokenByAccess(ctx context.Context, access string) (Token, error) {
	return scanToken(q.db.QueryRowContext(ctx, "SELECT "+tokenColumns+" FROM mcauth_tokens WHERE access_token = ?", access))
}

func (q *Queries) BindToken(ctx context.Context, token Token) error {
	if _, err := q.TokenByUID(ctx, token.UID); err == nil {
		_, err = q.db.ExecContext(ctx, "UPDATE mcauth_tokens SET client_token = ?, create_time = ? WHERE uid = ?", token.ClientToken, token.CreatedAt, token.UID)
		return err
	} else if !errors.Is(err, sql.ErrNoRows) {
		return err
	}
	_, err := q.db.ExecContext(ctx, `INSERT INTO mcauth_tokens
		(uid, player_name, access_token, client_token, create_time) VALUES (?, ?, ?, ?, ?)`,
		token.UID, token.Name, token.AccessToken, token.ClientToken, token.CreatedAt)
	return err
}

func (q *Queries) SaveProfile(ctx context.Context, profile Profile) error {
	if _, err := q.ProfileByUID(ctx, profile.UID); err == nil {
		_, err = q.db.ExecContext(ctx, "UPDATE mcauth_profiles SET texture_data = ? WHERE uid = ?", profile.TextureData, profile.UID)
		return err
	} else if !errors.Is(err, sql.ErrNoRows) {
		return err
	}
	_, err := q.db.ExecContext(ctx, `INSERT INTO mcauth_profiles
		(uid, player_name, profile_id, player_uuid, texture_data) VALUES (?, ?, ?, ?, ?)`,
		profile.UID, profile.Name, profile.ProfileID, profile.UUID, profile.TextureData)
	return err
}

func (q *Queries) TextureByUID(ctx context.Context, uid int64) (Texture, error) {
	var result Texture
	err := q.db.QueryRowContext(ctx, "SELECT "+textureColumns+" FROM mcauth_textures WHERE uid = ?", uid).
		Scan(&result.UID, &result.UUID, &result.Name, &result.Skin, &result.Model, &result.Cape, &result.Timestamp)
	return result, err
}

func (q *Queries) SaveTexture(ctx context.Context, texture Texture) error {
	if _, err := q.TextureByUID(ctx, texture.UID); err == nil {
		_, err = q.db.ExecContext(ctx, `UPDATE mcauth_textures SET player_name = ?, skin_hash = ?,
			skin_type = ?, cape_hash = ?, timestamps = ? WHERE uid = ?`,
			texture.Name, texture.Skin, texture.Model, texture.Cape, texture.Timestamp, texture.UID)
		return err
	} else if !errors.Is(err, sql.ErrNoRows) {
		return err
	}
	_, err := q.db.ExecContext(ctx, `INSERT INTO mcauth_textures
		(uid, uuid, player_name, skin_hash, skin_type, cape_hash, timestamps) VALUES (?, ?, ?, ?, ?, ?, ?)`,
		texture.UID, texture.UUID, texture.Name, texture.Skin, texture.Model, texture.Cape, texture.Timestamp)
	return err
}

// Refresh retains the legacy clientToken update scope. The existing unique access_token
// constraint continues to reject ambiguous updates when several rows share a clientToken.
func (q *Queries) Refresh(ctx context.Context, client, newAccess string) (Token, error) {
	if _, err := q.db.ExecContext(ctx, "UPDATE mcauth_tokens SET access_token = ? WHERE client_token = ?", newAccess, client); err != nil {
		return Token{}, err
	}
	return q.Token(ctx, newAccess, client)
}

func (q *Queries) Invalidate(ctx context.Context, access, client string) error {
	_, err := q.db.ExecContext(ctx, "DELETE FROM mcauth_tokens WHERE access_token = ? AND client_token = ?", access, client)
	return err
}

func (q *Queries) Signout(ctx context.Context, uid int64) error {
	_, err := q.db.ExecContext(ctx, "DELETE FROM mcauth_tokens WHERE uid = ?", uid)
	return err
}

func (q *Queries) RecordJoin(ctx context.Context, serverID, access, name, ip string, timestamp int64) error {
	_, err := q.db.ExecContext(ctx, `INSERT INTO mcauth_logins
		(server_id, access_token, playername, ip, timestamp) VALUES (?, ?, ?, ?, ?)`, serverID, access, name, ip, timestamp)
	return err
}

func (q *Queries) HasJoined(ctx context.Context, name, serverID string) (bool, error) {
	var exists int
	err := q.db.QueryRowContext(ctx, "SELECT 1 FROM mcauth_logins WHERE playername = ? AND server_id = ?", name, serverID).Scan(&exists)
	if errors.Is(err, sql.ErrNoRows) {
		return false, nil
	}
	return err == nil, err
}

func (q *Queries) DeleteOldJoins(ctx context.Context, name string, before int64) error {
	_, err := q.db.ExecContext(ctx, "DELETE FROM mcauth_logins WHERE timestamp < ? AND playername = ?", before, name)
	return err
}

func (q *Queries) SkinDomains(ctx context.Context) ([]string, error) {
	return q.strings(ctx, "SELECT skin_url FROM mcauth_skin_urls ORDER BY id")
}

func (q *Queries) SkinAPIs(ctx context.Context) ([]string, error) {
	// Selecting only api_url can use its covering index and reverse source priority.
	// The legacy SELECT * was verified to traverse the current table by primary ID.
	return q.strings(ctx, "SELECT api_url FROM texture_apis ORDER BY id")
}

func (q *Queries) strings(ctx context.Context, statement string) ([]string, error) {
	rows, err := q.db.QueryContext(ctx, statement)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := []string{}
	for rows.Next() {
		var value string
		if err := rows.Scan(&value); err != nil {
			return nil, err
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

func (q *Queries) DeleteUser(ctx context.Context, uid int64) (bool, error) {
	if _, err := q.TokenByUID(ctx, uid); errors.Is(err, sql.ErrNoRows) {
		return false, nil
	} else if err != nil {
		return false, err
	}
	texture, err := q.TextureByUID(ctx, uid)
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return false, err
	}
	if _, err := q.db.ExecContext(ctx, "DELETE FROM mcauth_logins WHERE playername = ?", texture.Name); err != nil {
		return false, err
	}
	for _, table := range []string{"mcauth_profiles", "mcauth_tokens", "mcauth_textures"} {
		// Table names are fixed application constants, never request input.
		if _, err := q.db.ExecContext(ctx, "DELETE FROM "+table+" WHERE uid = ?", uid); err != nil {
			return false, err
		}
	}
	return true, nil
}
