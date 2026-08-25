package middleware_test

// stack_test.go covers selection: which interceptors run for which methods, and
// the mistakes the stack refuses to accept.

import (
	"testing"

	"github.com/the-protobuf-project/http/netadapter/middleware"
	"github.com/the-protobuf-project/http/netadapter/route"
)

// hook implements every phase, so a selection test can see it in each.
type hook struct{ name string }

// Name implements [middleware.Interceptor].
func (h hook) Name() string { return h.name }

// OnRoute implements [middleware.RouteHook].
func (hook) OnRoute(*middleware.RouteCx) error { return nil }

// OnComplete implements [middleware.CompleteHook].
func (hook) OnComplete(*middleware.CallCx, middleware.Outcome) {}

// routeOnly implements one phase, to prove the others stay empty.
type routeOnly struct{}

// Name implements [middleware.Interceptor].
func (routeOnly) Name() string { return "route-only" }

// OnRoute implements [middleware.RouteHook].
func (routeOnly) OnRoute(*middleware.RouteCx) error { return nil }

// phaseless implements no phase at all, which the stack must reject.
type phaseless struct{}

// Name implements [middleware.Interceptor].
func (phaseless) Name() string { return "phaseless" }

var (
	getArtist = route.Method{
		Name: "GetArtist", FullName: "music.v1.ArtistService.GetArtist",
		Service: "music.v1.ArtistService", Pattern: route.PatternGet,
	}
	createArtist = route.Method{
		Name: "CreateArtist", FullName: "music.v1.ArtistService.CreateArtist",
		Service: "music.v1.ArtistService", Pattern: route.PatternCreate, Mutating: true,
	}
	getTrack = route.Method{
		Name: "GetTrack", FullName: "music.v1.TrackService.GetTrack",
		Service: "music.v1.TrackService", Pattern: route.PatternGet,
	}
)

func TestMutatingSelectorFollowsThePattern(t *testing.T) {
	// The property the AIP classification buys: a policy written once covers
	// every state-changing method, including ones added later.
	stack := middleware.NewStack()
	stack.UseFor(hook{"auth"}, middleware.Mutating())

	if got := len(stack.For(createArtist).Route); got != 1 {
		t.Errorf("Create saw %d route hooks, want 1", got)
	}
	if got := len(stack.For(getArtist).Route); got != 0 {
		t.Errorf("Get saw %d route hooks, want 0", got)
	}
}

func TestSelectorsCompose(t *testing.T) {
	stack := middleware.NewStack()
	stack.UseFor(hook{"scoped"}, middleware.Every(
		middleware.Service("music.v1.ArtistService"),
		middleware.ReadOnly(),
	))

	if got := len(stack.For(getArtist).Route); got != 1 {
		t.Errorf("read-only artist method saw %d hooks, want 1", got)
	}
	for _, method := range []route.Method{createArtist, getTrack} {
		if got := len(stack.For(method).Route); got != 0 {
			t.Errorf("%s saw %d hooks, want 0", method.Name, got)
		}
	}
}

func TestNotInvertsASelector(t *testing.T) {
	stack := middleware.NewStack()
	stack.UseFor(hook{"everything-else"}, middleware.Not(middleware.Method(getArtist.FullName)))

	if got := len(stack.For(getArtist).Route); got != 0 {
		t.Errorf("the excluded method saw %d hooks, want 0", got)
	}
	if got := len(stack.For(getTrack).Route); got != 1 {
		t.Errorf("another method saw %d hooks, want 1", got)
	}
}

func TestOnlyImplementedPhasesAreCollected(t *testing.T) {
	stack := middleware.NewStack()
	stack.Use(routeOnly{})

	selected := stack.For(getArtist)
	if len(selected.Route) != 1 {
		t.Errorf("route hooks = %d, want 1", len(selected.Route))
	}
	if len(selected.Request)+len(selected.Response)+len(selected.Complete) != 0 {
		t.Error("a route-only interceptor was collected into another phase")
	}
}

func TestRegistrationOrderIsPreserved(t *testing.T) {
	// Order is registration order in every phase, including the response ones.
	// Reversing on the way out would be the wrapper convention, and it is wrong
	// here: a reader tracing an audit log should not have to invert the list.
	stack := middleware.NewStack()
	stack.Use(hook{"first"})
	stack.Use(hook{"second"})

	selected := stack.For(getArtist)
	if selected.Route[0].Name() != "first" || selected.Route[1].Name() != "second" {
		t.Errorf("order = %v, want registration order", stack.Names())
	}
	if selected.Complete[0].Name() != "first" {
		t.Error("the completion phase reversed the stack")
	}
}

func TestAnInterceptorWithNoPhasePanics(t *testing.T) {
	// A policy that can never run is a mistake that is otherwise silent: a
	// misspelled OnRoute compiles fine and simply does nothing.
	defer func() {
		if recover() == nil {
			t.Error("a phaseless interceptor was accepted")
		}
	}()
	middleware.NewStack().Use(phaseless{})
}

func TestEmptySelectionIsReportedAsEmpty(t *testing.T) {
	if !middleware.NewStack().For(getArtist).Empty() {
		t.Error("an empty stack did not report an empty selection")
	}
}
