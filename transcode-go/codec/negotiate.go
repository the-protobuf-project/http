package codec

// negotiate.go selects the codecs a request and its response use.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/http/transcode-go/apierr"
)

// Negotiation is what the request asked for, gathered from the parts that
// select a codec.
type Negotiation struct {
	// ContentType is the request's Content-Type, if it carried a body.
	ContentType string

	// Accept is the request's Accept header, if present.
	Accept string

	// Alt is the ?alt= query parameter, if present.
	Alt string

	// Streaming is whether the method streams its response, which decides
	// whether a streaming-only codec such as SSE is a legal choice.
	Streaming bool
}

// RequestCodec selects the codec that decodes the request body.
//
// A request with no body needs no codec and yields a nil entry with no error.
// A Content-Type naming no registered codec is 415.
func RequestCodec(registry *Registry, n Negotiation, domain string) (*Entry, error) {
	if n.ContentType == "" {
		return nil, nil
	}
	if entry := registry.ByMediaType(n.ContentType); entry != nil {
		return entry, nil
	}
	return nil, apierr.UnsupportedMediaType(n.ContentType, registry.SupportedMediaTypes(), domain)
}

// ResponseCodec selects the codec that encodes the response.
//
// The order is fixed by README §3: an explicit ?alt= wins, then Accept, then
// whatever decoded the request, then the registry default.
//
// The failures are 400 when ?alt= names an unknown codec or names a
// streaming-only codec on a unary method, and 406 when Accept is present and
// nothing in it is registered. The transcoder does not fall back to a codec the
// client excluded: answering in a media type they refused is worse than telling
// them there is no overlap.
func ResponseCodec(registry *Registry, n Negotiation, request *Entry, domain string) (*Entry, error) {
	if n.Alt != "" {
		return selectByAlt(registry, n.Alt, n.Streaming, domain)
	}

	if n.Accept != "" {
		if entry := selectByAccept(registry, n.Accept, n.Streaming); entry != nil {
			return entry, nil
		}
		// A wildcard means "anything", so reaching here with one present means
		// the only matches were streaming-only codecs on a unary method.
		if !AcceptsAnything(n.Accept) {
			return nil, apierr.NotAcceptable(n.Accept, registry.SupportedMediaTypes(), domain)
		}
	}

	fallback := request
	if fallback == nil {
		fallback = registry.Default()
	}
	if fallback.Framing.AllowsUnary() || n.Streaming {
		return fallback, nil
	}
	return registry.Default(), nil
}

// selectByAlt resolves an explicit ?alt= selection.
func selectByAlt(registry *Registry, alt string, streaming bool, domain string) (*Entry, error) {
	entry := registry.ByName(alt)
	if entry == nil {
		return nil, apierr.New(apierr.InvalidArgument, fmt.Sprintf("Unknown response format %q.", alt)).
			WithErrorInfo("UNKNOWN_RESPONSE_FORMAT", domain, map[string]string{
				"supported": strings.Join(registry.Names(), ", "),
			})
	}

	if !streaming && !entry.Framing.AllowsUnary() {
		return nil, apierr.New(apierr.InvalidArgument, fmt.Sprintf(
			"Response format %q is only available for streaming methods.", alt,
		)).WithErrorInfo("STREAMING_ONLY_FORMAT", domain, map[string]string{"format": alt})
	}
	return entry, nil
}

// selectByAccept walks the Accept entries in preference order, returning the
// first registered codec that is legal for this method.
func selectByAccept(registry *Registry, accept string, streaming bool) *Entry {
	for _, wanted := range ParseAccept(accept) {
		if wanted.IsRefusal() {
			continue
		}
		entries := registry.Entries()
		for i := range entries {
			codec := &entries[i]
			if !streaming && !codec.Framing.AllowsUnary() {
				continue
			}
			for _, media := range codec.MediaTypes {
				if wanted.Media.Matches(media) {
					return codec
				}
			}
		}
	}
	return nil
}
