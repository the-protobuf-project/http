package music

// store.go is an in-memory catalog standing in for the gRPC service the adapter
// would normally call.
//
// Deliberately a plain synchronous store behind a mutex: the point of the
// example is the HTTP surface, and a real backend would only obscure whether
// that surface is correct.
//
// Errors are the same *apierr.Error a real service's status maps to, so the
// adapter's status projection is exercised rather than simulated.

import (
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/the-protobuf-project/http/examples/music-go/gateway"
	"github.com/the-protobuf-project/http/examples/music-go/gen/music/v1"
	"github.com/the-protobuf-project/http/netadapter/apierr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/durationpb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// fixedTimeRFC3339 is a constant timestamp, so responses are byte-stable across
// runs and the conformance tests mean something. A real service would use the
// clock.
const fixedTimeRFC3339 = "2026-08-24T12:00:00Z"

// FixedTime is that timestamp as the Timestamp the messages carry.
//
// A function rather than a package-level value because a protobuf message is
// mutable: handing out one shared pointer would let anything that touched a
// response rewrite every other response's create time.
func FixedTime() *timestamppb.Timestamp {
	parsed, err := time.Parse(time.RFC3339, fixedTimeRFC3339)
	if err != nil {
		// Unreachable: the constant above is a literal this file controls.
		return timestamppb.New(time.Unix(0, 0).UTC())
	}
	return timestamppb.New(parsed)
}

// Catalog is the store.
//
// Sorted iteration rather than map order, so listing is ordered by resource name
// and every response is deterministic.
type Catalog struct {
	// mu guards the whole catalog. One lock because the operations are short and
	// the contention that would justify finer locking is not what this example
	// is demonstrating.
	mu sync.Mutex

	// artists is keyed by resource name, "artists/{artist}".
	artists map[string]*musicv1.Artist

	// tracks is keyed by resource name, "artists/{artist}/tracks/{track}".
	tracks map[string]*musicv1.Track

	// revision is the monotonic source for etag values.
	revision int64
}

// NewCatalog returns an empty catalog.
func NewCatalog() *Catalog {
	return &Catalog{
		artists: map[string]*musicv1.Artist{},
		tracks:  map[string]*musicv1.Track{},
	}
}

// SeededCatalog returns a catalog with two artists and three tracks, for the
// example server and the tests.
func SeededCatalog() *Catalog {
	catalog := NewCatalog()
	catalog.seedArtist("artists/miles", "Miles Davis", 4312000)
	catalog.seedArtist("artists/coltrane", "John Coltrane", 2871003)
	catalog.seedTrack("artists/miles/tracks/so-what", "So What", 545*time.Second)
	catalog.seedTrack("artists/miles/tracks/blue-in-green", "Blue in Green", 337*time.Second)
	catalog.seedTrack("artists/coltrane/tracks/giant-steps", "Giant Steps", 286*time.Second)
	return catalog
}

// seedArtist inserts one artist during seeding.
func (c *Catalog) seedArtist(name, displayName string, listeners int64) {
	//nolint:errcheck // Seeding a fresh catalog cannot collide.
	_, _ = c.CreateArtist(&musicv1.Artist{
		Name:        name,
		DisplayName: displayName,
		// An int64 here, and a JSON string on the wire: that mapping is
		// protojson's, which is the whole reason these types are generated.
		MonthlyListeners: listeners,
	})
}

// seedTrack inserts one track during seeding.
func (c *Catalog) seedTrack(name, title string, duration time.Duration) {
	//nolint:errcheck // Seeding a fresh catalog cannot collide.
	_, _ = c.CreateTrack(parentOf(name), &musicv1.Track{
		Name:         name,
		Title:        title,
		Duration:     durationpb.New(duration),
		Availability: musicv1.Availability_AVAILABILITY_STREAMING,
	})
}

// nextEtag returns the next etag value. The caller must hold the lock.
func (c *Catalog) nextEtag() string {
	c.revision++
	return fmt.Sprintf("%q", fmt.Sprintf("%d", c.revision-1))
}

// clone returns a deep copy of a stored message.
//
// The catalog hands out copies rather than its own pointers: a protobuf message
// is mutable, and a handler that adjusted a response — or a middleware that
// redacted a field — would otherwise rewrite the stored record. The Rust store
// gets this from ownership; here it has to be deliberate.
func clone[M proto.Message](message M) M {
	copied, _ := proto.Clone(message).(M)
	return copied
}

// sortedNames returns the keys of a map in resource-name order.
//
// Generic over the value so both collections share it: listing order is part of
// the contract here, and two copies of the sort would be two chances to make it
// differ between artists and tracks.
func sortedNames[V any](items map[string]V) []string {
	names := make([]string, 0, len(items))
	for name := range items {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// parentOf returns the parent of a track's resource name, or "".
func parentOf(name string) string {
	const separator = "/tracks/"
	if index := strings.LastIndex(name, separator); index >= 0 {
		return name[:index]
	}
	return ""
}

// notFound is the error for a resource that does not exist.
//
// A ResourceInfo detail alongside the message, so a caller can act on which
// resource was missing without parsing prose.
func notFound(resourceType, name string) *apierr.Error {
	return apierr.New(apierr.NotFound, fmt.Sprintf("%s %q not found.", resourceType, name)).
		WithErrorInfo("RESOURCE_MISSING", gateway.Domain, map[string]string{"resource": name}).
		WithDetail(apierr.ResourceInfo{
			ResourceType: "music.example.com/" + resourceType,
			ResourceName: name,
			Description:  "The named resource does not exist.",
		})
}

// alreadyExists is the error for a resource that would collide with an existing
// one.
func alreadyExists(resourceType, name string) *apierr.Error {
	return apierr.New(apierr.AlreadyExists, fmt.Sprintf("%s %q already exists.", resourceType, name)).
		WithErrorInfo("RESOURCE_EXISTS", gateway.Domain, map[string]string{"resource": name})
}
