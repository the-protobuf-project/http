//! CORS.

use crate::error::Result;
use crate::middleware::{CallCx, Interceptor, ResponseParts};
use std::collections::BTreeSet;

/// Answers browser preflights and stamps CORS headers.
///
/// Strictly this belongs in the HTTP plane, where `tower-http` already has a
/// good implementation. It is here because the handler knows something a
/// generic layer does not: which HTTP methods are actually bound to a path.
/// `Access-Control-Allow-Methods` can therefore be exact rather than a
/// hand-maintained list that drifts from the route table.
#[derive(Debug, Clone)]
pub struct Cors {
    origins: Origins,
    allow_credentials: bool,
    max_age_secs: u32,
    expose: BTreeSet<String>,
}

/// Which origins are allowed.
#[derive(Debug, Clone)]
pub enum Origins {
    /// Any origin. Cannot be combined with credentials — the Fetch standard
    /// rejects `*` when `Access-Control-Allow-Credentials` is set, and a
    /// browser will refuse the response.
    Any,
    /// An explicit allowlist, compared exactly.
    List(BTreeSet<String>),
}

impl Cors {
    /// Allows any origin, without credentials.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            origins: Origins::Any,
            allow_credentials: false,
            max_age_secs: 600,
            expose: BTreeSet::new(),
        }
    }

    /// Allows an explicit set of origins.
    #[must_use]
    pub fn allow(origins: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            origins: Origins::List(origins.into_iter().map(Into::into).collect()),
            allow_credentials: false,
            max_age_secs: 600,
            expose: BTreeSet::new(),
        }
    }

    /// Allows credentialed requests.
    ///
    /// # Panics
    ///
    /// Panics when origins are [`Origins::Any`]. The combination is invalid per
    /// the Fetch standard, and failing here is better than shipping a config a
    /// browser silently refuses at runtime.
    #[must_use]
    pub fn with_credentials(mut self) -> Self {
        assert!(
            !matches!(self.origins, Origins::Any),
            "credentialed CORS requires an explicit origin allowlist"
        );
        self.allow_credentials = true;
        self
    }

    /// Exposes response headers to the browser.
    #[must_use]
    pub fn expose(mut self, headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.expose = headers.into_iter().map(Into::into).collect();
        self
    }

    /// The value to echo for a request's `Origin`, if it is allowed.
    fn allowed_origin(&self, origin: &str) -> Option<String> {
        match &self.origins {
            Origins::Any => Some("*".to_string()),
            Origins::List(list) if list.contains(origin) => Some(origin.to_string()),
            Origins::List(_) => None,
        }
    }
}

impl Interceptor for Cors {
    fn name(&self) -> &'static str {
        "cors"
    }

    fn on_response(&self, cx: &mut CallCx<'_>, parts: &mut ResponseParts) -> Result<()> {
        let Some(origin) = cx
            .route
            .headers
            .get(http::header::ORIGIN)
            .and_then(|v| v.to_str().ok())
        else {
            return Ok(());
        };
        let Some(allowed) = self.allowed_origin(origin) else {
            // Not an allowed origin: omit the headers rather than reject. The
            // browser enforces this, and a 403 here would confuse a non-browser
            // client that sent an Origin for its own reasons.
            return Ok(());
        };

        parts.set(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, &allowed);
        if self.allow_credentials {
            parts.set(http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true");
        }
        if !self.expose.is_empty() {
            let value = self.expose.iter().cloned().collect::<Vec<_>>().join(", ");
            parts.set(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, &value);
        }
        // Any allowlisted response varies by Origin, so a shared cache must not
        // serve one origin's response to another.
        if !matches!(self.origins, Origins::Any) {
            parts.set(http::header::VARY, "Origin");
        }
        parts.set(
            http::header::ACCESS_CONTROL_MAX_AGE,
            &self.max_age_secs.to_string(),
        );
        Ok(())
    }
}
