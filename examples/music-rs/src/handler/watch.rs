//! The streaming method.
//!
//! `WatchTracks` is a server-streaming RPC, so it is where README §6.2
//! actually applies. The two failure paths it exercises are the point: a
//! missing parent fails *before* any message and keeps its real `404`, while a
//! backend dying mid-stream can only be truncated. Which of the two happens is
//! the gateway's decision, made from whether a message has gone out — this
//! handler only reports the failure.

use super::call::CallExt;
use crate::generated::DOMAIN;
use crate::requests::WatchTracksResponse;
use crate::store::Catalog;
use bytes::BytesMut;
use transcode::codec::{Encode, JsonCodec};
use transcode::error::{Code, Error, Result};
use transcode::gateway::{Call, Stream};

/// The query parameter that makes the stream fail on cue.
///
/// It exists so both halves of README §6.2 can be exercised from outside, by a
/// test or the conformance script: `failAfter=0` fails before the first
/// message, `failAfter=1` after it.
pub const FAIL_AFTER: &str = "failAfter";

/// `GET /v1/{parent=artists/*}/tracks:watch`
pub(super) async fn watch(catalog: &Catalog, call: &Call<'_>, out: &mut Stream<'_>) -> Result<()> {
    call.reject_unknown_query(&[FAIL_AFTER])?;
    let parent = call.capture("parent")?;
    let tracks = call.rpc(catalog.list_tracks(parent, 0))?;

    let fail_after = if call.query.has(FAIL_AFTER) {
        Some(call.query_usize(FAIL_AFTER)?)
    } else {
        None
    };

    for (sent, track) in tracks.into_iter().enumerate() {
        if fail_after.is_some_and(|limit| sent >= limit) {
            return Err(unavailable(call));
        }
        let mut buf = BytesMut::new();
        JsonCodec::new()
            .encode(&WatchTracksResponse { track }, &mut buf)
            .map_err(|err| Box::new(err.into_gateway_error(DOMAIN, call.method.full_name)))?;
        out.send_encoded(&buf).await?;
    }

    if fail_after.is_some() {
        return Err(unavailable(call));
    }
    Ok(())
}

/// The failure a backend dying mid-stream produces.
fn unavailable(call: &Call<'_>) -> Box<Error> {
    call.error(
        Code::Unavailable,
        "The catalog became unavailable mid-stream.",
    )
}
