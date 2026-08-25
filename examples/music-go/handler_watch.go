package music

// handler_watch.go holds the streaming method, and is where the no-false-2xx
// rule is actually exercised.

import (
	"github.com/the-protobuf-project/http/examples/music-go/gen/music/v1"
	"github.com/the-protobuf-project/http/transcode-go"
	"github.com/the-protobuf-project/http/transcode-go/apierr"
)

// FailAfter is a query parameter the example honours to fail a stream
// deliberately, so the two halves of README §6.2 can both be exercised.
//
// It exists for the conformance tests: ?failAfter=0 fails before any message
// and must produce a real status with an ordinary error body, while
// ?failAfter=2 fails once the status line is spent and must produce an error
// frame, trailers, and a truncated body. A real service would have no such
// parameter — but a real service also cannot be asked to fail on cue, and a
// rule that is only tested by unit tests is a rule no transport is held to.
const FailAfter = "failAfter"

// watchTracks serves GET /v1/{parent=artists/*}/tracks:watch.
//
// The stream's headers are not written when it opens: they go out with the
// first Send. A failure before then — a bad parent, an authorization refusal —
// therefore still gets its real status and an ordinary error body.
func (s *Service) watchTracks(call *transcode.Call, out *transcode.Stream) error {
	parent, err := call.Capture("parent")
	if err != nil {
		return err
	}
	if err := call.RejectUnknownQuery(FailAfter); err != nil {
		return err
	}

	tracks, err := s.catalog.WatchTracks(parent)
	if err != nil {
		return err
	}

	failAfter, err := queryInt(call, FailAfter)
	if err != nil {
		return err
	}
	fails := call.Query.Has(FailAfter)

	for i, track := range tracks {
		if fails && i >= failAfter {
			return call.Errorf(apierr.Unavailable, "The catalog became unavailable mid-stream.")
		}
		encoded, err := marshal.Marshal(&musicv1.WatchTracksResponse{Track: track})
		if err != nil {
			return encodeFailed(call)
		}
		if err := out.Send(encoded); err != nil {
			// The peer is gone. There is nobody left to report to, and the
			// transcoder cancels the underlying RPC on its own.
			return nil
		}
	}

	if fails {
		return call.Errorf(apierr.Unavailable, "The catalog became unavailable mid-stream.")
	}
	return nil
}
