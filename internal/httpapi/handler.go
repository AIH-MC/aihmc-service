// Package httpapi preserves the existing router's public paths and response shapes.
package httpapi

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"net/http"
	"net/url"
	"path"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/AIH-MC/aihmc-service/internal/auth"
	"github.com/go-sql-driver/mysql"
)

var uuidPath = regexp.MustCompile(`profile/([a-f0-9]{32})`)

type Handler struct {
	auth    *auth.Service
	avatars AvatarProvider
	logger  *slog.Logger
}

type AvatarProvider interface {
	Avatar(context.Context, string, int) ([]byte, error)
}

func New(service *auth.Service, avatars AvatarProvider, logger *slog.Logger) *Handler {
	return &Handler{auth: service, avatars: avatars, logger: logger}
}

type result struct {
	status int
	body   any
}

func jsonResult(value any, err error) (result, error) {
	return result{status: 200, body: value}, err
}

func emptyResult(err error) (result, error) {
	return result{status: 204}, err
}

func profileResult(value *auth.Profile, err error) (result, error) {
	if value == nil {
		return emptyResult(err)
	}
	return jsonResult(value, err)
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	started := time.Now()
	response, err := h.dispatch(r)
	if err != nil {
		var apiError *auth.APIError
		if !errors.As(err, &apiError) {
			apiError = &auth.APIError{Status: 500, Name: "InternalServerError", Message: "Internal server error."}
			attributes := []any{"path", r.URL.Path, "error_type", errorCategory(err)}
			var databaseError *mysql.MySQLError
			if errors.As(err, &databaseError) {
				attributes = append(attributes, "database_code", databaseError.Number)
			}
			h.logger.Error("request failed", attributes...)
		}
		response = result{status: apiError.Status, body: map[string]string{"error": apiError.Name, "errorMessage": apiError.Message}}
	}
	contentType := "application/json; charset=utf-8"
	if _, ok := response.body.([]byte); ok {
		contentType = "image/png"
	}
	w.Header().Set("Content-Type", contentType)
	w.WriteHeader(response.status)
	if image, ok := response.body.([]byte); ok {
		if _, err := w.Write(image); err != nil {
			h.logger.Error("image response write failed", "path", r.URL.Path)
		}
	} else if response.status != http.StatusNoContent {
		encoder := json.NewEncoder(w)
		encoder.SetEscapeHTML(false)
		if err := encoder.Encode(response.body); err != nil {
			h.logger.Error("response write failed", "path", r.URL.Path)
		}
	}
	h.logger.Info("request completed", "method", r.Method, "path", r.URL.Path, "status", response.status, "duration_ms", time.Since(started).Milliseconds())
}

func errorCategory(err error) string {
	var databaseError *mysql.MySQLError
	if errors.As(err, &databaseError) {
		return "database"
	}
	return "dependency_or_internal"
}

func method(r *http.Request, expected string) error {
	if r.Method != expected {
		return &auth.APIError{Status: 405, Name: "HTTP/1.1 405 Method Not Allowed", Message: "不支持的请求方法"}
	}
	return nil
}

func decode(r *http.Request, target any) error {
	decoder := json.NewDecoder(r.Body)
	if err := decoder.Decode(target); err != nil {
		return badJSON()
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return badJSON()
	}
	return nil
}

func badJSON() error {
	return &auth.APIError{Status: 400, Name: "IllegalArgumentException", Message: "Invalid JSON request body."}
}

func truth(value any) bool {
	switch value := value.(type) {
	case nil:
		return false
	case bool:
		return value
	case string:
		return auth.PHPTruthyQuery(value)
	case float64:
		return value != 0
	case []any:
		return len(value) != 0
	case map[string]any:
		return len(value) != 0
	default:
		return false
	}
}

func lastQuery(query url.Values, key string) string {
	values := query[key]
	if len(values) == 0 {
		return ""
	}
	return values[len(values)-1]
}

func (h *Handler) dispatch(r *http.Request) (result, error) {
	ctx := r.Context()
	// Fixed routes intentionally match the complete URI, as in index.php.
	uri := r.RequestURI
	switch {
	case uri == "/":
		return jsonResult(map[string]string{"message": "This is AI Hobbyist MC API, please use the correct endpoint to access the API."}, nil)
	case uri == "/mcauth":
		if err := method(r, "GET"); err != nil {
			return result{}, err
		}
		return jsonResult(h.auth.Metadata(ctx))
	case uri == "/mcauth/authserver/authenticate":
		if err := method(r, "POST"); err != nil {
			return result{}, err
		}
		var request struct {
			Username    string `json:"username"`
			Password    string `json:"password"`
			ClientToken string `json:"clientToken"`
			RequestUser any    `json:"requestUser"`
			Agent       struct {
				Name string `json:"name"`
			} `json:"agent"`
		}
		if err := decode(r, &request); err != nil {
			return result{}, err
		}
		return jsonResult(h.auth.Authenticate(ctx, request.Username, request.Password, request.ClientToken, request.Agent.Name, truth(request.RequestUser)))
	case uri == "/mcauth/authserver/refresh", uri == "/mcauth/authserver/validate", uri == "/mcauth/authserver/invalidate":
		if err := method(r, "POST"); err != nil {
			return result{}, err
		}
		var request struct {
			AccessToken string `json:"accessToken"`
			ClientToken string `json:"clientToken"`
			RequestUser any    `json:"requestUser"`
		}
		if err := decode(r, &request); err != nil {
			return result{}, err
		}
		switch uri {
		case "/mcauth/authserver/refresh":
			return jsonResult(h.auth.Refresh(ctx, request.AccessToken, request.ClientToken, truth(request.RequestUser)))
		case "/mcauth/authserver/validate":
			return emptyResult(h.auth.Validate(ctx, request.AccessToken, request.ClientToken))
		default:
			return emptyResult(h.auth.Invalidate(ctx, request.AccessToken, request.ClientToken))
		}
	case uri == "/mcauth/authserver/signout":
		if err := method(r, "POST"); err != nil {
			return result{}, err
		}
		var request struct {
			Username string `json:"username"`
			Password string `json:"password"`
		}
		if err := decode(r, &request); err != nil {
			return result{}, err
		}
		return emptyResult(h.auth.Signout(ctx, request.Username, request.Password))
	case strings.HasPrefix(uri, "/mcauth/sessionserver/session/minecraft/profile"):
		if err := method(r, "GET"); err != nil {
			return result{}, err
		}
		matches := uuidPath.FindStringSubmatch(uri)
		uuid := ""
		if len(matches) == 2 {
			uuid = matches[1]
		}
		return profileResult(h.auth.Profile(ctx, uuid, lastQuery(r.URL.Query(), "unsigned")))
	case uri == "/mcauth/api/profiles/minecraft":
		if err := method(r, "POST"); err != nil {
			return result{}, err
		}
		var names []string
		if err := decode(r, &names); err != nil {
			return result{}, err
		}
		return jsonResult(h.auth.Profiles(ctx, names))
	case uri == "/mcauth/sessionserver/session/minecraft/join":
		if err := method(r, "POST"); err != nil {
			return result{}, err
		}
		var request struct {
			ServerID        string `json:"serverId"`
			AccessToken     string `json:"accessToken"`
			SelectedProfile string `json:"selectedProfile"`
		}
		if err := decode(r, &request); err != nil {
			return result{}, err
		}
		return emptyResult(h.auth.Join(ctx, request.ServerID, request.AccessToken, request.SelectedProfile, clientIP(r)))
	case strings.HasPrefix(uri, "/mcauth/sessionserver/session/minecraft/hasJoined"):
		if err := method(r, "GET"); err != nil {
			return result{}, err
		}
		query := r.URL.Query()
		name, err := url.QueryUnescape(lastQuery(query, "username"))
		if err != nil {
			return result{}, &auth.APIError{Status: 400, Name: "IllegalArgumentException", Message: "Invalid username encoding."}
		}
		return profileResult(h.auth.HasJoined(ctx, name, lastQuery(query, "serverId")))
	case strings.HasPrefix(uri, "/mcauth/users/profiles/minecraft/"), strings.HasPrefix(uri, "/mcauth/api/users/profiles/minecraft/"):
		if err := method(r, "GET"); err != nil {
			return result{}, err
		}
		name, err := url.QueryUnescape(path.Base(r.URL.EscapedPath()))
		if err != nil {
			return result{}, &auth.APIError{Status: 400, Name: "IllegalArgumentException", Message: "Invalid username encoding."}
		}
		return jsonResult(h.auth.Name(ctx, name))
	case strings.HasPrefix(uri, "/helm/"):
		if err := method(r, "GET"); err != nil {
			return result{}, err
		}
		parts := strings.Split(strings.Trim(r.URL.EscapedPath(), "/"), "/")
		if len(parts) < 3 {
			return result{}, &auth.APIError{Status: 400, Name: "IllegalArgumentException", Message: "Avatar UUID and size are required."}
		}
		size, err := strconv.Atoi(parts[2])
		if err != nil || size <= 0 {
			return result{}, &auth.APIError{Status: 400, Name: "IllegalArgumentException", Message: "Avatar size must be a positive integer."}
		}
		return jsonResult(h.avatars.Avatar(ctx, parts[1], size))
	case uri == "/system/deluser":
		if err := method(r, "POST"); err != nil {
			return result{}, err
		}
		var request struct {
			SuperToken string `json:"super_token"`
			Prarms     struct {
				UID json.Number `json:"uid"`
			} `json:"prarms"`
		}
		if err := decode(r, &request); err != nil {
			return result{}, err
		}
		var uid int64
		if request.Prarms.UID != "" {
			var err error
			uid, err = request.Prarms.UID.Int64()
			if err != nil {
				return result{}, badJSON()
			}
		}
		status, message, err := h.auth.DeleteUser(ctx, request.SuperToken, uid)
		return result{status: status, body: map[string]string{"msg": message}}, err
	default:
		return jsonResult(h.auth.Metadata(ctx))
	}
}
