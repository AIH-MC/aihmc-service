// Package auth implements the existing external-login protocol.
package auth

import (
	"bytes"
	"crypto"
	"crypto/md5"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha1"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"strconv"
	"strings"
)

type Property struct {
	Name      string `json:"name"`
	Value     string `json:"value"`
	Signature string `json:"signature,omitempty"`
}

type Profile struct {
	ID         string     `json:"id"`
	Name       string     `json:"name"`
	Properties []Property `json:"properties,omitempty"`
}

type User struct {
	ID         string     `json:"id"`
	Properties []Property `json:"properties"`
}

type Login struct {
	AccessToken       string    `json:"accessToken"`
	ClientToken       string    `json:"clientToken"`
	AvailableProfiles []Profile `json:"availableProfiles,omitempty"`
	SelectedProfile   *Profile  `json:"selectedProfile,omitempty"`
	User              *User     `json:"user,omitempty"`
}

type TextureReference struct {
	URL      string            `json:"url"`
	Metadata map[string]string `json:"metadata,omitempty"`
}

type TextureSet struct {
	Skin *TextureReference `json:"SKIN,omitempty"`
	Cape *TextureReference `json:"CAPE,omitempty"`
}

type TexturePayload struct {
	Timestamp   int64       `json:"timestamp"`
	ProfileID   string      `json:"profileId"`
	ProfileName string      `json:"profileName"`
	Textures    *TextureSet `json:"textures,omitempty"`
}

func md5Hex(value string) string {
	digest := md5.Sum([]byte(value))
	return hex.EncodeToString(digest[:])
}

func AccessToken(name, client string, timestamp int64) string {
	value := md5Hex(name+md5Hex(client)) + md5Hex(strconv.FormatInt(timestamp, 10))
	return base64.StdEncoding.EncodeToString([]byte(value))
}

func RefreshedToken(client string, timestamp int64) string {
	encodedClient := base64.StdEncoding.EncodeToString([]byte(md5Hex(client)))
	value := md5Hex(encodedClient + md5Hex(strconv.FormatInt(timestamp, 10)))
	return base64.StdEncoding.EncodeToString([]byte(value))
}

func ProfileID(name, email string, uid int64) string {
	return md5Hex(name + email + md5Hex(strconv.FormatInt(uid, 10)))
}

func ClientToken() (string, error) {
	var value [16]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", err
	}
	return hex.EncodeToString(value[:]), nil
}

func NewUUID() (string, error) {
	var value [16]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", err
	}
	value[6] = value[6]&0x0f | 0x40
	value[8] = value[8]&0x3f | 0x80
	return hex.EncodeToString(value[:]), nil
}

func Sign(value string, key *rsa.PrivateKey) (string, error) {
	digest := sha1.Sum([]byte(value))
	signature, err := rsa.SignPKCS1v15(nil, key, crypto.SHA1, digest[:])
	if err != nil {
		return "", err
	}
	return base64.StdEncoding.EncodeToString(signature), nil
}

// PHPTruthyQuery preserves PHP string truth values, including the special "0" case.
func PHPTruthyQuery(value string) bool { return value != "" && value != "0" }

func EncodeTexture(timestamp int64, uuid, name, skin, model, cape, baseURL string) (string, error) {
	payload := TexturePayload{Timestamp: timestamp, ProfileID: uuid, ProfileName: name}
	if skin != "" || cape != "" {
		payload.Textures = &TextureSet{}
	}
	if skin != "" {
		payload.Textures.Skin = &TextureReference{
			URL:      strings.TrimRight(baseURL, "/") + "/" + skin,
			Metadata: map[string]string{"model": model},
		}
	}
	if cape != "" {
		payload.Textures.Cape = &TextureReference{URL: strings.TrimRight(baseURL, "/") + "/" + cape}
	}
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "    ")
	if err := encoder.Encode(payload); err != nil {
		return "", err
	}
	// Match PHP's pretty JSON and slash escaping for newly generated texture values.
	value := strings.TrimSuffix(buffer.String(), "\n")
	value = strings.ReplaceAll(value, "/", `\/`)
	return base64.StdEncoding.EncodeToString([]byte(value)), nil
}
