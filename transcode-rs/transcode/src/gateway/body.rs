//! The response body, and the channel a call in progress feeds it through.
//!
//! # Why the call runs inside the body
//!
//! A tower service returns its response head before its body, but a streaming
//! call does not know its status until it has produced its first message or
//! failed — README §6.2. So the head cannot be computed up front, and the call
//! cannot finish before the head is returned.
//!
//! The call is therefore one future, `drive`, that reports what it decides as
//! [`Event`]s: a head, data, trailers, or an abort. The service future polls
//! `drive` until a head arrives; the body then owns `drive` and keeps polling it
//! as the listener asks for frames. Nothing is spawned, so the crate needs no
//! runtime, and a client that disconnects drops the body — and the call with it.

use super::reply::Reply;
use bytes::Bytes;
use futures_channel::mpsc;
use futures_util::future::{BoxFuture, poll_fn};
use futures_util::stream::Stream;
use http::{HeaderMap, HeaderValue, Response, StatusCode, header};
use http_body::{Body, Frame, SizeHint};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

/// How many events may queue between the call and the listener.
///
/// Small on purpose: a slow client should slow the producer down, not let a
/// stream buffer without bound in memory.
const CHANNEL_CAPACITY: usize = 16;

/// What a call reports to the body.
#[derive(Debug)]
pub(crate) enum Event {
    /// The status line and headers. Always first, and sent exactly once.
    Head(StatusCode, HeaderMap),
    /// Body bytes.
    Data(Bytes),
    /// Trailers, which end the body.
    Trailers(HeaderMap),
    /// Terminate the body abnormally: the stream failed after its status line
    /// was committed, and truncation is the only signal left.
    Abort,
}

/// The sending half, held by the call.
#[derive(Debug, Clone)]
pub(crate) struct Sink {
    tx: mpsc::Sender<Event>,
    committed: Arc<AtomicBool>,
}

impl Sink {
    /// Sends one event, waiting for room. `false` means the body is gone — the
    /// client disconnected — and nothing more will be read.
    pub(crate) async fn send(&mut self, event: Event) -> bool {
        if matches!(event, Event::Head(..)) {
            self.committed.store(true, Ordering::Release);
        }
        if poll_fn(|cx| self.tx.poll_ready(cx)).await.is_err() {
            return false;
        }
        self.tx.start_send(event).is_ok()
    }

    /// Whether a head has been sent.
    pub(crate) fn committed(&self) -> bool {
        self.committed.load(Ordering::Acquire)
    }

    /// Sends a whole reply: head, body, and trailers if there are any.
    ///
    /// `Content-Length` is set when `content_length` asks for it and nothing
    /// forbids it, so a unary response is not chunk-encoded merely because it
    /// travelled through a channel.
    pub(crate) async fn reply(
        &mut self,
        mut reply: Reply,
        trailers: HeaderMap,
        content_length: bool,
    ) {
        let forbids_body = reply.forbids_body();
        if forbids_body {
            reply.body = Bytes::new();
        }

        if !trailers.is_empty() {
            let names: Vec<&str> = trailers.keys().map(http::HeaderName::as_str).collect();
            if let Ok(value) = HeaderValue::from_str(&names.join(", ")) {
                reply.headers.insert(header::TRAILER, value);
            }
        } else if content_length && !forbids_body {
            reply
                .headers
                .insert(header::CONTENT_LENGTH, HeaderValue::from(reply.body.len()));
        }

        if !self.send(Event::Head(reply.status, reply.headers)).await {
            return;
        }
        if !reply.body.is_empty() && !self.send(Event::Data(reply.body)).await {
            return;
        }
        if !trailers.is_empty() {
            self.send(Event::Trailers(trailers)).await;
        }
    }
}

/// The error a truncated body ends with.
///
/// A listener that sees it terminates the response abnormally — `RST_STREAM`
/// on HTTP/2 and HTTP/3, a close without the terminating zero-length chunk on
/// HTTP/1.1 — which is what makes `curl` exit non-zero and `fetch()` reject
/// for a stream that failed after committing a `200`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruncatedStream;

impl std::fmt::Display for TruncatedStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stream failed after committing its status; body truncated")
    }
}

impl std::error::Error for TruncatedStream {}

/// The body of every response a [`Gateway`](super::Gateway) produces.
///
/// It drives the call that feeds it, so it must be polled for the call to make
/// progress — which every listener does, because that is how it reads a body.
pub struct GatewayBody {
    drive: Option<BoxFuture<'static, ()>>,
    rx: mpsc::Receiver<Event>,
    done: bool,
    /// An abort has been received and one poll has been yielded; the next
    /// poll fails the body.
    aborting: bool,
}

impl std::fmt::Debug for GatewayBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayBody")
            .field("running", &self.drive.is_some())
            .field("done", &self.done)
            .field("aborting", &self.aborting)
            .finish()
    }
}

impl GatewayBody {
    /// Advances the call, forgetting it once it completes so it is never
    /// polled again.
    fn poll_drive(&mut self, cx: &mut Context<'_>) {
        if let Some(drive) = self.drive.as_mut()
            && drive.as_mut().poll(cx).is_ready()
        {
            self.drive = None;
        }
    }

    /// Polls until the head arrives. `None` means the call ended without
    /// producing one, which only a bug in this module could cause.
    fn poll_head(&mut self, cx: &mut Context<'_>) -> Poll<Option<(StatusCode, HeaderMap)>> {
        loop {
            self.poll_drive(cx);
            match Pin::new(&mut self.rx).poll_next(cx) {
                Poll::Ready(Some(Event::Head(status, headers))) => {
                    return Poll::Ready(Some((status, headers)));
                }
                // Anything before the head is a sequencing bug; skip it rather
                // than serve a body under a status nobody chose.
                Poll::Ready(Some(_)) => {}
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Body for GatewayBody {
    type Data = Bytes;
    type Error = TruncatedStream;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if this.aborting {
            this.aborting = false;
            this.done = true;
            return Poll::Ready(Some(Err(TruncatedStream)));
        }
        loop {
            if this.done {
                return Poll::Ready(None);
            }
            this.poll_drive(cx);
            match Pin::new(&mut this.rx).poll_next(cx) {
                Poll::Ready(Some(Event::Data(bytes))) => {
                    return Poll::Ready(Some(Ok(Frame::data(bytes))));
                }
                Poll::Ready(Some(Event::Trailers(trailers))) => {
                    this.done = true;
                    return Poll::Ready(Some(Ok(Frame::trailers(trailers))));
                }
                Poll::Ready(Some(Event::Abort)) => {
                    // Yield once before failing. A listener that sees the error
                    // at once may tear the connection down with what it has
                    // buffered still unwritten — the head and the frames before
                    // the failure — which turns a truncated stream into an
                    // empty reply. Pending with an immediate wake gives it one
                    // turn to flush first.
                    this.aborting = true;
                    this.drive = None;
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Ready(Some(Event::Head(..))) => {}
                Poll::Ready(None) => {
                    this.done = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.done && !self.aborting
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

/// Runs a call, returning its response once the call has decided its head.
///
/// `call` is handed the [`Sink`] to report through; it owns everything it
/// needs, so the body can keep driving it after this returns.
pub(crate) async fn respond<F, C>(call: C) -> Response<GatewayBody>
where
    C: FnOnce(Sink) -> F,
    F: Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
    let sink = Sink {
        tx,
        committed: Arc::new(AtomicBool::new(false)),
    };
    let mut body = GatewayBody {
        drive: Some(Box::pin(call(sink))),
        rx,
        done: false,
        aborting: false,
    };

    match poll_fn(|cx| body.poll_head(cx)).await {
        Some((status, headers)) => {
            let mut response = Response::new(body);
            *response.status_mut() = status;
            *response.headers_mut() = headers;
            response
        }
        None => {
            let mut response = Response::new(GatewayBody {
                drive: None,
                rx: mpsc::channel(0).1,
                done: true,
                aborting: false,
            });
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        }
    }
}
