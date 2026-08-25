//! The streaming method.
//!
//! `WatchTracks` is a server-streaming RPC, so it is where README §6.2
//! actually applies. The two failure paths it exercises are the point:
//! a missing parent fails *before* any message and keeps its real `404`,
//! while a backend dying mid-stream can only be truncated.

use super::{Call, Reply};
use crate::generated::DOMAIN;
use crate::model::Track;
use crate::requests::WatchTracksResponse;
use bytes::BytesMut;
use transcode::codec::{Encode, Framing, JsonCodec};
use transcode::error::Code;
use transcode::error::Error;
use transcode::stream::{StreamWriter, Termination};

/// The query parameter that makes the stream fail on cue.
///
/// It exists so both halves of README §6.2 can be exercised from outside, by a
/// client watching real bytes on a real socket: `?failAfter=0` fails before the
/// first message and must keep its real status, while `?failAfter=2` fails once
/// the status line is spent and must truncate instead.
///
/// A real service has no such parameter — and a real service also cannot be
/// asked to fail on cue, so a rule tested only from the inside is a rule no
/// transport is ever held to. The Go example carries the same hook, which is
/// what lets the conformance script ask both the same question.
pub const FAIL_AFTER: &str = "failAfter";

/// `GET /v1/{parent=artists/*}/tracks:watch`
///
/// The example drives the stream to completion in memory rather than holding a
/// connection open, which is enough to exercise the contract: what matters is
/// which [`Termination`] comes out, and that is decided by whether a message
/// was written before the failure.
pub(super) fn watch(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    call.reject_unknown_query(&[FAIL_AFTER])?;
    let parent = call.capture("parent")?;

    // Resolving the parent *before* the first message is deliberate. It is what
    // puts a not-found failure on the deferred path, where it still gets a real
    // 404 rather than a 200 with an error buried in the body.
    let tracks = call.rpc(call.catalog.list_tracks(parent, 0))?;

    let framing = negotiated_framing(call);
    let mut writer = StreamWriter::new(framing, content_type(framing));
    let mut body = Vec::new();

    // Absent means never fail, which is the ordinary path.
    let fail_after = match call.query.get(FAIL_AFTER) {
        Some(_) => Some(call.query_usize(FAIL_AFTER)?),
        None => None,
    };

    for (sent, track) in tracks.iter().enumerate() {
        if fail_after.is_some_and(|limit| sent >= limit) {
            return finish_failed(writer, body, unavailable());
        }
        match encode_track(track) {
            Ok(encoded) => body.extend_from_slice(&writer.message(&encoded)),
            // A message that will not encode is a service bug, and it arrives
            // mid-stream, so it takes the truncation path.
            Err(err) => return finish_failed(writer, body, err),
        }
    }

    // A limit at or past the end still fails: the stream was asked to, and
    // failing only when it happens to stop early would make the hook depend on
    // how many tracks the catalog holds.
    if fail_after.is_some() {
        return finish_failed(writer, body, unavailable());
    }

    match writer.finish() {
        Termination::Complete { close, trailers } => {
            body.extend_from_slice(&close);
            let mut reply = Reply {
                status: writer.status(),
                headers: writer.headers(),
                body,
            };
            reply.headers.extend(trailers.to_headers());
            Ok(reply)
        }
        // finish() only ever completes.
        other => unreachable!("finish produced {other:?}"),
    }
}

/// The failure `?failAfter=` raises.
///
/// `UNAVAILABLE` rather than `INTERNAL` because it stands in for a backend that
/// went away mid-stream, which is the case the truncation rule exists for.
fn unavailable() -> Box<Error> {
    Box::new(
        Error::new(
            Code::Unavailable,
            "The catalog became unavailable mid-stream.",
        )
        .with_error_info(
            "UNAVAILABLE",
            DOMAIN,
            [(
                "method".into(),
                "music.v1.TrackService.WatchTracks".to_string(),
            )],
        ),
    )
}

/// Ends a stream that failed, honouring whichever branch of §8.4 applies.
fn finish_failed(
    mut writer: StreamWriter,
    mut body: Vec<u8>,
    err: Box<Error>,
) -> Result<Reply, Box<Error>> {
    match writer.fail(err, |e| {
        serde_json::to_vec(&e.to_json()).unwrap_or_default()
    }) {
        // Nothing had been written, so this is an ordinary error response.
        Termination::Deferred { error } => Err(error),

        Termination::Truncate {
            frame, trailers, ..
        } => {
            body.extend_from_slice(&frame);
            let mut reply = Reply {
                status: writer.status(),
                headers: writer.headers(),
                body,
            };
            reply.headers.extend(trailers.to_headers());

            // The transport must now terminate the body abnormally. This header
            // is how the example's listener is told to; a real integration
            // would carry it out of band rather than on the wire.
            reply
                .headers
                .insert("x-handler-truncate", http::HeaderValue::from_static("1"));
            Ok(reply)
        }
        other => unreachable!("fail produced {other:?}"),
    }
}

/// The framing the request negotiated.
///
/// `?alt=sse` and `Accept: text/event-stream` both reach SSE; everything else
/// gets the JSON array, which is the default and what Google's own REST
/// endpoints stream.
fn negotiated_framing(call: &Call<'_>) -> Framing {
    let wants_sse = call.query.get("alt").is_some_and(|alt| alt == "sse")
        || call
            .accept
            .as_deref()
            .is_some_and(|accept| accept.contains("text/event-stream"));

    if wants_sse {
        Framing::Sse
    } else {
        Framing::JsonArray
    }
}

/// The `Content-Type` for a framing.
const fn content_type(framing: Framing) -> &'static str {
    match framing {
        Framing::Sse => "text/event-stream",
        Framing::LineDelimited => "application/x-ndjson",
        Framing::LengthPrefixed => "application/x-protobuf",
        Framing::JsonArray => "application/json",
    }
}

/// Encodes one streamed message.
fn encode_track(track: &Track) -> Result<Vec<u8>, Box<Error>> {
    let mut buf = BytesMut::new();
    JsonCodec::new()
        .encode(
            &WatchTracksResponse {
                track: track.clone(),
            },
            &mut buf,
        )
        .map_err(|err| {
            Box::new(err.into_gateway_error(DOMAIN, "music.v1.TrackService.WatchTracks"))
        })?;
    Ok(buf.to_vec())
}
