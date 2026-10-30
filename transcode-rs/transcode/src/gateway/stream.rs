//! The handle a streaming dispatcher writes messages through.

use super::body::{Event, Sink};
use crate::codec::{self, CodecEntry, Payload};
use crate::error::{Code, Error, Result};
use crate::stream::StreamWriter;
use bytes::{Bytes, BytesMut};

/// Writes the messages of a server-streaming call.
///
/// It exists so a dispatcher cannot commit the status line by accident: there
/// is no way to reach the connection through it, and the head goes out with the
/// first [`Stream::send`] rather than when the stream opens. A failure before
/// that therefore still has a real status — which covers authorization,
/// validation, quota and not-found, the overwhelming majority of real failures.
#[derive(Debug)]
pub struct Stream<'s> {
    /// The state machine enforcing README §6.2.
    pub(crate) writer: StreamWriter,
    sink: &'s mut Sink,
    codec: &'static CodecEntry,
    domain: &'static str,
    method: &'static str,
}

impl<'s> Stream<'s> {
    pub(crate) fn new(
        sink: &'s mut Sink,
        codec: &'static CodecEntry,
        domain: &'static str,
        method: &'static str,
    ) -> Self {
        Self {
            writer: StreamWriter::new(codec.framing, codec.content_type()),
            sink,
            codec,
            domain,
            method,
        }
    }

    /// Encodes and writes one message with the negotiated codec, committing the
    /// status line if it is the first.
    ///
    /// # Errors
    ///
    /// The message could not be encoded, or the client went away. Either way
    /// the dispatcher should return the error: it ends the stream, and how it
    /// ends is this crate's decision.
    pub async fn send<M: Payload>(&mut self, message: &M) -> Result<()> {
        let mut buf = BytesMut::new();
        codec::payload::encode(self.codec, message, &mut buf)
            .map_err(|err| Box::new(err.into_gateway_error(self.domain, self.method)))?;
        self.send_encoded(&buf).await
    }

    /// Writes one already-encoded message, committing the status line if it is
    /// the first.
    ///
    /// # Errors
    ///
    /// `CANCELLED` when the client went away.
    pub async fn send_encoded(&mut self, encoded: &[u8]) -> Result<()> {
        let first = !self.writer.committed();
        let frame = self.writer.message(encoded);
        if first {
            let head = Event::Head(self.writer.status(), self.writer.headers());
            if !self.sink.send(head).await {
                return Err(self.gone());
            }
        }
        if !self.sink.send(Event::Data(Bytes::from(frame))).await {
            return Err(self.gone());
        }
        Ok(())
    }

    /// Writes a keepalive frame, for framings that have one.
    ///
    /// A dispatcher calls it on an idle timer: an SSE connection that is merely
    /// waiting looks identical to one that has died, and intermediaries reap
    /// it. Before the first message there is no body to keep alive, so it does
    /// nothing.
    pub async fn keepalive(&mut self) {
        if !self.writer.committed() {
            return;
        }
        if let Some(frame) = self.writer.keepalive() {
            // A failed keepalive means the peer is gone, which the next send
            // reports properly.
            let _ = self.sink.send(Event::Data(Bytes::from(frame))).await;
        }
    }

    /// Whether the status line has gone out.
    #[must_use]
    pub const fn committed(&self) -> bool {
        self.writer.committed()
    }

    fn gone(&self) -> Box<Error> {
        Box::new(
            Error::new(Code::Cancelled, "The client went away.").with_error_info(
                "CANCELLED",
                self.domain,
                [("method".into(), self.method.into())],
            ),
        )
    }
}
