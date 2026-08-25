//! The built-in interceptors.

use super::fixtures::{DOMAIN, Fixture};
use crate::error::Code;
use crate::middleware::builtin::{Auth, Authenticator, Identity};
use crate::middleware::{Interceptor, MethodPattern};

/// Accepts one token and nothing else.
struct OneToken;

impl Authenticator for OneToken {
    fn authenticate(&self, _: &str, credential: &str) -> Result<Identity, String> {
        if credential == "good" {
            Ok(Identity {
                subject: "user:1".into(),
                scopes: vec!["music.read".into()],
                issuer: "test".into(),
            })
        } else {
            Err("The access token is invalid.".into())
        }
    }
}

#[test]
fn auth_publishes_the_identity_for_later_interceptors() {
    let fixture = Fixture::get().header("authorization", "Bearer good");
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);

    Auth::bearer(OneToken, DOMAIN).on_route(&mut cx).unwrap();

    let identity = cx.extensions.get::<Identity>().expect("identity");
    assert_eq!(identity.subject, "user:1");
    assert!(identity.has_scope("music.read"));
}

#[test]
fn auth_rejects_a_missing_or_bad_credential_with_401() {
    let auth = Auth::bearer(OneToken, DOMAIN);

    let none = Fixture::get();
    let err = auth
        .on_route(&mut none.route("GetArtist", MethodPattern::Get))
        .unwrap_err();
    assert_eq!(err.code, Code::Unauthenticated);

    let bad = Fixture::get().header("authorization", "Bearer nope");
    let err = auth
        .on_route(&mut bad.route("GetArtist", MethodPattern::Get))
        .unwrap_err();
    assert_eq!(err.http.as_u16(), 401);
    // The error model turns this into a well-formed challenge.
    assert!(err.headers().contains_key(http::header::WWW_AUTHENTICATE));
}

#[test]
fn auth_scheme_comparison_is_case_insensitive() {
    // RFC 9110 §11.1 makes the scheme token case-insensitive.
    let fixture = Fixture::get().header("authorization", "bearer good");
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);
    Auth::bearer(OneToken, DOMAIN).on_route(&mut cx).unwrap();
    assert!(cx.extensions.get::<Identity>().is_some());
}
