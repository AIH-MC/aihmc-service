package auth

import (
	"context"
	"database/sql"
	"errors"
	"strings"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/store"
	"github.com/AIH-MC/aihmc-service/internal/ucenter"
	"github.com/go-sql-driver/mysql"
)

type APIError struct {
	Status  int
	Name    string
	Message string
}

func (e *APIError) Error() string { return e.Name }

func InvalidToken() error {
	return &APIError{Status: 403, Name: "ForbiddenOperationException", Message: "Invalid token."}
}

func credentialsError() error {
	return &APIError{Status: 403, Name: "ForbiddenOperationException", Message: "Invalid credentials. Invalid username or password."}
}

type Material struct {
	Skin  string
	Model string
	Cape  string
}

type MaterialProvider interface {
	Resolve(context.Context, string) (Material, error)
}

type UserProvider interface {
	Login(context.Context, string, string) (ucenter.User, error)
}

type Service struct {
	cfg       *config.Config
	store     *store.Store
	users     UserProvider
	materials MaterialProvider
}

func NewService(cfg *config.Config, database *store.Store, users UserProvider, materials MaterialProvider) *Service {
	return &Service{cfg: cfg, store: database, users: users, materials: materials}
}

func (s *Service) Authenticate(ctx context.Context, account, password, client, agent string, requestUser bool) (Login, error) {
	if account == "" || password == "" {
		return Login{}, &APIError{Status: 400, Name: "HTTP/1.1 400 Bad Request", Message: "账号或密码不能为空"}
	}
	if len(agent) != len("minecraft") || strings.ToLower(agent) != "minecraft" {
		return Login{}, &APIError{Status: 403, Name: "HTTP/1.1 403 Forbidden", Message: "非法请求"}
	}
	user, err := s.users.Login(ctx, account, password)
	if errors.Is(err, ucenter.ErrInvalidCredentials) {
		return Login{}, credentialsError()
	}
	if err != nil {
		return Login{}, err
	}
	material, err := s.materials.Resolve(ctx, user.Name)
	if err != nil {
		return Login{}, err
	}
	if client == "" {
		client, err = ClientToken()
		if err != nil {
			return Login{}, err
		}
	}
	now := time.Now().Unix()
	var response Login
	ctx, cancel := context.WithTimeout(ctx, s.cfg.Database.QueryTimeout.Value())
	defer cancel()
	// Concurrent first logins can deadlock on InnoDB's missing-UID gap locks.
	// Retry only that transaction; upstream requests have already completed.
	for attempt := 0; attempt < 3; attempt++ {
		err = s.store.Transaction(ctx, func(ctx context.Context, q *store.Queries) error {
			profile, err := q.LockProfile(ctx, user.UID)
			if err != nil && !errors.Is(err, sql.ErrNoRows) {
				return err
			}
			if errors.Is(err, sql.ErrNoRows) {
				uuid, err := NewUUID()
				if err != nil {
					return err
				}
				profile = store.Profile{UID: user.UID, Name: user.Name, UUID: uuid, ProfileID: ProfileID(user.Name, user.Email, user.UID)}
			}
			if err := q.BindToken(ctx, store.Token{
				UID: user.UID, Name: user.Name, AccessToken: AccessToken(user.Name, client, now), ClientToken: client, CreatedAt: now,
			}); err != nil {
				return err
			}
			if err := q.SaveTexture(ctx, store.Texture{
				UID: user.UID, UUID: profile.UUID, Name: user.Name, Skin: material.Skin, Model: material.Model, Cape: material.Cape, Timestamp: now,
			}); err != nil {
				return err
			}
			texture, err := q.TextureByUID(ctx, user.UID)
			if err != nil {
				return err
			}
			profile.TextureData, err = EncodeTexture(texture.Timestamp, texture.UUID, texture.Name, texture.Skin, texture.Model, texture.Cape, s.cfg.Textures.PublicBaseURL)
			if err != nil {
				return err
			}
			if err := q.SaveProfile(ctx, profile); err != nil {
				return err
			}
			token, err := q.TokenByUID(ctx, user.UID)
			if err != nil {
				return err
			}
			selected := Profile{ID: profile.UUID, Name: profile.Name}
			response = Login{AccessToken: token.AccessToken, ClientToken: token.ClientToken, AvailableProfiles: []Profile{selected}, SelectedProfile: &selected}
			if requestUser {
				response.User = &User{ID: profile.ProfileID, Properties: []Property{}}
			}
			return nil
		})
		var databaseError *mysql.MySQLError
		if !errors.As(err, &databaseError) || databaseError.Number != 1213 {
			break
		}
	}
	return response, err
}

func (s *Service) Refresh(ctx context.Context, access, client string, requestUser bool) (Login, error) {
	var response Login
	err := s.store.Transaction(ctx, func(ctx context.Context, q *store.Queries) error {
		token, err := q.Token(ctx, access, client)
		if errors.Is(err, sql.ErrNoRows) {
			return InvalidToken()
		}
		if err != nil {
			return err
		}
		profile, err := q.LockProfile(ctx, token.UID)
		if err != nil {
			return err
		}
		if _, err := q.LockToken(ctx, access, client); errors.Is(err, sql.ErrNoRows) {
			return InvalidToken()
		} else if err != nil {
			return err
		}
		token, err = q.Refresh(ctx, client, RefreshedToken(client, time.Now().Unix()))
		if err != nil {
			return err
		}
		// The legacy update scope is by clientToken. A successful update has one row
		// because access_token is unique; profile identity remains tied to that row.
		if token.UID != profile.UID {
			profile, err = q.ProfileByUID(ctx, token.UID)
			if err != nil {
				return err
			}
		}
		selected := Profile{ID: profile.UUID, Name: profile.Name, Properties: []Property{{Name: "textures", Value: profile.TextureData}}}
		response = Login{AccessToken: token.AccessToken, ClientToken: token.ClientToken, SelectedProfile: &selected}
		if requestUser {
			response.User = &User{ID: profile.ProfileID, Properties: []Property{}}
		}
		return nil
	})
	return response, err
}

func (s *Service) Validate(ctx context.Context, access, client string) error {
	_, err := s.store.Token(ctx, access, client)
	if errors.Is(err, sql.ErrNoRows) {
		return InvalidToken()
	}
	return err
}

func (s *Service) Invalidate(ctx context.Context, access, client string) error {
	return s.store.Invalidate(ctx, access, client)
}

func (s *Service) Signout(ctx context.Context, account, password string) error {
	user, err := s.users.Login(ctx, account, password)
	if errors.Is(err, ucenter.ErrInvalidCredentials) {
		return credentialsError()
	}
	if err != nil {
		return err
	}
	return s.store.Signout(ctx, user.UID)
}

func (s *Service) Profile(ctx context.Context, uuidOrName, unsigned string) (*Profile, error) {
	profile, err := s.store.Profile(ctx, uuidOrName)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	property := Property{Name: "textures", Value: profile.TextureData}
	if !PHPTruthyQuery(unsigned) {
		property.Signature, err = Sign(profile.TextureData, s.cfg.PrivateKey())
		if err != nil {
			return nil, err
		}
	}
	return &Profile{ID: profile.UUID, Name: profile.Name, Properties: []Property{property}}, nil
}

func (s *Service) Name(ctx context.Context, name string) (*Profile, error) {
	profile, err := s.store.Profile(ctx, name)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, &APIError{Status: 404, Name: "UserNotFound", Message: "The requested user " + name + " could not be found"}
	}
	if err != nil {
		return nil, err
	}
	return &Profile{ID: profile.UUID, Name: profile.Name}, nil
}

func (s *Service) Profiles(ctx context.Context, names []string) ([]Profile, error) {
	profiles := []Profile{}
	for _, name := range names {
		profile, err := s.store.ProfileByName(ctx, name)
		if errors.Is(err, sql.ErrNoRows) {
			continue
		}
		if err != nil {
			return nil, err
		}
		profiles = append(profiles, Profile{ID: profile.UUID, Name: name})
	}
	return profiles, nil
}

func (s *Service) Join(ctx context.Context, serverID, access, selected, ip string) error {
	return s.store.Transaction(ctx, func(ctx context.Context, q *store.Queries) error {
		token, err := q.TokenByAccess(ctx, access)
		if errors.Is(err, sql.ErrNoRows) {
			return InvalidToken()
		}
		if err != nil {
			return err
		}
		profile, err := q.ProfileByUID(ctx, token.UID)
		if err != nil {
			return err
		}
		if selected != profile.UUID {
			return InvalidToken()
		}
		return q.RecordJoin(ctx, serverID, access, profile.Name, ip, time.Now().Unix())
	})
}

func (s *Service) HasJoined(ctx context.Context, name, serverID string) (*Profile, error) {
	joined, err := s.store.HasJoined(ctx, name, serverID)
	if err != nil || !joined {
		return nil, err
	}
	profile, err := s.Profile(ctx, name, "")
	if err != nil || profile == nil {
		return profile, err
	}
	before := time.Now().Add(-s.cfg.Auth.LoginRecordRetention.Value()).Unix()
	if err := s.store.DeleteOldJoins(ctx, name, before); err != nil {
		return nil, err
	}
	return profile, nil
}

func (s *Service) Metadata(ctx context.Context) (any, error) {
	domains, err := s.store.SkinDomains(ctx)
	if err != nil {
		return nil, err
	}
	return map[string]any{
		"meta": map[string]any{
			"serverName":              s.cfg.Site.Name,
			"implementationName":      s.cfg.Site.ImplementationName,
			"implementationVersion":   s.cfg.Site.ImplementationVersion,
			"links":                   map[string]string{"homepage": s.cfg.Site.Homepage, "register": s.cfg.Site.RegisterURL},
			"feature.non_email_login": s.cfg.Site.AllowNonEmailLogin,
		},
		"skinDomains":        domains,
		"signaturePublickey": s.cfg.PublicKeyPEM(),
	}, nil
}

func (s *Service) DeleteUser(ctx context.Context, providedKey string, uid int64) (int, string, error) {
	if !PHPTruthyQuery(providedKey) || uid == 0 {
		return 400, "超级操作密钥或用户UID不能为空", nil
	}
	if providedKey != s.cfg.SuperKey() {
		return 403, "超级操作密钥错误", nil
	}
	var deleted bool
	err := s.store.Transaction(ctx, func(ctx context.Context, q *store.Queries) error {
		var err error
		deleted, err = q.DeleteUser(ctx, uid)
		return err
	})
	if err != nil {
		return 500, "", err
	}
	if !deleted {
		return 404, "用户不存在，或未登录过 Minecraft 服务器", nil
	}
	return 200, "删除成功", nil
}
