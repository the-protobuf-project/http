//! Authentication.

use crate::error::{Code, GatewayError, Result};
use crate::middleware::{Interceptor, RouteCx};
use std::sync::Arc;

/// Who the caller is, once authenticated.
///
/// Placed in [`RouteCx::extensions`] so an authorizer, an audit log, or a rate
/// limiter can read it without any of them knowing how authentication happened.
///
/// [`RouteCx::extensions`]: crate::middleware::RouteCx::extensions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// A stable identifier for the caller.
    pub subject: String,
    /// The scopes or roles the credential carries.
    pub scopes: Vec<String>,
    /// Who issued the credential.
    pub issuer: String,
}

impl Identity {
    /// Whether the caller holds a scope.
    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

/// Verifies a credential.
///
/// Deliberately not a JWT verifier: token formats and key rotation are a
/// deployment's concern, and baking one in would make the common case easy and
/// every other case impossible.
pub trait Authenticator: Send + Sync + 'static {
    /// Verifies a credential and returns the identity it proves.
    ///
    /// # Errors
    ///
    /// A description of why the credential was rejected. It reaches the client
    /// in `WWW-Authenticate`, so it must not name internal state.
    fn authenticate(&self, scheme: &str, credential: &str)
    -> std::result::Result<Identity, String>;
}

/// Requires a credential before a call proceeds.
///
/// Runs in `on_route`, so a rejection costs nothing: no body has been read and
/// no message decoded.
#[derive(Clone)]
pub struct Auth {
    scheme: &'static str,
    authenticator: Arc<dyn Authenticator>,
    domain: &'static str,
}

impl Auth {
    /// Requires a bearer token.
    #[must_use]
    pub fn bearer(authenticator: impl Authenticator, domain: &'static str) -> Self {
        Self {
            scheme: "Bearer",
            authenticator: Arc::new(authenticator),
            domain,
        }
    }

    /// Requires a credential under a named scheme.
    #[must_use]
    pub fn with_scheme(
        scheme: &'static str,
        authenticator: impl Authenticator,
        domain: &'static str,
    ) -> Self {
        Self {
            scheme,
            authenticator: Arc::new(authenticator),
            domain,
        }
    }

    /// Builds the `401`, which the error model turns into a well-formed
    /// `WWW-Authenticate` challenge.
    fn unauthenticated(&self, reason: &str) -> Box<GatewayError> {
        Box::new(
            GatewayError::new(Code::Unauthenticated, reason.to_string()).with_error_info(
                "CREDENTIAL_INVALID",
                self.domain,
                [("scheme".into(), self.scheme.to_string())],
            ),
        )
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("scheme", &self.scheme)
            .finish()
    }
}

impl Interceptor for Auth {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        let header = cx
            .headers
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| self.unauthenticated("No credentials were supplied."))?;

        let (scheme, credential) = header
            .split_once(' ')
            .ok_or_else(|| self.unauthenticated("Malformed Authorization header."))?;

        // Scheme comparison is case-insensitive per RFC 9110 §11.1.
        if !scheme.eq_ignore_ascii_case(self.scheme) {
            return Err(self.unauthenticated("Unsupported authentication scheme."));
        }

        let identity = self
            .authenticator
            .authenticate(scheme, credential.trim())
            .map_err(|reason| self.unauthenticated(&reason))?;

        cx.extensions.insert(identity);
        Ok(())
    }
}
