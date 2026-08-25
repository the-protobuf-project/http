package music_test

// middleware_test.go covers the message plane: that a policy runs, that it runs
// only for the methods its selector matches, and that a rejection leaves through
// the same error funnel as everything else.

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/the-protobuf-project/http/examples/music-go"
	"github.com/the-protobuf-project/http/netadapter"
	"github.com/the-protobuf-project/http/netadapter/apierr"
	"github.com/the-protobuf-project/http/netadapter/middleware"
	"github.com/the-protobuf-project/http/netadapter/middleware/builtin"
)

// recorder is an interceptor that records which methods reached it.
type recorder struct {
	// mu guards seen, since an adapter may serve concurrently.
	mu sync.Mutex

	// seen holds the fully-qualified names, in arrival order.
	seen []string
}

// Name implements [middleware.Interceptor].
func (*recorder) Name() string { return "test-recorder" }

// OnRoute implements [middleware.RouteHook].
func (r *recorder) OnRoute(cx *middleware.RouteCx) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.seen = append(r.seen, cx.Method.FullName)
	return nil
}

// names returns what the interceptor saw.
func (r *recorder) names() []string {
	r.mu.Lock()
	defer r.mu.Unlock()
	return append([]string(nil), r.seen...)
}

// serveWith runs one request against an adapter built with the given options.
func serveWith(t *testing.T, method, target string, opts ...netadapter.Option) *http.Response {
	t.Helper()

	response := httptest.NewRecorder()
	gateway := music.NewAdapter(music.SeededCatalog(), opts...)
	gateway.ServeHTTP(response, httptest.NewRequest(method, target, nil))
	return response.Result()
}

func TestMutatingSelectorFollowsTheAIPPattern(t *testing.T) {
	// The property the classification buys: the selector is written once and
	// covers every Create, Update, Delete and Undelete — including ones added to
	// the protos later. A policy written against a name prefix would miss them.
	seen := &recorder{}
	gateway := music.NewAdapter(
		music.SeededCatalog(),
		netadapter.UseFor(seen, middleware.Mutating()),
	)

	for _, request := range []struct{ method, target string }{
		{http.MethodGet, "/v1/artists/miles"},                          // Get: read-only
		{http.MethodGet, "/v1/artists"},                                // List: read-only
		{http.MethodDelete, "/v1/artists/coltrane"},                    // Delete: mutating
		{http.MethodPost, "/v1/artists/miles/tracks/so-what:withdraw"}, // Custom: mutating
	} {
		response := httptest.NewRecorder()
		gateway.ServeHTTP(response, httptest.NewRequest(request.method, request.target, strings.NewReader("{}")))
	}

	got := seen.names()
	want := []string{
		"music.v1.ArtistService.DeleteArtist",
		"music.v1.TrackService.WithdrawTrack",
	}
	if len(got) != len(want) {
		t.Fatalf("interceptor saw %v, want exactly the mutating methods %v", got, want)
	}
	for i, name := range want {
		if got[i] != name {
			t.Errorf("saw[%d] = %q, want %q", i, got[i], name)
		}
	}
}

// denyAll is an authenticator that refuses every credential.
type denyAll struct{}

// Authenticate implements [builtin.Authenticator].
func (denyAll) Authenticate(string, string) (builtin.Identity, error) {
	return builtin.Identity{}, errRejected
}

// errRejected is what denyAll returns. Its text reaches the client in
// WWW-Authenticate, so it names nothing internal.
var errRejected = apierr.New(apierr.Unauthenticated, "The access token expired.")

func TestAuthRejectionIsAWellFormedChallenge(t *testing.T) {
	// grpc-gateway sets this header to the raw status message, which violates
	// the RFC 7235 grammar as soon as a message contains a quote — and a message
	// describing a rejected token very often does.
	response := serveWith(t, http.MethodGet, "/v1/artists/miles",
		netadapter.Use(builtin.Bearer(denyAll{}, music.Domain())))

	if response.StatusCode != http.StatusUnauthorized {
		t.Fatalf("status = %d, want 401", response.StatusCode)
	}

	challenge := response.Header.Get("WWW-Authenticate")
	for _, want := range []string{`Bearer realm="music.example.com"`, `error="invalid_token"`} {
		if !strings.Contains(challenge, want) {
			t.Errorf("WWW-Authenticate = %q, missing %s", challenge, want)
		}
	}
}

// overQuota is a limiter that refuses everything.
type overQuota struct{}

// Allow implements [builtin.Limiter].
func (overQuota) Allow(string, string) (retryAfter time.Duration, allowed bool) {
	return 30 * time.Second, false
}

func TestRateLimitCarriesRetryAfter(t *testing.T) {
	// Knowing a limit was hit without knowing when to come back leaves a client
	// with nothing better to do than retry immediately, which is the behaviour
	// the limit exists to stop.
	response := serveWith(t, http.MethodGet, "/v1/artists/miles",
		netadapter.Use(builtin.NewRateLimit(overQuota{}, music.Domain())))

	if response.StatusCode != http.StatusTooManyRequests {
		t.Fatalf("status = %d, want 429", response.StatusCode)
	}
	if got := response.Header.Get("Retry-After"); got != "30" {
		t.Errorf("Retry-After = %q, want 30", got)
	}
}

func TestCORSAllowsOnlyListedOrigins(t *testing.T) {
	request := httptest.NewRequest(http.MethodGet, "/v1/artists/miles", nil)
	request.Header.Set("Origin", "https://elsewhere.example")

	response := httptest.NewRecorder()
	music.NewAdapter(
		music.SeededCatalog(),
		netadapter.Use(builtin.AllowOrigins("https://console.example")),
	).ServeHTTP(response, request)

	// Not an allowed origin: the headers are omitted rather than the request
	// rejected. The browser is what enforces this, and a 403 here would confuse
	// a non-browser client that sent an Origin for its own reasons.
	if got := response.Header().Get("Access-Control-Allow-Origin"); got != "" {
		t.Errorf("Access-Control-Allow-Origin = %q, want none for an unlisted origin", got)
	}
	if response.Code != http.StatusOK {
		t.Errorf("status = %d, want the request served normally", response.Code)
	}
}

func TestHealthAnswersBeforeRouting(t *testing.T) {
	// A health check must keep working when the route table cannot serve
	// anything else, which is precisely when a health check matters.
	health := builtin.Healthz()
	handler := health.Wrap(music.NewAdapter(music.SeededCatalog()))

	response := httptest.NewRecorder()
	handler.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/healthz", nil))

	if response.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", response.Code)
	}
	if !strings.Contains(response.Body.String(), `"SERVING"`) {
		t.Errorf("body = %q, want the health status", response.Body.String())
	}
	if got := response.Header().Get("Cache-Control"); got != "no-store" {
		t.Errorf("Cache-Control = %q, want no-store: health checks are polled constantly", got)
	}
}
