//! Writing messages into a framing.

use crate::codec::Framing;
use crate::error::Error;

/// Renders messages and errors into one framing's byte layout.
///
/// Separate from [`StreamWriter`](super::StreamWriter) because the framing is
/// pure byte arrangement while the writer is a state machine about what may be
/// written when. Keeping them apart is what lets a new framing be added without
/// touching the rule in §8.4.
#[derive(Debug, Clone, Copy)]
pub struct FrameWriter {
    framing: Framing,
    /// Whether anything has been written, which decides between the opening
    /// bytes and the separator.
    started: bool,
}

impl FrameWriter {
    /// A writer for one framing.
    #[must_use]
    pub const fn new(framing: Framing) -> Self {
        Self {
            framing,
            started: false,
        }
    }

    /// The framing being written.
    #[must_use]
    pub const fn framing(&self) -> Framing {
        self.framing
    }

    /// The bytes for one encoded message, including any leading delimiter.
    ///
    /// For [`Framing::JsonArray`] the first call emits `[` and later calls emit
    /// `,`, so the response is valid JSON at every point a reader could stop.
    pub fn message(&mut self, encoded: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(encoded.len() + 16);

        match self.framing {
            Framing::JsonArray => {
                out.extend_from_slice(if self.started { b"," } else { b"[" });
                out.extend_from_slice(encoded);
            }
            Framing::Sse => {
                out.extend_from_slice(b"event: message\ndata: ");
                out.extend_from_slice(encoded);
                out.extend_from_slice(b"\n\n");
            }
            Framing::LineDelimited => {
                out.extend_from_slice(encoded);
                out.push(b'\n');
            }
            Framing::LengthPrefixed => {
                // Four-byte big-endian length, matching gRPC's framing minus
                // the compression flag.
                let len = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(encoded);
            }
        }
        self.started = true;
        out
    }

    /// The bytes that close a stream that completed cleanly.
    #[must_use]
    pub fn close(&self) -> Vec<u8> {
        match self.framing {
            // An empty JSON array still has to be well-formed.
            Framing::JsonArray if !self.started => b"[]".to_vec(),
            Framing::JsonArray => b"]".to_vec(),
            _ => Vec::new(),
        }
    }

    /// The terminal error frame for a stream that failed after committing.
    ///
    /// `encoded` is the AIP-193 envelope, already serialized by the negotiated
    /// codec. The frame goes out *before* the body is truncated, so a client
    /// that does read the body learns why.
    #[must_use]
    pub fn error(&mut self, encoded: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(encoded.len() + 24);

        match self.framing {
            Framing::JsonArray => {
                out.extend_from_slice(if self.started { b"," } else { b"[" });
                out.extend_from_slice(encoded);
                out.extend_from_slice(b"]");
            }
            Framing::Sse => {
                // A distinct event name, so a browser handler can bind to it
                // rather than having to inspect every message.
                out.extend_from_slice(b"event: error\ndata: ");
                out.extend_from_slice(encoded);
                out.extend_from_slice(b"\n\n");
            }
            Framing::LineDelimited => {
                out.extend_from_slice(encoded);
                out.push(b'\n');
            }
            Framing::LengthPrefixed => {
                let len = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(encoded);
            }
        }
        self.started = true;
        out
    }

    /// A keepalive comment, for framings that have one.
    ///
    /// SSE connections are reaped by intermediaries when idle, and a stream
    /// that is merely waiting looks identical to one that has died.
    #[must_use]
    pub fn keepalive(&self) -> Option<Vec<u8>> {
        matches!(self.framing, Framing::Sse).then(|| b": keepalive\n\n".to_vec())
    }
}

/// The framing-level view of a stream, for a transport writing the bytes.
#[derive(Debug)]
pub struct StreamFrames {
    /// The `Content-Type` for the negotiated framing.
    pub content_type: &'static str,
    /// The error rendered into the framing, when the stream failed.
    pub error: Option<Error>,
}
