//! Projecting error details onto HTTP headers.
//!
//! Several `google.rpc` details have an HTTP counterpart that a client — or an
//! intermediary that never parses the body — can act on. The detail stays in
//! the body either way; the header is an additional projection, not a move.

use super::details::Detail;
use super::{Code, GatewayError};
use http::{HeaderMap, HeaderValue, header};

impl GatewayError {
    /// The response headers, including those projected from details.
    pub fn headers(&self) -> HeaderMap {
        let mut out = self.extra_headers.clone();

        for detail in &self.details {
            match detail {
                Detail::RetryInfo(r) => project_retry_after(&mut out, r),
                Detail::Help(h) => project_help_links(&mut out, h),
                _ => {}
            }
        }

        if self.code == Code::Unauthenticated && !out.contains_key(header::WWW_AUTHENTICATE) {
            self.project_challenge(&mut out);
        }
        out
    }

    /// Emits a well-formed `WWW-Authenticate` challenge.
    ///
    /// grpc-gateway sets this header to the raw status message, which violates
    /// the RFC 7235 grammar as soon as a message contains a quote — and a
    /// message describing a rejected token very often does.
    fn project_challenge(&self, out: &mut HeaderMap) {
        let realm = self
            .error_info()
            .map(|e| e.domain.as_str())
            .unwrap_or("api");
        let challenge = format!(
            "Bearer realm=\"{}\", error=\"invalid_token\", error_description=\"{}\"",
            quote_escape(realm),
            quote_escape(self.rendered_message()),
        );
        if let Ok(v) = HeaderValue::from_str(&challenge) {
            out.insert(header::WWW_AUTHENTICATE, v);
        }
    }
}

/// Projects `RetryInfo.retry_delay` to `Retry-After`.
///
/// The header carries whole seconds, and any fraction rounds up: rounding down
/// would invite a retry the server is still not ready for.
fn project_retry_after(out: &mut HeaderMap, retry: &super::RetryInfo) {
    let Some(delay) = &retry.retry_delay else {
        return;
    };
    let seconds = delay.seconds.max(0) + i64::from(delay.nanos > 0);
    if let Ok(v) = HeaderValue::from_str(&seconds.to_string()) {
        out.insert(header::RETRY_AFTER, v);
    }
}

/// Projects each `Help.links` entry to a `Link: <url>; rel="help"` header.
fn project_help_links(out: &mut HeaderMap, help: &super::Help) {
    for link in &help.links {
        if let Ok(v) = HeaderValue::from_str(&format!("<{}>; rel=\"help\"", link.url)) {
            out.append(header::LINK, v);
        }
    }
}

/// Escapes a string for an RFC 7235 quoted-string, dropping control characters
/// that cannot appear in a header value at all.
fn quote_escape(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .flat_map(|c| {
            if c == '"' || c == '\\' {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}
