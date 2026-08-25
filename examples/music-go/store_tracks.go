package music

// store_tracks.go holds the track operations.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/http/examples/music-go/gen/music/v1"
	"github.com/the-protobuf-project/http/examples/music-go/routes"
	"github.com/the-protobuf-project/http/transcode-go/apierr"
)

// GetTrack returns one track. (AIP-131)
func (c *Catalog) GetTrack(name string) (*musicv1.Track, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	track, ok := c.tracks[name]
	if !ok {
		return nil, notFound("Track", name)
	}
	return clone(track), nil
}

// ListTracks returns a page of one artist's tracks. (AIP-132)
//
// A parent that does not exist is NOT_FOUND rather than an empty page: those
// are different answers, and collapsing them hides a typo in the parent name
// behind a plausible-looking empty result.
func (c *Catalog) ListTracks(parent string, pageSize int) ([]*musicv1.Track, error) {
	const defaultPageSize, maxPageSize = 50, 1000

	c.mu.Lock()
	defer c.mu.Unlock()

	if _, ok := c.artists[parent]; !ok {
		return nil, notFound("Artist", parent)
	}
	if pageSize <= 0 {
		pageSize = defaultPageSize
	}
	pageSize = min(pageSize, maxPageSize)

	var out []*musicv1.Track
	for _, name := range sortedNames(c.tracks) {
		if len(out) == pageSize {
			break
		}
		if parentOf(name) == parent {
			out = append(out, clone(c.tracks[name]))
		}
	}
	return out, nil
}

// CreateTrack adds a track under an artist. (AIP-133)
func (c *Catalog) CreateTrack(parent string, track *musicv1.Track) (*musicv1.Track, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if _, ok := c.artists[parent]; !ok {
		return nil, notFound("Artist", parent)
	}
	if !strings.HasPrefix(track.Name, parent+"/tracks/") {
		return nil, malformedName(parent+"/tracks/{track}", track.Name)
	}
	if _, exists := c.tracks[track.Name]; exists {
		return nil, alreadyExists("Track", track.Name)
	}

	track.CreateTime = FixedTime()
	c.tracks[track.Name] = track
	return clone(track), nil
}

// UpdateTrack applies a patch under a field mask. (AIP-134)
func (c *Catalog) UpdateTrack(name string, patch *musicv1.Track, mask []string) (*musicv1.Track, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	track, ok := c.tracks[name]
	if !ok {
		return nil, notFound("Track", name)
	}

	if len(mask) == 0 {
		track.Title = patch.Title
		track.Duration = patch.Duration
		track.Explicit = patch.Explicit
		track.Availability = patch.Availability
	}
	for _, field := range mask {
		switch field {
		case "title":
			track.Title = patch.Title
		case "duration":
			track.Duration = patch.Duration
		case "explicit":
			track.Explicit = patch.Explicit
		case "availability":
			track.Availability = patch.Availability
		default:
			return nil, immutableField(field)
		}
	}

	c.tracks[name] = track
	return clone(track), nil
}

// DeleteTrack removes a track. (AIP-135)
func (c *Catalog) DeleteTrack(name string) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if _, ok := c.tracks[name]; !ok {
		return notFound("Track", name)
	}
	delete(c.tracks, name)
	return nil
}

// WithdrawTrack takes a track out of distribution. (AIP-136)
//
// A custom method, because it is not one of the standard five: it is neither a
// delete nor a general update, and modelling it as an update would let a client
// withdraw a track by accident with a field mask.
func (c *Catalog) WithdrawTrack(name string) (*musicv1.Track, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	track, ok := c.tracks[name]
	if !ok {
		return nil, notFound("Track", name)
	}
	if track.Availability == musicv1.Availability_AVAILABILITY_UNAVAILABLE {
		return nil, apierr.New(apierr.FailedPrecondition,
			fmt.Sprintf("Track %q is already withdrawn.", name)).
			WithErrorInfo("ALREADY_WITHDRAWN", routes.Domain, map[string]string{"resource": name})
	}

	// The proto has no WITHDRAWN value: a withdrawn track is one that is no
	// longer distributed, which is what UNAVAILABLE already means. Adding a
	// second value for it would give two spellings of one state.
	track.Availability = musicv1.Availability_AVAILABILITY_UNAVAILABLE
	c.tracks[name] = track
	return clone(track), nil
}

// WatchTracks returns the tracks a stream would emit, one per message.
//
// The example's streaming method. A real one would push changes as they happen;
// this returns the current contents, which is enough to exercise the framing and
// the no-false-2xx rule.
func (c *Catalog) WatchTracks(parent string) ([]*musicv1.Track, error) {
	return c.ListTracks(parent, 0)
}
