package builtin

// realip.go recovers the client's address from proxy headers.

import (
	"net"
	"net/http"
	"strings"

	"github.com/the-protobuf-project/http/transcode-go/middleware"
)

// clientIPKey is the context key the resolved address is stored under.
type clientIPKey struct{}

// ClientIPFrom returns the resolved client address on a routing context.
func ClientIPFrom(cx *middleware.RouteCx) (string, bool) {
	ip, ok := cx.Ctx.Value(clientIPKey{}).(string)
	return ip, ok
}

// RealIP resolves the client address behind a proxy.
//
// X-Forwarded-For accumulates left to right, so the leftmost entry is the
// original client — and is also entirely client-controlled. TrustedHops says how
// many proxies at the right end are yours; the address is taken from just left
// of them, and anything further left is ignored.
//
// Getting this wrong is how address rate limits and allowlists get bypassed, so
// the default trusts nothing and falls back to the transport peer.
type RealIP struct {
	// trustedHops is how many proxies closest to this server are trusted.
	trustedHops int
}

// Direct trusts no proxy: the transport peer is the client.
func Direct() *RealIP { return &RealIP{} }

// TrustedHops trusts the given number of proxies closest to this server. One
// load balancer in front means one hop.
func TrustedHops(hops int) *RealIP { return &RealIP{trustedHops: hops} }

// Name implements [middleware.Interceptor].
func (*RealIP) Name() string { return "real-ip" }

// Resolve returns the client address for a request, or "".
func (r *RealIP) Resolve(cx *middleware.RouteCx) string {
	if r.trustedHops == 0 {
		return peerIP(cx)
	}

	forwarded := forwardedFor(cx.Request.Header)
	// Count back past the proxies we trust. If the header is shorter than that,
	// it was not written by our own chain, so it is not trusted.
	index := len(forwarded) - r.trustedHops - 1
	if index < 0 || index >= len(forwarded) {
		return peerIP(cx)
	}
	return forwarded[index]
}

// OnRoute publishes the resolved address for later interceptors, and forwards it
// to the service so the backend logs the caller rather than the proxy.
func (r *RealIP) OnRoute(cx *middleware.RouteCx) error {
	if ip := r.Resolve(cx); ip != "" {
		cx.Set(clientIPKey{}, ip)
		cx.Metadata.Append("x-forwarded-for", ip)
	}
	return nil
}

// forwardedFor returns every parsable address in X-Forwarded-For, in order.
//
// Unparsable entries are dropped rather than kept as opaque strings: the whole
// point of the header here is to feed an address-keyed policy, and a policy
// keyed on something that is not an address is worse than one keyed on the
// transport peer.
func forwardedFor(header http.Header) []string {
	var out []string
	for _, value := range header.Values("X-Forwarded-For") {
		for _, entry := range strings.Split(value, ",") {
			entry = strings.TrimSpace(entry)
			if net.ParseIP(entry) != nil {
				out = append(out, entry)
			}
		}
	}
	return out
}

// peerIP is the transport peer's address, without its port.
func peerIP(cx *middleware.RouteCx) string {
	if cx.Peer == "" {
		return ""
	}
	host, _, err := net.SplitHostPort(cx.Peer)
	if err != nil {
		return cx.Peer
	}
	return host
}
