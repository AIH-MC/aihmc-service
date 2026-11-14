// Package ucenter adapts the existing AIHMC UCenter HTTP API.
package ucenter

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"net/http"
	"regexp"
	"strconv"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/config"
)

var ErrInvalidCredentials = errors.New("invalid UCenter credentials")
var ErrUpstream = errors.New("UCenter request failed")

var emailPattern = regexp.MustCompile(`^[a-zA-Z0-9_.+\-]+@[a-zA-Z0-9\-]+\.[a-zA-Z0-9\-.]+$`)

type Client struct {
	endpoint string
	appID    string
	nonce    string
	secret   string
	http     *http.Client
}

type User struct {
	UID   int64
	Name  string
	Email string
}

func New(cfg *config.Config) *Client {
	return &Client{
		endpoint: cfg.UCenter.Endpoint,
		appID:    cfg.UCenter.AppID,
		nonce:    cfg.UCenter.Nonce,
		secret:   cfg.UCenterSecret(),
		http: &http.Client{
			Timeout: cfg.UCenter.RequestTimeout.Value(),
			CheckRedirect: func(_ *http.Request, _ []*http.Request) error {
				return http.ErrUseLastResponse
			},
		},
	}
}

func Signature(nonce string, timestamp int64, secret string) string {
	digest := sha256.Sum256([]byte(nonce + strconv.FormatInt(timestamp, 10) + secret))
	return base64.StdEncoding.EncodeToString([]byte(hex.EncodeToString(digest[:])))
}

func (c *Client) request(ctx context.Context, path string, body any, headers http.Header, target any) error {
	data, err := json.Marshal(body)
	if err != nil {
		return ErrUpstream
	}
	// Concatenation is intentional: the existing endpoint may route through /api/?/...
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, c.endpoint+path, bytes.NewReader(data))
	if err != nil {
		return ErrUpstream
	}
	request.Header = headers.Clone()
	request.Header.Set("Content-Type", "application/json; charset=utf-8")
	response, err := c.http.Do(request)
	if err != nil {
		return ErrUpstream
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return ErrUpstream
	}
	if err := json.NewDecoder(response.Body).Decode(target); err != nil {
		return ErrUpstream
	}
	return nil
}

func (c *Client) headers(ctx context.Context) (http.Header, error) {
	timestamp := time.Now().Unix()
	headers := http.Header{}
	headers.Set("appid", c.appID)
	headers.Set("nonce", c.nonce)
	headers.Set("t", strconv.FormatInt(timestamp, 10))
	headers.Set("sign", Signature(c.nonce, timestamp, c.secret))
	var result struct {
		Token string `json:"token"`
	}
	if err := c.request(ctx, "/token", map[string]string{"token": ""}, headers, &result); err != nil {
		return nil, err
	}
	if result.Token == "" {
		return nil, ErrUpstream
	}
	headers.Set("token", result.Token)
	return headers, nil
}

func (c *Client) Login(ctx context.Context, account, password string) (User, error) {
	if c.http.Timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, c.http.Timeout)
		defer cancel()
	}
	headers, err := c.headers(ctx)
	if err != nil {
		return User{}, err
	}
	isUID := 0
	if emailPattern.MatchString(account) {
		isUID = 2
	}
	var login struct {
		Data struct {
			Status   json.Number `json:"status"`
			Username string      `json:"username"`
		} `json:"data"`
	}
	if err := c.request(ctx, "/user/login", map[string]any{
		"username": account, "password": password, "isuid": isUID,
	}, headers, &login); err != nil {
		return User{}, err
	}
	status, err := login.Data.Status.Int64()
	if err != nil {
		return User{}, ErrUpstream
	}
	if status <= 0 {
		return User{}, ErrInvalidCredentials
	}
	headers, err = c.headers(ctx)
	if err != nil {
		return User{}, err
	}
	var user struct {
		Data struct {
			UID      json.Number `json:"uid"`
			Username string      `json:"username"`
			Email    string      `json:"email"`
		} `json:"data"`
	}
	if err := c.request(ctx, "/user/get_user", map[string]any{
		"username": login.Data.Username, "isuid": 0,
	}, headers, &user); err != nil {
		return User{}, err
	}
	uid, err := user.Data.UID.Int64()
	if err != nil || uid <= 0 || user.Data.Username == "" {
		return User{}, ErrUpstream
	}
	return User{UID: uid, Name: user.Data.Username, Email: user.Data.Email}, nil
}
