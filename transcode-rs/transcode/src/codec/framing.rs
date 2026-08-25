//! How a sequence of messages is delimited on the wire.

/// The framing a codec uses for a server-streaming response.
///
/// Framing is a property of the codec rather than a separate negotiation axis:
/// asking for `application/json` and asking for a JSON array of results are the
/// same request, and splitting them would let a client select a combination
/// that has no meaning, like SSE carrying length-prefixed protobuf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Framing {
    /// One JSON array, written incrementally: `[` with the first message, `,`
    /// before each subsequent one, `]` at the end.
    ///
    /// The response is valid JSON once complete and parseable by a streaming
    /// reader throughout. This is how Google's own REST endpoints stream, and
    /// it is the default.
    JsonArray,

    /// Server-Sent Events: `event: message` and a `data:` line per message.
    ///
    /// Streaming only. A unary request that selects it is rejected, because a
    /// single-message event stream is a worse answer than a plain body.
    Sse,

    /// One compact JSON value per line, `\n`-separated.
    ///
    /// What grpc-gateway emits. Offered for clients already written against it.
    LineDelimited,

    /// A four-byte big-endian length followed by that many bytes, per message.
    ///
    /// This is the gRPC message framing minus the compression flag, and the
    /// only sensible choice for a binary codec: line-delimiting bytes that may
    /// themselves contain a newline does not work.
    LengthPrefixed,
}

impl Framing {
    /// Whether a unary response may use this framing.
    ///
    /// SSE is the exception: it exists to carry a sequence of events, and a
    /// one-event stream is strictly worse for the client than a normal body.
    pub const fn allows_unary(self) -> bool {
        !matches!(self, Framing::Sse)
    }

    /// The bytes written before the first message of a stream.
    pub const fn open(self) -> &'static [u8] {
        match self {
            Framing::JsonArray => b"[",
            _ => b"",
        }
    }

    /// The bytes written between two consecutive messages.
    ///
    /// For [`Framing::JsonArray`] this is the array separator; for the
    /// line-oriented framings it terminates the preceding message. SSE puts its
    /// separator after each event instead, so it has none here.
    pub const fn separator(self) -> &'static [u8] {
        match self {
            Framing::JsonArray => b",",
            Framing::LineDelimited => b"\n",
            Framing::Sse | Framing::LengthPrefixed => b"",
        }
    }

    /// The bytes written after the final message of a successful stream.
    ///
    /// A stream that fails mid-flight does **not** get this: it is terminated
    /// abnormally instead, which is the only signal HTTP has left once the
    /// status line is spent. See README §6.2
    pub const fn close(self) -> &'static [u8] {
        match self {
            Framing::JsonArray => b"]",
            Framing::LineDelimited => b"\n",
            Framing::Sse | Framing::LengthPrefixed => b"",
        }
    }

    /// Whether each message needs its own flush to reach the client promptly.
    ///
    /// True for every framing: a stream the client cannot see until it ends is
    /// not a stream. Present as a method so a future buffered framing can say
    /// otherwise without the writer growing a special case.
    pub const fn flush_per_message(self) -> bool {
        true
    }
}
