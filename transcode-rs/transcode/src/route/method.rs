//! The method table: what each handler index names.

use crate::middleware::MethodPattern;

/// One method of one service, as the generator classified it.
///
/// A matched [`Route`](super::Route) carries only a handler index, because the
/// route table is a positional scan and has no use for names. Everything that
/// runs *around* a call — a tracing span, a selector, an `ErrorInfo` — needs to
/// know which method it is, and this is where it finds out.
///
/// Emitted by `protoc-gen-http` as a `static` slice in handler-index order, so
/// `METHODS[route.handler]` is the method a route serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodInfo {
    /// The bare method name, e.g. `GetArtist`.
    pub name: &'static str,
    /// The fully-qualified method name, e.g. `music.v1.ArtistService.GetArtist`.
    pub full_name: &'static str,
    /// The fully-qualified service name, e.g. `music.v1.ArtistService`.
    pub service: &'static str,
    /// The AIP classification, which is what a selector dispatches on.
    pub pattern: MethodPattern,
    /// Whether the method changes state.
    pub mutating: bool,
    /// Whether the method streams its response.
    ///
    /// There is no client-stream counterpart: the generator rejects a
    /// `google.api.http` rule on a client-streaming method, because HTTP has no
    /// honest mapping for one.
    pub server_stream: bool,
}
