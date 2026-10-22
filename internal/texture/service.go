// Package texture resolves, stores and renders the existing Minecraft materials.
package texture

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	"github.com/AIH-MC/aihmc-service/internal/auth"
	"github.com/AIH-MC/aihmc-service/internal/config"
	"github.com/AIH-MC/aihmc-service/internal/store"
)

type Service struct {
	cfg   *config.Config
	store Repository
	http  *http.Client
	files sync.Mutex
}

type Repository interface {
	SkinAPIs(context.Context) ([]string, error)
	ProfileByUUID(context.Context, string) (store.Profile, error)
}

func NewService(cfg *config.Config, database Repository) *Service {
	return &Service{
		cfg: cfg, store: database,
		http: &http.Client{
			Timeout: cfg.Textures.RequestTimeout.Value(),
			CheckRedirect: func(_ *http.Request, _ []*http.Request) error {
				return http.ErrUseLastResponse
			},
		},
	}
}

func (s *Service) fetch(ctx context.Context, address string) ([]byte, bool) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, address, nil)
	if err != nil {
		return nil, false
	}
	response, err := s.http.Do(request)
	if err != nil {
		return nil, false
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, false
	}
	data, err := io.ReadAll(response.Body)
	return data, err == nil
}

func (s *Service) Resolve(parent context.Context, name string) (auth.Material, error) {
	ctx, cancel := context.WithTimeout(parent, s.cfg.Textures.RequestTimeout.Value())
	defer cancel()
	apis, err := s.store.SkinAPIs(ctx)
	if err != nil {
		return auth.Material{}, err
	}
	var material auth.Material
	for _, api := range apis {
		data, found := s.fetch(ctx, api+"/csl/"+url.QueryEscape(name)+".json")
		if !found {
			continue
		}
		var csl struct {
			Skins map[string]*string `json:"skins"`
			Cape  string             `json:"cape"`
		}
		if err := json.Unmarshal(data, &csl); err != nil {
			continue
		}
		material.Model = "default"
		if csl.Skins["slim"] != nil {
			material.Model = "slim"
		}
		if skin := csl.Skins[material.Model]; skin != nil && *skin != "" {
			found, err := s.downloadNamed(ctx, api, *skin)
			if err != nil {
				return auth.Material{}, err
			}
			if found {
				material.Skin = *skin
			}
		}
		if csl.Cape != "" {
			found, err := s.downloadNamed(ctx, api, csl.Cape)
			if err != nil {
				return auth.Material{}, err
			}
			if found {
				material.Cape = csl.Cape
			}
		}
		if material.Skin != "" || material.Cape != "" {
			break
		}
	}
	if material.Skin == "" || material.Cape == "" {
		if err := s.fallback(ctx, name, &material); err != nil {
			return auth.Material{}, err
		}
	}
	if err := parent.Err(); err != nil {
		return auth.Material{}, err
	}
	// Material sources are optional. Missing remote data does not reject valid credentials.
	return material, nil
}

func validKey(key string) bool {
	return key != "" && key != "." && key != ".." && filepath.IsLocal(key) && !strings.ContainsAny(key, "/\\\x00")
}

func (s *Service) downloadNamed(ctx context.Context, api, key string) (bool, error) {
	if !validKey(key) {
		return false, nil
	}
	data, found := s.fetch(ctx, api+"/textures/"+url.PathEscape(key))
	if !found {
		return false, nil
	}
	if _, err := DecodePNG(data); err != nil {
		return false, nil
	}
	return true, s.save(key, data)
}

func (s *Service) save(key string, data []byte) error {
	if !validKey(key) {
		return errors.New("invalid texture filename")
	}
	file, err := os.CreateTemp(s.cfg.Textures.Directory, ".texture-*")
	if err != nil {
		return errors.New("cannot create texture file")
	}
	name := file.Name()
	defer os.Remove(name)
	if _, err := file.Write(data); err != nil {
		file.Close()
		return errors.New("cannot write texture file")
	}
	if err := file.Chmod(0644); err != nil {
		file.Close()
		return errors.New("cannot set public texture file permissions")
	}
	if err := file.Close(); err != nil {
		return errors.New("cannot close texture file")
	}
	// Concurrent requests share material keys. Publish replacements and read them
	// under the same lock, while network downloads and temporary writes stay parallel.
	s.files.Lock()
	err = os.Rename(name, filepath.Join(s.cfg.Textures.Directory, key))
	s.files.Unlock()
	if err != nil {
		return errors.New("cannot publish texture file")
	}
	return nil
}

func (s *Service) fallback(ctx context.Context, name string, material *auth.Material) error {
	data, found := s.fetch(ctx, strings.TrimRight(s.cfg.Textures.MojangBaseURL, "/")+"/users/profiles/minecraft/"+url.PathEscape(name))
	if !found {
		return nil
	}
	var profile struct {
		ID string `json:"id"`
	}
	if err := json.Unmarshal(data, &profile); err != nil || profile.ID == "" {
		return nil
	}
	base := strings.TrimRight(s.cfg.Textures.CrafatarBaseURL, "/")
	for _, item := range []struct {
		path string
		key  *string
	}{
		{"/skins/", &material.Skin},
		{"/capes/", &material.Cape},
	} {
		if *item.key != "" {
			continue
		}
		data, found := s.fetch(ctx, base+item.path+url.PathEscape(profile.ID))
		if !found {
			continue
		}
		image, err := DecodePNG(data)
		if err != nil {
			continue
		}
		digest := sha256.Sum256(data)
		key := hex.EncodeToString(digest[:])
		if err := s.save(key, data); err != nil {
			return err
		}
		*item.key = key
		if item.path == "/skins/" {
			material.Model = ModelType(image)
		}
	}
	return nil
}

func (s *Service) Avatar(ctx context.Context, uuid string, size int) ([]byte, error) {
	if size <= 0 {
		return nil, &auth.APIError{Status: 400, Name: "IllegalArgumentException", Message: "Avatar size must be a positive integer."}
	}
	profile, err := s.store.ProfileByUUID(ctx, uuid)
	if errors.Is(err, sql.ErrNoRows) {
		address := strings.TrimRight(s.cfg.Textures.CraftheadBaseURL, "/") + "/helm/" + url.PathEscape(uuid) + "/" + strconv.Itoa(size)
		data, found := s.fetch(ctx, address)
		if !found {
			return nil, errors.New("avatar fallback unavailable")
		}
		if _, err := DecodePNG(data); err != nil {
			return nil, errors.New("avatar fallback returned invalid PNG")
		}
		return data, nil
	}
	if err != nil {
		return nil, err
	}
	material, err := s.Resolve(ctx, profile.Name)
	if err != nil {
		return nil, err
	}
	if !validKey(material.Skin) {
		return nil, errors.New("no skin available for local avatar")
	}
	s.files.Lock()
	data, err := os.ReadFile(filepath.Join(s.cfg.Textures.Directory, material.Skin))
	s.files.Unlock()
	if err != nil {
		return nil, errors.New("cannot read local avatar skin")
	}
	image, err := DecodePNG(data)
	if err != nil {
		return nil, errors.New("local avatar skin is not PNG")
	}
	return Avatar(image, size)
}
