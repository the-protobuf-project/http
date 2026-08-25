package middleware

// metadata.go builds gRPC metadata from an HTTP request.

import (
	"encoding/base64"
	"net/http"
	"sort"
	"strconv"
	"strings"
	"time"
)

// Value is one metadata value: text, or base64-decoded binary for a "-bin" key.
type Value struct {
	// Text is the value when the key is not binary.
	Text string

	// Binary is the decoded value when the key ends in "-bin".
	Binary []byte

	// IsBinary distinguishes an empty binary value from a text one.
	IsBinary bool
}

// Metadata is what will be sent with a call.
//
// Keys are lowercase, which is what gRPC requires. [Metadata.Keys] sorts, so
// anything that iterates — a log line, a test assertion — sees a stable order.
type Metadata struct {
	// entries maps a lowercase key to its values, in the order they arrived.
	entries map[string][]Value
}

// NewMetadata returns empty metadata.
func NewMetadata() Metadata { return Metadata{entries: map[string][]Value{}} }

// Append adds a text value.
func (m *Metadata) Append(key, value string) {
	m.ensure()
	key = strings.ToLower(key)
	m.entries[key] = append(m.entries[key], Value{Text: value})
}

// AppendBinary adds a binary value, for a "-bin" key.
func (m *Metadata) AppendBinary(key string, value []byte) {
	m.ensure()
	key = strings.ToLower(key)
	m.entries[key] = append(m.entries[key], Value{Binary: value, IsBinary: true})
}

// Get returns the values for a key.
func (m Metadata) Get(key string) []Value { return m.entries[strings.ToLower(key)] }

// Text returns the first value for a key as text, or "".
func (m Metadata) Text(key string) string {
	values := m.Get(key)
	if len(values) == 0 || values[0].IsBinary {
		return ""
	}
	return values[0].Text
}

// Keys returns every key, sorted.
func (m Metadata) Keys() []string {
	keys := make([]string, 0, len(m.entries))
	for key := range m.entries {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

// Len is the number of distinct keys.
func (m Metadata) Len() int { return len(m.entries) }

// ensure lazily allocates, so a zero Metadata is usable.
func (m *Metadata) ensure() {
	if m.entries == nil {
		m.entries = map[string][]Value{}
	}
}

// MetadataFromHeaders builds metadata from request headers, per an incoming
// matcher.
//
// A "-bin" header is base64-decoded; one that fails to decode is dropped rather
// than forwarded as text, since a service reading it as binary would otherwise
// get silent garbage.
func MetadataFromHeaders(header http.Header, match MatcherFunc) Metadata {
	metadata := NewMetadata()
	for name, values := range header {
		key, ok := match(name)
		if !ok {
			continue
		}
		for _, value := range values {
			if !IsBinary(key) {
				metadata.Append(key, value)
				continue
			}
			if decoded, err := decodeBase64(value); err == nil {
				metadata.AppendBinary(key, decoded)
			}
		}
	}
	return metadata
}

// Annotator adds metadata to a call from the request.
//
// grpc-gateway's WithMetadata. Several may be registered; each sees what the
// ones before it added, so one can build on another.
type Annotator interface {
	// Name identifies the annotator in diagnostics.
	Name() string

	// Annotate adds to the metadata, given the request headers.
	Annotate(header http.Header, metadata *Metadata)
}

// ParseGrpcTimeout parses a Grpc-Timeout header.
//
// The wire format is a positive integer followed by a unit: H, M, S, m, u, n.
// A malformed header yields false, which the caller treats as "no client
// deadline" rather than as an error — a bad timeout header should not fail an
// otherwise valid request.
func ParseGrpcTimeout(raw string) (time.Duration, bool) {
	if len(raw) < 2 {
		return 0, false
	}
	amount, err := strconv.ParseInt(raw[:len(raw)-1], 10, 64)
	if err != nil || amount < 0 {
		return 0, false
	}

	units := map[byte]time.Duration{
		'H': time.Hour, 'M': time.Minute, 'S': time.Second,
		'm': time.Millisecond, 'u': time.Microsecond, 'n': time.Nanosecond,
	}
	unit, ok := units[raw[len(raw)-1]]
	if !ok {
		return 0, false
	}
	return time.Duration(amount) * unit, true
}

// decodeBase64 decodes standard or URL-safe base64, with or without padding.
//
// gRPC metadata is standard base64, but clients send the URL-safe alphabet
// often enough that rejecting it would be unhelpful.
func decodeBase64(input string) ([]byte, error) {
	if strings.ContainsAny(input, "-_") {
		return base64.RawURLEncoding.DecodeString(strings.TrimRight(input, "="))
	}
	return base64.RawStdEncoding.DecodeString(strings.TrimRight(input, "="))
}
