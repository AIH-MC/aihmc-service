package httpapi

import (
	"net/http/httptest"
	"testing"
)

func TestClientIPTrustsOnlyTheSameHostGateway(t *testing.T) {
	cases := []struct {
		peer      string
		forwarded string
		want      string
	}{
		{"127.0.0.1:1234", "198.51.100.9", "198.51.100.9"},
		{"[::1]:1234", "2001:db8::9", "2001:db8::9"},
		{"203.0.113.7:1234", "198.51.100.9", "203.0.113.7"},
		{"127.0.0.1:1234", "198.51.100.9, 203.0.113.7", "127.0.0.1"},
		{"127.0.0.1:1234", "invalid", "127.0.0.1"},
		{"127.0.0.1:1234", "", "127.0.0.1"},
	}
	for _, test := range cases {
		r := httptest.NewRequest("POST", "/", nil)
		r.RemoteAddr = test.peer
		r.Header.Set("X-Real-IP", test.forwarded)
		if got := clientIP(r); got != test.want {
			t.Errorf("peer %s: want %s, got %s", test.peer, test.want, got)
		}
	}
}
