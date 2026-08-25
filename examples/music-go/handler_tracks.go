package music

// handler_tracks.go holds the track method handlers.

import (
	"github.com/the-protobuf-project/http/examples/music-go/gen/music/v1"
	"github.com/the-protobuf-project/http/transcode-go"
)

// getTrack serves GET /v1/{name=artists/*/tracks/*}.
func (s *Service) getTrack(call *transcode.Call) (*transcode.Reply, error) {
	name, err := call.Capture("name")
	if err != nil {
		return nil, err
	}
	track, err := s.catalog.GetTrack(name)
	if err != nil {
		return nil, err
	}
	return reply(call, track)
}

// listTracks serves GET /v1/{parent=artists/*}/tracks.
func (s *Service) listTracks(call *transcode.Call) (*transcode.Reply, error) {
	parent, err := call.Capture("parent")
	if err != nil {
		return nil, err
	}
	if err := call.RejectUnknownQuery("pageSize", "pageToken"); err != nil {
		return nil, err
	}
	pageSize, err := queryInt(call, "pageSize")
	if err != nil {
		return nil, err
	}

	tracks, err := s.catalog.ListTracks(parent, pageSize)
	if err != nil {
		return nil, err
	}
	return reply(call, &musicv1.ListTracksResponse{Tracks: tracks})
}

// createTrack serves POST /v1/{parent=artists/*}/tracks with body: "track".
func (s *Service) createTrack(call *transcode.Call) (*transcode.Reply, error) {
	parent, err := call.Capture("parent")
	if err != nil {
		return nil, err
	}
	track := &musicv1.Track{}
	if err := decode(call, track); err != nil {
		return nil, err
	}
	if track.Name == "" {
		track.Name = parent + "/tracks/" + slug(track.Title)
	}

	createdTrack, err := s.catalog.CreateTrack(parent, track)
	if err != nil {
		return nil, err
	}
	return created(call, createdTrack, "/v1/"+createdTrack.Name)
}

// updateTrack serves PATCH /v1/{track.name=artists/*/tracks/*} with
// body: "track".
func (s *Service) updateTrack(call *transcode.Call) (*transcode.Reply, error) {
	name, err := call.Capture("track.name")
	if err != nil {
		return nil, err
	}
	if err := call.RejectUnknownQuery("updateMask"); err != nil {
		return nil, err
	}

	patch := &musicv1.Track{}
	if err := decode(call, patch); err != nil {
		return nil, err
	}
	updated, err := s.catalog.UpdateTrack(name, patch, updateMask(call))
	if err != nil {
		return nil, err
	}
	return reply(call, updated)
}

// deleteTrack serves DELETE /v1/{name=artists/*/tracks/*}.
func (s *Service) deleteTrack(call *transcode.Call) (*transcode.Reply, error) {
	name, err := call.Capture("name")
	if err != nil {
		return nil, err
	}
	if err := s.catalog.DeleteTrack(name); err != nil {
		return nil, err
	}
	return noContent()
}

// withdrawTrack serves POST /v1/{name=artists/*/tracks/*}:withdraw with
// body: "*".
//
// The custom-verb route, which is the one a general-purpose HTTP router cannot
// express: it would accept the template and silently bind ":withdraw" into the
// resource name.
func (s *Service) withdrawTrack(call *transcode.Call) (*transcode.Reply, error) {
	name, err := call.Capture("name")
	if err != nil {
		return nil, err
	}
	track, err := s.catalog.WithdrawTrack(name)
	if err != nil {
		return nil, err
	}
	return reply(call, track)
}
