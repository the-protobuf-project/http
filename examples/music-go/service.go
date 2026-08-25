package music

// service.go is the dispatcher: the method index to a handler.
//
// This is the shape protoc-gen-http will emit per service once the Go target
// generates handlers as well as the table. Writing it by hand first fixes what
// the generator has to produce, and proves the runtime works end to end.

import (
	"github.com/the-protobuf-project/http/examples/music-go/routes"
	"github.com/the-protobuf-project/http/transcode-go"
	"github.com/the-protobuf-project/http/transcode-go/apierr"
)

// Service serves the music catalog over the generated route table.
type Service struct {
	// catalog is the store behind the transcoder.
	catalog *Catalog
}

// NewService returns a dispatcher over a catalog.
func NewService(catalog *Catalog) *Service { return &Service{catalog: catalog} }

// NewHandler returns the handler, with the middleware a deployment would run.
//
// The stack is the point of the example as much as the routing is: every policy
// below is selected by what a method means rather than by what it is called, so
// adding a method to the protos puts it in the right buckets automatically.
func NewHandler(catalog *Catalog, opts ...transcode.Option) *transcode.Handler {
	return transcode.New(
		routes.NewTable(),
		routes.NewRegistry(),
		NewService(catalog),
		routes.Domain,
		opts...,
	)
}

// Dispatch implements [transcode.Dispatcher].
//
// A switch over generated constants rather than a lookup on a string, so a
// method that is added to the protos and not handled here is a build failure
// rather than a 500 in production.
func (s *Service) Dispatch(call *transcode.Call) (*transcode.Reply, error) {
	switch call.Handler {
	case routes.MethodGetArtist:
		return s.getArtist(call)
	case routes.MethodListArtists:
		return s.listArtists(call)
	case routes.MethodCreateArtist:
		return s.createArtist(call)
	case routes.MethodUpdateArtist:
		return s.updateArtist(call)
	case routes.MethodDeleteArtist:
		return s.deleteArtist(call)
	case routes.MethodGetTrack:
		return s.getTrack(call)
	case routes.MethodListTracks:
		return s.listTracks(call)
	case routes.MethodCreateTrack:
		return s.createTrack(call)
	case routes.MethodUpdateTrack:
		return s.updateTrack(call)
	case routes.MethodDeleteTrack:
		return s.deleteTrack(call)
	case routes.MethodWithdrawTrack:
		return s.withdrawTrack(call)
	}

	// Reached only when the route table names a handler this switch does not,
	// which means the two were generated from different protos.
	return nil, apierr.BindingMismatch(call.Route.Template, routes.Domain, call.Method.FullName)
}

// DispatchStream implements [transcode.StreamDispatcher].
func (s *Service) DispatchStream(call *transcode.Call, out *transcode.Stream) error {
	if call.Handler == routes.MethodWatchTracks {
		return s.watchTracks(call, out)
	}
	return apierr.BindingMismatch(call.Route.Template, routes.Domain, call.Method.FullName)
}
