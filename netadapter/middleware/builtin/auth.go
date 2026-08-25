package builtin

// auth.go requires a credential before a call proceeds.

import (
	"strings"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
)

// identityKey is the context key the resolved identity is stored under.
//
// An unexported empty struct type, so nothing outside this package can collide
// with it or forge an identity by writing the same key.
type identityKey struct{}

// Identity is who the caller is, once authenticated.
//
// Placed on the call context so an authorizer, an audit log or a rate limiter
// can read it without any of them knowing how authentication happened.
type Identity struct {
	// Subject is a stable identifier for the caller.
	Subject string

	// Scopes are the scopes or roles the credential carries.
	Scopes []string

	// Issuer is who issued the credential.
	Issuer string
}

// HasScope reports whether the caller holds a scope.
func (i Identity) HasScope(scope string) bool {
	for _, held := range i.Scopes {
		if held == scope {
			return true
		}
	}
	return false
}

// IdentityFrom returns the authenticated identity on a routing context.
func IdentityFrom(cx *middleware.RouteCx) (Identity, bool) {
	identity, ok := cx.Ctx.Value(identityKey{}).(Identity)
	return identity, ok
}

// Authenticator verifies a credential.
//
// Deliberately not a JWT verifier: token formats and key rotation are a
// deployment's concern, and baking one in would make the common case easy and
// every other case impossible.
type Authenticator interface {
	// Authenticate verifies a credential and returns the identity it proves.
	//
	// The error reaches the client in WWW-Authenticate, so it must not name
	// internal state.
	Authenticate(scheme, credential string) (Identity, error)
}

// Auth requires a credential before a call proceeds.
//
// Runs in the route phase, so a rejection costs nothing: no body has been read
// and no message decoded.
type Auth struct {
	// scheme is the authentication scheme required, e.a. "Bearer".
	scheme string

	// authenticator verifies the credential.
	authenticator Authenticator

	// domain is the API's error domain.
	domain string
}

// Bearer requires a bearer token.
func Bearer(authenticator Authenticator, domain string) *Auth {
	return &Auth{scheme: "Bearer", authenticator: authenticator, domain: domain}
}

// WithScheme requires a credential under a named scheme.
func WithScheme(scheme string, authenticator Authenticator, domain string) *Auth {
	return &Auth{scheme: scheme, authenticator: authenticator, domain: domain}
}

// Name implements [middleware.Interceptor].
func (*Auth) Name() string { return "auth" }

// OnRoute implements [middleware.RouteHook].
func (a *Auth) OnRoute(cx *middleware.RouteCx) error {
	header := cx.Request.Header.Get("Authorization")
	if header == "" {
		return a.unauthenticated("No credentials were supplied.")
	}

	scheme, credential, found := strings.Cut(header, " ")
	if !found {
		return a.unauthenticated("Malformed Authorization header.")
	}
	// Scheme comparison is case-insensitive per RFC 9110 §11.1.
	if !strings.EqualFold(scheme, a.scheme) {
		return a.unauthenticated("Unsupported authentication scheme.")
	}

	identity, err := a.authenticator.Authenticate(scheme, strings.TrimSpace(credential))
	if err != nil {
		return a.unauthenticated(err.Error())
	}

	cx.Set(identityKey{}, identity)
	return nil
}

// unauthenticated builds the 401, which the error model turns into a
// well-formed WWW-Authenticate challenge.
func (a *Auth) unauthenticated(reason string) error {
	return apierr.New(apierr.Unauthenticated, reason).
		WithErrorInfo("CREDENTIAL_INVALID", a.domain, map[string]string{"scheme": a.scheme})
}
