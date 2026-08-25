package middleware_test

// metadata_test.go covers the header/metadata mapping, which is where a transcoder
// most easily leaks a transport header into a service's namespace.

import (
	"net/http"
	"testing"
	"time"

	"github.com/the-protobuf-project/http/transcode-go/middleware"
)

func TestIncomingMappingSeparatesTheThreeCases(t *testing.T) {
	for _, tc := range []struct {
		header string
		want   string
		kept   bool
	}{
		// Explicit metadata loses its prefix.
		{"Grpc-Metadata-Tenant", "tenant", true},
		// A permanent HTTP header is prefixed, so a service asking for metadata
		// "host" does not silently receive the transport's Host.
		{"Host", "grpcgateway-host", true},
		{"Authorization", "grpcgateway-authorization", true},
		// A hop-by-hop header describes this hop and must not travel.
		{"Connection", "", false},
		{"Transfer-Encoding", "", false},
		// Anything else passes through lowercased.
		{"X-Tenant-Id", "x-tenant-id", true},
	} {
		got, kept := middleware.DefaultIncoming(tc.header)
		if kept != tc.kept {
			t.Errorf("%s: kept = %v, want %v", tc.header, kept, tc.kept)
			continue
		}
		if kept && got != tc.want {
			t.Errorf("%s: mapped to %q, want %q", tc.header, got, tc.want)
		}
	}
}

func TestBinaryMetadataIsDecoded(t *testing.T) {
	header := http.Header{"Grpc-Metadata-Trace-Bin": []string{"aGVsbG8="}}
	metadata := middleware.MetadataFromHeaders(header, middleware.DefaultIncoming)

	values := metadata.Get("trace-bin")
	if len(values) != 1 {
		t.Fatalf("got %d values, want 1", len(values))
	}
	if !values[0].IsBinary {
		t.Fatal("a -bin key was kept as text")
	}
	if string(values[0].Binary) != "hello" {
		t.Errorf("decoded %q, want hello", values[0].Binary)
	}
}

func TestUndecodableBinaryMetadataIsDropped(t *testing.T) {
	// Forwarding it as text would hand a service reading it as binary silent
	// garbage.
	header := http.Header{"Grpc-Metadata-Trace-Bin": []string{"!!!not base64!!!"}}
	metadata := middleware.MetadataFromHeaders(header, middleware.DefaultIncoming)

	if got := len(metadata.Get("trace-bin")); got != 0 {
		t.Errorf("kept %d values, want the undecodable one dropped", got)
	}
}

func TestGrpcTimeoutParsesEveryUnit(t *testing.T) {
	for _, tc := range []struct {
		raw  string
		want time.Duration
	}{
		{"30S", 30 * time.Second},
		{"500m", 500 * time.Millisecond},
		{"1H", time.Hour},
		{"100u", 100 * time.Microsecond},
		{"5n", 5 * time.Nanosecond},
		{"2M", 2 * time.Minute},
	} {
		got, ok := middleware.ParseGrpcTimeout(tc.raw)
		if !ok {
			t.Errorf("%s: rejected", tc.raw)
			continue
		}
		if got != tc.want {
			t.Errorf("%s: parsed %v, want %v", tc.raw, got, tc.want)
		}
	}
}

func TestMalformedGrpcTimeoutIsNoDeadlineNotAnError(t *testing.T) {
	// A bad timeout header should not fail an otherwise valid request; the
	// configured default takes over.
	for _, raw := range []string{"", "30", "abcS", "30X", "-5S"} {
		if _, ok := middleware.ParseGrpcTimeout(raw); ok {
			t.Errorf("%q: accepted, want rejected", raw)
		}
	}
}

func TestMetadataKeysAreSortedAndLowercased(t *testing.T) {
	// Metadata ends up in logs and in test assertions, and an order that shifts
	// between runs makes both worse.
	metadata := middleware.NewMetadata()
	metadata.Append("Zulu", "1")
	metadata.Append("alpha", "2")
	metadata.Append("MIKE", "3")

	keys := metadata.Keys()
	want := []string{"alpha", "mike", "zulu"}
	if len(keys) != len(want) {
		t.Fatalf("keys = %v, want %v", keys, want)
	}
	for i := range want {
		if keys[i] != want[i] {
			t.Errorf("keys[%d] = %q, want %q", i, keys[i], want[i])
		}
	}
}
