package codec

// registry.go is the codec table.

import "strings"

// Entry is one codec's static metadata, as the generator emits it.
//
// The registry stores metadata rather than encoders because negotiation only
// ever needs the metadata. The concrete codec is reached by the generated
// handler switching on [Entry.Index], so encoding stays direct.
type Entry struct {
	// Name is the ?alt= selector, e.a. "json". Unique within a registry and
	// stable: it appears in client URLs.
	Name string

	// MediaTypes are the types this codec answers to, most canonical first. The
	// first entry becomes the response Content-Type; the rest are accepted
	// aliases, e.a. application/x-protobuf alongside application/protobuf.
	MediaTypes []string

	// Framing is how this codec delimits a server-streaming response.
	Framing Framing

	// Index is the position in the registry's slice, and the discriminant the
	// generated handler switches on.
	Index int
}

// ContentType is the Content-Type a response encoded by this codec carries.
func (e *Entry) ContentType() string {
	if len(e.MediaTypes) == 0 {
		return "application/octet-stream"
	}
	return e.MediaTypes[0]
}

// answersTo reports whether this codec answers to a concrete media type.
func (e *Entry) answersTo(mediaType string) bool {
	for _, candidate := range e.MediaTypes {
		if strings.EqualFold(candidate, mediaType) {
			return true
		}
	}
	return false
}

// Registry is the set of codecs a transcoder was generated with.
//
// Ordering is significant: the first entry is the default, used when a request
// expresses no preference at all.
type Registry struct {
	// entries are the codecs, default first.
	entries []Entry
}

// NewRegistry builds a registry over a generated codec slice.
//
// It panics on an empty slice. A transcoder with no codec cannot answer any
// request, and failing at construction is far better than failing on the first
// one — this runs at package init in a generated binary, so the panic surfaces
// at startup rather than in production traffic.
func NewRegistry(entries []Entry) *Registry {
	if len(entries) == 0 {
		panic("codec: a registry needs at least one codec")
	}
	return &Registry{entries: entries}
}

// Default returns the codec used when a request expresses no preference.
func (r *Registry) Default() *Entry { return &r.entries[0] }

// Entries returns every registered codec, in declaration order.
func (r *Registry) Entries() []Entry { return r.entries }

// ByName looks a codec up by its ?alt= name.
func (r *Registry) ByName(name string) *Entry {
	for i := range r.entries {
		if r.entries[i].Name == name {
			return &r.entries[i]
		}
	}
	return nil
}

// ByMediaType looks a codec up by a concrete media type, ignoring parameters.
func (r *Registry) ByMediaType(mediaType string) *Entry {
	bare := strings.TrimSpace(strings.Split(mediaType, ";")[0])
	for i := range r.entries {
		if r.entries[i].answersTo(bare) {
			return &r.entries[i]
		}
	}
	return nil
}

// Names returns the registered ?alt= names, for an error message listing what is
// supported.
func (r *Registry) Names() []string {
	out := make([]string, 0, len(r.entries))
	for i := range r.entries {
		out = append(out, r.entries[i].Name)
	}
	return out
}

// SupportedMediaTypes returns the canonical media type of every registered
// codec, for the same purpose.
func (r *Registry) SupportedMediaTypes() []string {
	out := make([]string, 0, len(r.entries))
	for i := range r.entries {
		out = append(out, r.entries[i].ContentType())
	}
	return out
}
