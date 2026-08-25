package music

// service.go is the dispatcher: the method index to a handler.
//
// This is the shape protoc-gen-http will emit per service once the Go target
// generates handlers as well as the table. Writing it by hand first fixes what
// the generator has to produce, and proves the runtime works end to end.

import (
	"github.com/the-protobuf-project/http/examples/music-go/gateway"
	"github.com/the-protobuf-project/http/netadapter"
	"github.com/the-protobuf-project/http/netadapter/apierr"
)

// Service serves the music catalog over the generated route table.
type Service struct {
	// catalog is the store behind the adapter.
	catalog *Catalog
}

// NewService returns a dispatcher over a catalog.
func NewService(catalog *Catalog) *Service { return &Service{catalog: catalog} }

// NewAdapter returns the adapter, with the middleware a deployment would run.
//
// The stack is the point of the example as much as the routing is: every policy
// below is selected by what a method means rather than by what it is called, so
// adding a method to the protos puts it in the right buckets automatically.
func NewAdapter(catalog *Catalog, opts ...netadapter.Option) *netadapter.Adapter {
	return netadapter.New(
		gateway.NewTable(),
		gateway.NewRegistry(),
		NewService(catalog),
		gateway.Domain,
		opts...,
	)
}

// Dispatch implements [netadapter.Dispatcher].
//
// A switch over generated constants rather than a lookup on a string, so a
// method that is added to the protos and not handled here is a build failure
// rather than a 500 in production.
func (s *Service) Dispatch(call *netadapter.Call) (*netadapter.Reply, error) {
	switch call.Handler {
	case gateway.MethodGetArtist:
		return s.getArtist(call)
	case gateway.MethodListArtists:
		return s.listArtists(call)
	case gateway.MethodCreateArtist:
		return s.createArtist(call)
	case gateway.MethodUpdateArtist:
		return s.updateArtist(call)
	case gateway.MethodDeleteArtist:
		return s.deleteArtist(call)
	case gateway.MethodGetTrack:
		return s.getTrack(call)
	case gateway.MethodListTracks:
		return s.listTracks(call)
	case gateway.MethodCreateTrack:
		return s.createTrack(call)
	case gateway.MethodUpdateTrack:
		return s.updateTrack(call)
	case gateway.MethodDeleteTrack:
		return s.deleteTrack(call)
	case gateway.MethodWithdrawTrack:
		return s.withdrawTrack(call)
	}

	// Reached only when the route table names a handler this switch does not,
	// which means the two were generated from different protos.
	return nil, apierr.BindingMismatch(call.Route.Template, gateway.Domain, call.Method.FullName)
}

// DispatchStream implements [netadapter.StreamDispatcher].
func (s *Service) DispatchStream(call *netadapter.Call, out *netadapter.Stream) error {
	if call.Handler == gateway.MethodWatchTracks {
		return s.watchTracks(call, out)
	}
	return apierr.BindingMismatch(call.Route.Template, gateway.Domain, call.Method.FullName)
}
