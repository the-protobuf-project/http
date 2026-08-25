package middleware

// headers.go maps names between HTTP headers and gRPC metadata.
//
// Three matchers, one per direction, matching grpc-gateway's
// WithIncomingHeaderMatcher, WithOutgoingHeaderMatcher and
// WithOutgoingTrailerMatcher. Each answers one question: given a name on one
// side, what name does it take on the other, and does it cross at all?

import "strings"

const (
	// MetadataHeaderPrefix is what a client uses to send arbitrary gRPC
	// metadata: "Grpc-Metadata-Foo: bar" arrives at the service as "foo: bar".
	MetadataHeaderPrefix = "grpc-metadata-"

	// MetadataPrefix is what a permanent HTTP header takes on the way in.
	//
	// "Accept-Language" becomes "grpcgateway-accept-language", which keeps a
	// header the transport owns from colliding with a metadata key the service
	// defines.
	MetadataPrefix = "grpcgateway-"

	// MetadataTrailerPrefix is what a gRPC trailer takes on the way out.
	MetadataTrailerPrefix = "Grpc-Trailer-"

	// BinarySuffix marks base64-encoded binary metadata.
	BinarySuffix = "-bin"
)

// MatcherFunc decides whether a name crosses between HTTP and gRPC, and under
// what name. Returning false drops it.
type MatcherFunc func(name string) (string, bool)

// Headers holds the three matchers a transcoder uses.
type Headers struct {
	// Incoming maps an HTTP request header to a gRPC metadata key.
	Incoming MatcherFunc

	// Outgoing maps a gRPC response metadata key to an HTTP response header.
	Outgoing MatcherFunc

	// Trailer maps a gRPC trailer key to an HTTP trailer.
	Trailer MatcherFunc
}

// DefaultHeaders returns the standard three matchers.
func DefaultHeaders() Headers {
	return Headers{
		Incoming: DefaultIncoming,
		Outgoing: DefaultOutgoing,
		Trailer:  DefaultTrailer,
	}
}

// hopByHop are the headers that must not be forwarded, because they describe
// this hop (RFC 9110 §7.6.1). Forwarding Connection or Transfer-Encoding to a
// service would describe a connection the service is not on.
var hopByHop = map[string]bool{
	"connection": true, "keep-alive": true, "proxy-authenticate": true,
	"proxy-authorization": true, "te": true, "trailer": true,
	"transfer-encoding": true, "upgrade": true,
}

// permanent are the HTTP headers that are prefixed rather than passed through.
//
// These belong to HTTP itself, so a service asking for metadata "host" should
// not silently receive the transport's Host.
var permanent = map[string]bool{
	"accept": true, "accept-charset": true, "accept-encoding": true,
	"accept-language": true, "accept-ranges": true, "authorization": true,
	"cache-control": true, "content-type": true, "cookie": true, "date": true,
	"expect": true, "from": true, "host": true, "if-match": true,
	"if-modified-since": true, "if-none-match": true, "if-unmodified-since": true,
	"max-forwards": true, "origin": true, "pragma": true, "referer": true,
	"user-agent": true, "warning": true, "via": true,
}

// DefaultIncoming is the default request-header rule.
//
// "Grpc-Metadata-Foo" loses its prefix; a permanent header gains
// "grpcgateway-"; anything else passes through lowercased. Hop-by-hop headers
// are dropped.
func DefaultIncoming(name string) (string, bool) {
	lower := strings.ToLower(name)

	if hopByHop[lower] {
		return "", false
	}
	if rest, found := strings.CutPrefix(lower, MetadataHeaderPrefix); found {
		return rest, true
	}
	if permanent[lower] {
		return MetadataPrefix + lower, true
	}
	return lower, true
}

// DefaultOutgoing is the default response rule: every metadata key gains
// "Grpc-Metadata-".
func DefaultOutgoing(name string) (string, bool) { return "Grpc-Metadata-" + name, true }

// DefaultTrailer is the default trailer rule: every trailer key gains
// "Grpc-Trailer-".
func DefaultTrailer(name string) (string, bool) { return MetadataTrailerPrefix + name, true }

// IsBinary reports whether a metadata key carries base64 binary rather than
// text.
func IsBinary(key string) bool {
	return strings.HasSuffix(strings.ToLower(key), BinarySuffix)
}
