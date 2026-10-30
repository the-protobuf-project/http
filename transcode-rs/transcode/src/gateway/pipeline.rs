//! The request pipeline: route, bind, run the middleware phases, dispatch,
//! render.

use super::body::{GatewayBody, Sink, respond};
use super::call::Call;
use super::dispatch::Dispatcher;
use super::query::Query;
use super::reply::{Reply, encode_error};
use super::stream::Stream;
use crate::codec::{CodecRegistry, Negotiation, request_codec, response_codec};
use crate::error::Error;
use crate::middleware::builtin::Health;
use crate::middleware::{
    CallCx, DefaultErrorRenderer, ErrorRenderer, Extensions, Headers, Interceptor, Metadata,
    MetadataAnnotator, Options, Outcome, ResponseParts, RouteCx, Selector, Stack,
    parse_grpc_timeout,
};
use crate::route::{Resolution, RouteTable};
use crate::stream::Termination;
use bytes::Bytes;
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use http::{HeaderMap, Request, Response, header};
use http_body_util::{BodyExt, LengthLimitError, Limited};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

/// Serves a generated route table over HTTP.
///
/// It is a [`tower::Service`] over [`http::Request`] and nothing more: it owns
/// no listener and no lifecycle, so it mounts into an existing router — axum,
/// a hybrid gRPC server, a bare hyper connection — and inherits whatever the
/// deployment already does about TLS, timeouts and shutdown.
///
/// Every failure — routing, negotiation, binding, the RPC's own status — leaves
/// through one funnel, the [`ErrorRenderer`], so a 404 and a mid-call
/// `PERMISSION_DENIED` produce the same envelope shape. That single funnel is
/// the structural fix for grpc-gateway rendering its three error paths
/// differently.
///
/// [`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
pub struct Gateway<D> {
    inner: Arc<Inner<D>>,
}

impl<D> Clone for Gateway<D> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<D> std::fmt::Debug for Gateway<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway")
            .field("domain", &self.inner.domain)
            .field("routes", &self.inner.table.routes().len())
            .field("interceptors", &self.inner.stack.names())
            .finish_non_exhaustive()
    }
}

struct Inner<D> {
    table: RouteTable,
    codecs: CodecRegistry,
    dispatch: D,
    domain: &'static str,
    stack: Stack,
    headers: Headers,
    annotators: Vec<Arc<dyn MetadataAnnotator>>,
    renderer: Arc<dyn ErrorRenderer>,
    options: Options,
    health: Option<Health>,
}

/// Configures a [`Gateway`]. Every setting has a default, so
/// [`GatewayBuilder::build`] can be called straight away.
pub struct GatewayBuilder<D> {
    inner: Inner<D>,
}

impl<D> std::fmt::Debug for GatewayBuilder<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayBuilder")
            .field("domain", &self.inner.domain)
            .finish_non_exhaustive()
    }
}

impl<D: Dispatcher> Gateway<D> {
    /// A gateway with every default.
    ///
    /// `table` should be built with [`RouteTable::with_methods`]: a route the
    /// method table does not cover is answered with a `500 BINDING_MISMATCH`.
    pub fn new(
        table: RouteTable,
        codecs: CodecRegistry,
        dispatch: D,
        domain: &'static str,
    ) -> Self {
        Self::builder(table, codecs, dispatch, domain).build()
    }

    /// Starts configuring a gateway.
    pub fn builder(
        table: RouteTable,
        codecs: CodecRegistry,
        dispatch: D,
        domain: &'static str,
    ) -> GatewayBuilder<D> {
        GatewayBuilder {
            inner: Inner {
                table,
                codecs,
                dispatch,
                domain,
                stack: Stack::new(),
                headers: Headers::default(),
                annotators: Vec::new(),
                renderer: Arc::new(DefaultErrorRenderer),
                options: Options::new(domain),
                health: None,
            },
        }
    }

    /// The route table, for a test asserting on resolution directly.
    pub fn table(&self) -> &RouteTable {
        &self.inner.table
    }

    /// The dispatcher calls are served by.
    pub fn dispatcher(&self) -> &D {
        &self.inner.dispatch
    }

    /// Serves one request.
    ///
    /// The same as calling the gateway as a [`tower::Service`], for a caller
    /// that would rather not import the trait.
    ///
    /// [`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
    pub async fn handle<B>(&self, request: Request<B>) -> Response<GatewayBody>
    where
        B: http_body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let inner = Arc::clone(&self.inner);
        respond(move |sink| drive(inner, request, sink)).await
    }
}

impl<D: Dispatcher> GatewayBuilder<D> {
    /// Registers an interceptor for every method. This is Go's `Use`.
    #[must_use]
    pub fn layer(mut self, interceptor: impl Interceptor) -> Self {
        self.inner.stack = self.inner.stack.layer(interceptor);
        self
    }

    /// Registers an interceptor for the methods a selector matches. This is
    /// Go's `UseFor`.
    ///
    /// This is where the AIP classification pays off: `layer_on(auth,
    /// Selector::Mutating)` covers every Create, Update, Delete and Undelete,
    /// and keeps covering them when one is added later.
    #[must_use]
    pub fn layer_on(mut self, interceptor: impl Interceptor, selector: Selector) -> Self {
        self.inner.stack = self.inner.stack.layer_on(interceptor, selector);
        self
    }

    /// Replaces the whole interceptor stack.
    #[must_use]
    pub fn stack(mut self, stack: Stack) -> Self {
        self.inner.stack = stack;
        self
    }

    /// Replaces the header and metadata matchers, which is grpc-gateway's
    /// incoming, outgoing and trailer matchers in one place.
    #[must_use]
    pub fn headers(mut self, headers: Headers) -> Self {
        self.inner.headers = headers;
        self
    }

    /// Registers a metadata annotator, which is grpc-gateway's `WithMetadata`.
    /// Several may be registered; each sees what the ones before it added.
    #[must_use]
    pub fn annotator(mut self, annotator: impl MetadataAnnotator) -> Self {
        self.inner.annotators.push(Arc::new(annotator));
        self
    }

    /// Replaces the error renderer every failure leaves through.
    #[must_use]
    pub fn renderer(mut self, renderer: impl ErrorRenderer) -> Self {
        self.inner.renderer = Arc::new(renderer);
        self
    }

    /// Replaces the tunables. The domain stays the gateway's, whatever the
    /// options say, so the two cannot disagree.
    #[must_use]
    pub fn options(mut self, options: Options) -> Self {
        self.inner.options = Options {
            domain: self.inner.domain,
            ..options
        };
        self
    }

    /// Sets the largest request body accepted. A larger one is rejected with
    /// `413` before any decoding happens.
    #[must_use]
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.inner.options.max_body_bytes = bytes;
        self
    }

    /// Keeps `google.rpc.DebugInfo` details in error responses.
    ///
    /// Takes a bool rather than being a bare toggle so a deployment can wire it
    /// to a configuration flag without a conditional around the builder.
    #[must_use]
    pub fn expose_debug_info(mut self, expose: bool) -> Self {
        self.inner.options.expose_debug_info = expose;
        self
    }

    /// Answers a health endpoint ahead of routing, which is Go's
    /// `Health.Wrap`.
    ///
    /// Ahead of routing rather than as a route, so a health check never runs
    /// the interceptor stack: an authenticator that rejected a load balancer's
    /// probe would take the instance out of rotation for the wrong reason.
    #[must_use]
    pub fn health(mut self, health: Health) -> Self {
        self.inner.health = Some(health);
        self
    }

    /// Finishes the gateway.
    pub fn build(self) -> Gateway<D> {
        Gateway {
            inner: Arc::new(self.inner),
        }
    }
}

impl<D, B> tower_service::Service<Request<B>> for Gateway<D>
where
    D: Dispatcher,
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response<GatewayBody>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Infallible>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let inner = Arc::clone(&self.inner);
        Box::pin(async move { Ok(respond(move |sink| drive(inner, request, sink)).await) })
    }
}

/// Runs one request, containing a panic anywhere in it.
///
/// A panic before the head went out is an ordinary `500`; after it, the status
/// line is spent, so the body is truncated instead — the same rule a failed
/// stream follows, for the same reason.
async fn drive<D, B>(inner: Arc<Inner<D>>, request: Request<B>, mut sink: Sink)
where
    D: Dispatcher,
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let mut fallback = sink.clone();
    let domain = inner.domain;
    let content_length = inner.options.write_content_length;
    let served = AssertUnwindSafe(serve(&inner, request, &mut sink))
        .catch_unwind()
        .await;
    drop(sink);

    if served.is_err() {
        tracing::error!("gateway handler panicked");
        if fallback.committed() {
            fallback.send(super::body::Event::Abort).await;
        } else {
            let err = Error::panicked(domain, "").ensure_error_info(domain);
            fallback
                .reply(Reply::error(&err), HeaderMap::new(), content_length)
                .await;
        }
    }
}

/// The pipeline proper.
///
/// The order is fixed: routing, path captures, query, negotiation, the route
/// phase, the body, the request phase, dispatch, the response phase, the
/// completion phase. Each stage fails rather than overwriting an earlier one.
///
/// It differs from Go's in one place: the body is read *after* the route
/// phase, so an unauthenticated caller is rejected before it can make the
/// server buffer a body. Every other stage runs in Go's order.
async fn serve<D, B>(inner: &Inner<D>, request: Request<B>, sink: &mut Sink)
where
    D: Dispatcher,
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let (parts, body) = request.into_parts();
    let domain = inner.domain;

    // The URI's path is still percent-encoded, which is what routing needs:
    // routing on a decoded path would let a %2F invent a segment boundary
    // (README §1.2).
    let path = parts.uri.path();
    if let Some(health) = &inner.health
        && health.handles(path)
    {
        let (head, body) = health.respond(parts.uri.query());
        let mut reply = Reply::new(head.status, body);
        reply.headers = head.headers;
        sink.reply(reply, HeaderMap::new(), inner.options.write_content_length)
            .await;
        return;
    }

    let matched = match inner.table.resolve(parts.method.as_str(), path) {
        Resolution::Matched(matched) => matched,
        Resolution::NotFound => {
            return inner
                .reject(sink, Error::route_not_found(path, domain))
                .await;
        }
        Resolution::MethodNotAllowed { allow } => {
            let err = Error::method_not_allowed(parts.method.as_str(), &allow, domain);
            return inner.reject(sink, err).await;
        }
    };

    let route = matched.route;
    let Some(method) = inner.table.method(route.handler) else {
        let err = Error::binding_mismatch(route.template, domain, "");
        return inner.reject(sink, err).await;
    };

    // The path matched but a captured segment is malformed: a 400 naming the
    // field rather than a 404.
    let captures: HashMap<&'static str, String> = match matched.captures() {
        Ok(captures) => captures
            .into_iter()
            .map(|(field, value)| (field, value.into_owned()))
            .collect(),
        Err(err) => {
            let err =
                Error::malformed_path(err.field, err.kind.description(), domain, method.full_name);
            return inner.reject(sink, err).await;
        }
    };

    let query = match Query::parse(parts.uri.query().unwrap_or("")) {
        Ok(query) => query,
        Err(reserved) => {
            let err = Error::reserved_query_parameter(&reserved, domain, method.full_name);
            return inner.reject(sink, err).await;
        }
    };

    let negotiation = Negotiation {
        content_type: header_str(&parts.headers, header::CONTENT_TYPE),
        accept: header_str(&parts.headers, header::ACCEPT),
        alt: query.alt.as_deref(),
        streaming: method.server_stream,
    };
    let codecs = request_codec(&inner.codecs, &negotiation, domain).and_then(|request| {
        Ok((
            request,
            response_codec(&inner.codecs, &negotiation, request, domain)?,
        ))
    });
    let (request_codec, response_codec) = match codecs {
        Ok(codecs) => codecs,
        Err(err) => return inner.reject(sink, *err).await,
    };

    let mut cx = CallCx {
        route: RouteCx {
            http_method: &parts.method,
            uri: &parts.uri,
            headers: &parts.headers,
            peer: parts.extensions.get::<SocketAddr>().copied(),
            service: method.service,
            method: method.full_name,
            pattern: method.pattern,
            template: route.template,
            captures: &captures,
            metadata: Metadata::from_headers(&parts.headers, &inner.headers),
            extensions: Extensions::new(),
        },
        started: Instant::now(),
        deadline: None,
    };

    let outcome = inner
        .run(
            &mut cx,
            body,
            &query,
            Bound {
                method,
                route,
                request_codec,
                response_codec,
            },
            sink,
        )
        .await;

    // The completion phase runs whatever happened, and cannot fail: the
    // response is already written. A hook that panics is contained rather than
    // taking down a call that already succeeded.
    let outcome = match &outcome {
        Ok(status) => Outcome::Success(*status),
        Err(err) => Outcome::Failure(err),
    };
    let completed =
        std::panic::catch_unwind(AssertUnwindSafe(|| inner.stack.on_complete(&cx, &outcome)));
    if completed.is_err() {
        tracing::error!(method = method.full_name, "a completion hook panicked");
    }
}

/// What binding decided, carried into the phases.
struct Bound {
    method: &'static crate::route::MethodInfo,
    route: &'static crate::route::Route,
    request_codec: Option<&'static crate::codec::CodecEntry>,
    response_codec: &'static crate::codec::CodecEntry,
}

impl<D: Dispatcher> Inner<D> {
    /// Runs the phases from the route phase onwards, writing the response and
    /// returning how the call ended.
    async fn run<B>(
        &self,
        cx: &mut CallCx<'_>,
        body: B,
        query: &Query,
        bound: Bound,
        sink: &mut Sink,
    ) -> Result<http::StatusCode, Box<Error>>
    where
        B: http_body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        // Annotators run before the route hooks, so an authorizer sees the
        // metadata a deployment added.
        for annotator in &self.annotators {
            annotator.annotate(cx.route.headers, &mut cx.route.metadata);
        }
        if let Err(err) = self.stack.on_route(&mut cx.route) {
            return Err(self.fail(sink, err, cx).await);
        }

        // The deadline is read from the metadata the route phase left, so a
        // deadline interceptor's capping is what the rest of the call sees. The
        // tightest one wins when the client sent its own as well.
        cx.deadline = cx
            .route
            .metadata
            .get("grpc-timeout")
            .unwrap_or_default()
            .iter()
            .filter_map(|value| value.as_text().and_then(parse_grpc_timeout))
            .min();

        let body = match self.read_body(body, bound.method.full_name).await {
            Ok(body) => body,
            Err(err) => return Err(self.fail(sink, err, cx).await),
        };

        if let Err(err) = self.stack.on_request(cx) {
            return Err(self.fail(sink, err, cx).await);
        }

        if bound.method.server_stream {
            return self.serve_stream(cx, query, bound, body, sink).await;
        }
        self.serve_unary(cx, query, bound, body, sink).await
    }

    /// Dispatches a unary call and writes its reply.
    async fn serve_unary(
        &self,
        cx: &mut CallCx<'_>,
        query: &Query,
        bound: Bound,
        body: Bytes,
        sink: &mut Sink,
    ) -> Result<http::StatusCode, Box<Error>> {
        let dispatched = {
            let call = self.call(cx, query, &bound, body);
            self.dispatch.dispatch(&call).await
        };
        let mut reply = match dispatched {
            Ok(reply) => reply,
            Err(err) => return Err(self.fail(sink, err, cx).await),
        };

        // The response phase starts from the reply the handler produced rather
        // than an empty 200, so a hook sees the status the handler chose.
        let mut parts = ResponseParts {
            status: reply.status,
            headers: std::mem::take(&mut reply.headers),
            trailers: HeaderMap::new(),
        };
        if let Err(err) = self.stack.on_response(cx, &mut parts) {
            return Err(self.fail(sink, err, cx).await);
        }
        reply.status = parts.status;
        reply.headers = parts.headers;

        let status = reply.status;
        sink.reply(reply, parts.trailers, self.options.write_content_length)
            .await;
        Ok(status)
    }

    /// Dispatches a streaming call and terminates it per README §6.2.
    ///
    /// There is no response phase for a stream: by the time a hook could run,
    /// the head has gone out with the first message.
    async fn serve_stream(
        &self,
        cx: &mut CallCx<'_>,
        query: &Query,
        bound: Bound,
        body: Bytes,
        sink: &mut Sink,
    ) -> Result<http::StatusCode, Box<Error>> {
        let (result, mut writer) = {
            let call = self.call(cx, query, &bound, body);
            let mut out = Stream::new(
                sink,
                bound.response_codec,
                self.domain,
                bound.method.full_name,
            );
            let result = self.dispatch.dispatch_stream(&call, &mut out).await;
            (result, out.writer)
        };

        let committed = writer.committed();
        let termination = match result {
            Ok(()) => writer.finish(),
            Err(err) => writer.fail(self.render(err, Some(&cx.route)), encode_error),
        };

        match termination {
            Termination::Complete { close, trailers } => {
                if !committed {
                    // A stream that produced nothing still answers 200 with
                    // its framing's empty form.
                    sink.send(super::body::Event::Head(writer.status(), writer.headers()))
                        .await;
                }
                if !close.is_empty() {
                    sink.send(super::body::Event::Data(Bytes::from(close)))
                        .await;
                }
                sink.send(super::body::Event::Trailers(trailers.to_headers()))
                    .await;
                Ok(writer.status())
            }
            Termination::Deferred { error } => {
                // Nothing was written, so the status line is still ours.
                self.renderer.observe(&error, cx);
                sink.reply(
                    Reply::error(&error),
                    HeaderMap::new(),
                    self.options.write_content_length,
                )
                .await;
                Err(error)
            }
            Termination::Truncate { frame, error, .. } => {
                // Trailers are not sent: on HTTP/1.1 they would end the body
                // with the terminating chunk, which is exactly the clean close
                // that would report this failure as a success.
                self.renderer.observe(&error, cx);
                sink.send(super::body::Event::Data(Bytes::from(frame)))
                    .await;
                sink.send(super::body::Event::Abort).await;
                Err(error)
            }
        }
    }

    fn call<'c>(
        &'c self,
        cx: &'c CallCx<'c>,
        query: &'c Query,
        bound: &Bound,
        body: Bytes,
    ) -> Call<'c> {
        Call::new(
            cx,
            &self.headers,
            self.codecs.default_codec(),
            bound.method,
            bound.route,
            query,
            body,
            bound.request_codec,
            bound.response_codec,
            self.domain,
        )
    }

    /// Reads the body under the configured limit.
    ///
    /// The limit is enforced by the reader rather than by trusting
    /// `Content-Length`, which a client controls and a chunked request omits.
    async fn read_body<B>(&self, body: B, method: &str) -> Result<Bytes, Box<Error>>
    where
        B: http_body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let limit = self.options.max_body_bytes;
        match Limited::new(body, limit).collect().await {
            Ok(collected) => Ok(collected.to_bytes()),
            Err(err) if err.is::<LengthLimitError>() => {
                Err(Box::new(Error::payload_too_large(limit, self.domain)))
            }
            Err(err) => Err(Box::new(Error::malformed_body(
                &err.to_string(),
                self.domain,
                method,
            ))),
        }
    }

    /// Normalizes a failure through the renderer: the `ErrorInfo` ensured, the
    /// `DebugInfo` stripped unless the deployment asked for it.
    fn render(&self, mut err: Box<Error>, route: Option<&RouteCx<'_>>) -> Box<Error> {
        self.renderer.render(&mut err, route, &self.options);
        err
    }

    /// Renders and writes a failure from inside a call, returning it for the
    /// completion phase.
    async fn fail(&self, sink: &mut Sink, err: Box<Error>, cx: &CallCx<'_>) -> Box<Error> {
        let err = self.render(err, Some(&cx.route));
        self.renderer.observe(&err, cx);
        sink.reply(
            Reply::error(&err),
            HeaderMap::new(),
            self.options.write_content_length,
        )
        .await;
        err
    }

    /// Renders and writes a failure from before a call existed — routing,
    /// binding, negotiation. There is no completion phase for these, as in Go:
    /// no method was reached.
    async fn reject(&self, sink: &mut Sink, err: Error) {
        let err = self.render(Box::new(err), None);
        sink.reply(
            Reply::error(&err),
            HeaderMap::new(),
            self.options.write_content_length,
        )
        .await;
    }
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}
