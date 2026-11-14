package httpapi

import (
	"net"
	"net/http"
)

func clientIP(r *http.Request) string {
	address, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		address = r.RemoteAddr
	}
	peer := net.ParseIP(address)
	// The default deployment binds Go to loopback behind the same-host gateway.
	// That gateway must overwrite X-Real-IP with its TCP client's address.
	if peer != nil && peer.IsLoopback() {
		if forwarded := net.ParseIP(r.Header.Get("X-Real-IP")); forwarded != nil {
			return forwarded.String()
		}
	}
	return address
}
