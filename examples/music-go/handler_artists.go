package music

// handler_artists.go holds the artist method handlers.
//
// Each is the shape protoc-gen-http will emit: bind from the path, decode the
// body with the negotiated codec, call the service, encode the response.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go/gen/music/v1"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter"
)

// getArtist serves GET /v1/{name=artists/*}.
func (s *Service) getArtist(call *netadapter.Call) (*netadapter.Reply, error) {
	name, err := call.Capture("name")
	if err != nil {
		return nil, err
	}
	artist, err := s.catalog.GetArtist(name)
	if err != nil {
		return nil, err
	}
	return reply(call, artist)
}

// listArtists serves GET /v1/artists.
func (s *Service) listArtists(call *netadapter.Call) (*netadapter.Reply, error) {
	if err := call.RejectUnknownQuery("pageSize", "pageToken"); err != nil {
		return nil, err
	}
	pageSize, err := queryInt(call, "pageSize")
	if err != nil {
		return nil, err
	}

	artists, err := s.catalog.ListArtists(pageSize)
	if err != nil {
		return nil, err
	}
	return reply(call, &musicv1.ListArtistsResponse{Artists: artists})
}

// createArtist serves POST /v1/artists with body: "artist".
func (s *Service) createArtist(call *netadapter.Call) (*netadapter.Reply, error) {
	artist := &musicv1.Artist{}
	if err := decode(call, artist); err != nil {
		return nil, err
	}
	if artist.Name == "" {
		artist.Name = "artists/" + slug(artist.DisplayName)
	}

	createdArtist, err := s.catalog.CreateArtist(artist)
	if err != nil {
		return nil, err
	}
	return created(call, createdArtist, "/v1/"+createdArtist.Name)
}

// updateArtist serves PATCH /v1/{artist.name=artists/*} with body: "artist".
func (s *Service) updateArtist(call *netadapter.Call) (*netadapter.Reply, error) {
	name, err := call.Capture("artist.name")
	if err != nil {
		return nil, err
	}
	if err := call.RejectUnknownQuery("updateMask"); err != nil {
		return nil, err
	}

	patch := &musicv1.Artist{}
	if err := decode(call, patch); err != nil {
		return nil, err
	}
	updated, err := s.catalog.UpdateArtist(name, patch, updateMask(call))
	if err != nil {
		return nil, err
	}
	return reply(call, updated)
}

// deleteArtist serves DELETE /v1/{name=artists/*}.
//
// AIP-135's force decides whether child tracks go with it; without it, an artist
// that still has tracks is a FAILED_PRECONDITION rather than a silent cascade.
func (s *Service) deleteArtist(call *netadapter.Call) (*netadapter.Reply, error) {
	name, err := call.Capture("name")
	if err != nil {
		return nil, err
	}
	if err := call.RejectUnknownQuery("force"); err != nil {
		return nil, err
	}
	if err := s.catalog.DeleteArtist(name, queryBool(call, "force")); err != nil {
		return nil, err
	}

	// google.protobuf.Empty with no response_body is 204, per README §4.
	return noContent()
}

// slug derives a resource id from a display name.
//
// A server-assigned id, which is what AIP-133 expects when the client does not
// supply one. Real services use something opaque; this is readable so the
// example's URLs are.
func slug(displayName string) string {
	var out strings.Builder
	for _, r := range strings.ToLower(displayName) {
		switch {
		case r >= 'a' && r <= 'z', r >= '0' && r <= '9':
			out.WriteRune(r)
		case r == ' ' || r == '-' || r == '_':
			out.WriteByte('-')
		}
	}
	if out.Len() == 0 {
		return fmt.Sprintf("artist-%d", len(displayName))
	}
	return out.String()
}
