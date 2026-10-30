//! What generated code implements to serve resolved calls.

use super::call::Call;
use super::reply::Reply;
use super::stream::Stream;
use crate::error::{Code, Result};
use std::future::Future;

/// Serves resolved calls. Generated code implements it by matching on
/// [`Call::handler`] and calling the service.
///
/// Both methods are `async`, because the service they call is: an
/// implementation may write `async fn` for either.
pub trait Dispatcher: Send + Sync + 'static {
    /// Serves one unary call, returning a rendered reply or the failure.
    ///
    /// The response phase runs on the reply before anything is written, so an
    /// interceptor can still change its status — and one that rejects produces
    /// an error response rather than a half-written success.
    fn dispatch(&self, call: &Call<'_>) -> impl Future<Output = Result<Reply>> + Send;

    /// Serves one server-streaming call, writing messages through `out` and
    /// returning the failure that ended it, if any.
    ///
    /// Optional: the default rejects with `UNIMPLEMENTED` rather than serving a
    /// streaming method wrongly. Because nothing has been written by then, that
    /// rejection carries its real status.
    fn dispatch_stream(
        &self,
        call: &Call<'_>,
        out: &mut Stream<'_>,
    ) -> impl Future<Output = Result<()>> + Send {
        let _ = out;
        let err = call.error(
            Code::Unimplemented,
            "This method streams its response, which this transcoder was not built to serve.",
        );
        async move { Err(err) }
    }
}
