package codec_test

// negotiate_test.go covers content negotiation, which is where an adapter most
// often quietly answers in a format the client refused.

import (
	"testing"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/codec"
)

// registry is a two-codec registry: JSON default, SSE streaming-only.
func registry() *codec.Registry {
	return codec.NewRegistry([]codec.Entry{
		{Name: "json", MediaTypes: []string{"application/json"}, Framing: codec.JSONArray, Index: 0},
		{Name: "sse", MediaTypes: []string{"text/event-stream"}, Framing: codec.SSE, Index: 1},
	})
}

func TestAltWinsOverAccept(t *testing.T) {
	// The order is fixed by README §3: an explicit ?alt= is the client saying
	// exactly what it wants, and it outranks a header it may not control.
	entry, err := codec.ResponseCodec(registry(), codec.Negotiation{
		Alt: "sse", Accept: "application/json", Streaming: true,
	}, nil, "test")
	if err != nil {
		t.Fatalf("negotiate: %v", err)
	}
	if entry.Name != "sse" {
		t.Errorf("codec = %q, want sse", entry.Name)
	}
}

func TestStreamingOnlyCodecIsRejectedOnAUnaryMethod(t *testing.T) {
	// A one-event event stream is strictly worse for the client than a normal
	// body, so this is an error rather than a silent downgrade.
	_, err := codec.ResponseCodec(registry(), codec.Negotiation{Alt: "sse"}, nil, "test")
	if err == nil {
		t.Fatal("SSE was accepted for a unary method")
	}
}

func TestUnsatisfiableAcceptIsNotAcceptable(t *testing.T) {
	// The adapter does not fall back to a codec the client excluded: answering
	// in a media type they refused is worse than telling them there is no
	// overlap.
	_, err := codec.ResponseCodec(registry(), codec.Negotiation{
		Accept: "application/xml",
	}, nil, "test")
	if err == nil {
		t.Fatal("an unsatisfiable Accept was served anyway")
	}
}

func TestAcceptPrefersTheMoreSpecificRangeAtEqualQuality(t *testing.T) {
	// RFC 9110 precedence: "*/*, application/json" prefers JSON even though the
	// wildcard came first.
	entries := codec.ParseAccept("*/*, application/json")
	if len(entries) != 2 {
		t.Fatalf("parsed %d entries, want 2", len(entries))
	}
	if entries[0].Media.String() != "application/json" {
		t.Errorf("first = %s, want application/json", entries[0].Media)
	}
}

func TestZeroQualityIsARefusalNotALowPreference(t *testing.T) {
	// A codec matched only by a q=0 entry must not be selected.
	_, err := codec.ResponseCodec(registry(), codec.Negotiation{
		Accept: "application/json;q=0",
	}, nil, "test")
	if err == nil {
		t.Fatal("a refused codec was selected")
	}
}

func TestWildcardAcceptFallsBackRatherThanRejecting(t *testing.T) {
	entry, err := codec.ResponseCodec(registry(), codec.Negotiation{Accept: "*/*"}, nil, "test")
	if err != nil {
		t.Fatalf("negotiate: %v", err)
	}
	if entry.Name != "json" {
		t.Errorf("codec = %q, want the default", entry.Name)
	}
}

func TestUnknownContentTypeIsUnsupportedMediaType(t *testing.T) {
	_, err := codec.RequestCodec(registry(), codec.Negotiation{
		ContentType: "application/xml",
	}, "test")
	if err == nil {
		t.Fatal("an unregistered Content-Type was accepted")
	}
}

func TestBodylessRequestNeedsNoCodec(t *testing.T) {
	entry, err := codec.RequestCodec(registry(), codec.Negotiation{}, "test")
	if err != nil {
		t.Fatalf("negotiate: %v", err)
	}
	if entry != nil {
		t.Errorf("codec = %v, want none for a request with no body", entry)
	}
}

func TestMediaTypeParametersAreIgnoredForSelection(t *testing.T) {
	entry, err := codec.RequestCodec(registry(), codec.Negotiation{
		ContentType: "application/json; charset=utf-8",
	}, "test")
	if err != nil {
		t.Fatalf("negotiate: %v", err)
	}
	if entry == nil || entry.Name != "json" {
		t.Errorf("codec = %v, want json", entry)
	}
}
