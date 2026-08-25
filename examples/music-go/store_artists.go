package music

// store_artists.go holds the artist operations.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go/gateway"
	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go/gen/music/v1"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
)

// GetArtist returns one artist. (AIP-131)
func (c *Catalog) GetArtist(name string) (*musicv1.Artist, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	artist, ok := c.artists[name]
	if !ok {
		return nil, notFound("Artist", name)
	}
	return clone(artist), nil
}

// ListArtists returns a page of artists, ordered by resource name. (AIP-132)
//
// A page size of zero means the default, and one over the maximum is capped
// rather than refused: AIP-158 says a service may return fewer than requested,
// so capping is a legal response where rejecting would break a client that
// asked for too many.
func (c *Catalog) ListArtists(pageSize int) ([]*musicv1.Artist, error) {
	const defaultPageSize, maxPageSize = 50, 1000

	c.mu.Lock()
	defer c.mu.Unlock()

	if pageSize <= 0 {
		pageSize = defaultPageSize
	}
	pageSize = min(pageSize, maxPageSize)

	names := sortedNames(c.artists)
	out := make([]*musicv1.Artist, 0, min(pageSize, len(names)))
	for _, name := range names[:min(pageSize, len(names))] {
		out = append(out, clone(c.artists[name]))
	}
	return out, nil
}

// CreateArtist adds an artist. (AIP-133)
func (c *Catalog) CreateArtist(artist *musicv1.Artist) (*musicv1.Artist, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if !strings.HasPrefix(artist.Name, "artists/") {
		return nil, malformedName("artists/{artist}", artist.Name)
	}
	if _, exists := c.artists[artist.Name]; exists {
		return nil, alreadyExists("Artist", artist.Name)
	}

	artist.CreateTime = FixedTime()
	artist.Etag = c.nextEtag()
	c.artists[artist.Name] = artist
	return clone(artist), nil
}

// UpdateArtist applies a patch under a field mask. (AIP-134)
//
// An empty mask replaces every mutable field, which is what AIP-134 specifies —
// and the reason a client that means to change one field should always send one.
func (c *Catalog) UpdateArtist(name string, patch *musicv1.Artist, mask []string) (*musicv1.Artist, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	artist, ok := c.artists[name]
	if !ok {
		return nil, notFound("Artist", name)
	}

	if len(mask) == 0 {
		artist.DisplayName = patch.DisplayName
		artist.Biography = patch.Biography
	}
	for _, field := range mask {
		switch field {
		case "displayName":
			artist.DisplayName = patch.DisplayName
		case "biography":
			artist.Biography = patch.Biography
		default:
			return nil, immutableField(field)
		}
	}

	artist.Etag = c.nextEtag()
	c.artists[name] = artist
	return clone(artist), nil
}

// DeleteArtist removes an artist. (AIP-135)
//
// Without force, an artist that still has tracks is a FAILED_PRECONDITION rather
// than a silent cascade: deleting a parent's children without being asked is the
// kind of thing a caller finds out about afterwards.
func (c *Catalog) DeleteArtist(name string, force bool) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if _, ok := c.artists[name]; !ok {
		return notFound("Artist", name)
	}

	children := c.trackNamesOf(name)
	if len(children) > 0 && !force {
		return apierr.New(apierr.FailedPrecondition,
			fmt.Sprintf("Artist %q still has %d tracks.", name, len(children))).
			WithErrorInfo("CHILDREN_PRESENT", gateway.Domain, map[string]string{
				"resource": name,
				"children": fmt.Sprintf("%d", len(children)),
			}).
			WithDetail(apierr.PreconditionFailure{Violations: []apierr.PreconditionViolation{{
				Type:        "CHILDREN_PRESENT",
				Subject:     name,
				Description: "Pass force=true to delete the artist and its tracks.",
			}}})
	}

	for _, child := range children {
		delete(c.tracks, child)
	}
	delete(c.artists, name)
	return nil
}

// trackNamesOf returns the tracks under an artist. The caller must hold the
// lock.
func (c *Catalog) trackNamesOf(artist string) []string {
	var names []string
	for name := range c.tracks {
		if parentOf(name) == artist {
			names = append(names, name)
		}
	}
	return names
}

// malformedName is the error for a resource name that does not match its
// pattern. (AIP-122)
func malformedName(pattern, got string) *apierr.Error {
	return apierr.InvalidFields([]apierr.FieldViolation{{
		Field:       "name",
		Description: fmt.Sprintf("must match pattern %q, got %q", pattern, got),
		Reason:      "RESOURCE_NAME_MALFORMED",
	}}, "INVALID_ARGUMENT", gateway.Domain, "")
}

// immutableField is the error for an update mask naming a field that cannot be
// changed.
func immutableField(field string) *apierr.Error {
	return apierr.InvalidFields([]apierr.FieldViolation{{
		Field:       field,
		Description: "This field cannot be updated.",
		Reason:      "FIELD_IMMUTABLE",
	}}, "INVALID_ARGUMENT", gateway.Domain, "")
}
