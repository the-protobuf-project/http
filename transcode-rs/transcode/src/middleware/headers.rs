//! Header/metadata name mapping.
//!
//! Three matchers, one per direction, matching grpc-gateway's
//! `WithIncomingHeaderMatcher`, `WithOutgoingHeaderMatcher`, and
//! `WithOutgoingTrailerMatcher`. Each answers one question: given a name on one
//! side, what name does it take on the other, and does it cross at all?

use std::sync::Arc;

/// The prefix a client uses to send arbitrary gRPC metadata.
///
/// `Grpc-Metadata-Foo: bar` arrives at the service as metadata `foo: bar`.
pub const METADATA_HEADER_PREFIX: &str = "grpc-metadata-";

/// The prefix a permanent HTTP header takes on the way in.
///
/// `Accept-Language` becomes `grpcgateway-accept-language`, which keeps a
/// header the transport owns from colliding with a metadata key the service
/// defines.
pub const METADATA_PREFIX: &str = "grpcgateway-";

/// The prefix a gRPC trailer takes on the way out.
pub const METADATA_TRAILER_PREFIX: &str = "Grpc-Trailer-";

/// Suffix marking base64-encoded binary metadata.
pub const BINARY_SUFFIX: &str = "-bin";

/// Decides whether a name crosses between HTTP and gRPC, and under what name.
///
/// Returns the translated name, or `None` to drop it.
pub type MatcherFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A named header matcher.
#[derive(Clone)]
pub struct HeaderMatcher {
    name: &'static str,
    matcher: MatcherFn,
}

impl HeaderMatcher {
    /// Builds a matcher from a closure.
    pub fn new(
        name: &'static str,
        matcher: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            matcher: Arc::new(matcher),
        }
    }

    /// Translates one name, or `None` to drop it.
    #[must_use]
    pub fn translate(&self, key: &str) -> Option<String> {
        (self.matcher)(key)
    }
}

impl std::fmt::Debug for HeaderMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderMatcher")
            .field("name", &self.name)
            .finish()
    }
}

/// The three matchers a handler uses.
#[derive(Clone, Debug)]
pub struct Headers {
    /// HTTP request header to gRPC metadata.
    pub incoming: HeaderMatcher,
    /// gRPC response metadata to HTTP response header.
    pub outgoing: HeaderMatcher,
    /// gRPC trailer to HTTP trailer.
    pub trailer: HeaderMatcher,
}

impl Default for Headers {
    fn default() -> Self {
        Self {
            incoming: HeaderMatcher::new("default-incoming", default_incoming),
            outgoing: HeaderMatcher::new("default-outgoing", default_outgoing),
            trailer: HeaderMatcher::new("default-trailer", default_trailer),
        }
    }
}

/// Headers that must not be forwarded, because they describe *this* hop.
///
/// RFC 9110 §7.6.1. Forwarding `Connection` or `Transfer-Encoding` to a service
/// would describe a connection the service is not on.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Permanent HTTP headers, which are prefixed rather than passed through.
///
/// These belong to HTTP itself, so a service asking for metadata `host` should
/// not silently receive the transport's `Host`.
const PERMANENT: &[&str] = &[
    "accept",
    "accept-charset",
    "accept-encoding",
    "accept-language",
    "accept-ranges",
    "authorization",
    "cache-control",
    "content-type",
    "cookie",
    "date",
    "expect",
    "from",
    "host",
    "if-match",
    "if-modified-since",
    "if-none-match",
    "if-schedule-tag-match",
    "if-unmodified-since",
    "max-forwards",
    "origin",
    "pragma",
    "referer",
    "user-agent",
    "warning",
    "via",
];

/// The default incoming rule.
///
/// `Grpc-Metadata-Foo` loses its prefix; a permanent header gains
/// `grpcgateway-`; anything else passes through lowercased. Hop-by-hop headers
/// are dropped.
#[must_use]
pub fn default_incoming(key: &str) -> Option<String> {
    let lower = key.to_ascii_lowercase();

    if HOP_BY_HOP.contains(&lower.as_str()) {
        return None;
    }
    if let Some(rest) = lower.strip_prefix(METADATA_HEADER_PREFIX) {
        return Some(rest.to_string());
    }
    if PERMANENT.contains(&lower.as_str()) {
        return Some(format!("{METADATA_PREFIX}{lower}"));
    }
    Some(lower)
}

/// The default outgoing rule: every metadata key gains `Grpc-Metadata-`.
#[must_use]
pub fn default_outgoing(key: &str) -> Option<String> {
    Some(format!("Grpc-Metadata-{key}"))
}

/// The default trailer rule: every trailer key gains `Grpc-Trailer-`.
#[must_use]
pub fn default_trailer(key: &str) -> Option<String> {
    Some(format!("{METADATA_TRAILER_PREFIX}{key}"))
}

/// Whether a metadata key carries base64 binary rather than text.
#[must_use]
pub fn is_binary(key: &str) -> bool {
    key.to_ascii_lowercase().ends_with(BINARY_SUFFIX)
}
