//! Dispatch: method index to handler.
//!
//! A `match` over a generated enum rather than a lookup on a string, so adding
//! a method cannot silently shadow an existing one and every arm is checked for
//! exhaustiveness at compile time.

use super::{Call, Reply, artists, tracks};
use crate::generated::Method;
use grpc_http::error::GatewayError;

/// Routes a resolved call to its handler.
///
/// # Errors
///
/// Whatever the handler returns, plus `UNIMPLEMENTED` for the streaming method,
/// which the runtime does not serve yet.
pub(super) fn dispatch(call: &Call<'_>) -> Result<Reply, Box<GatewayError>> {
    match call.method {
        Method::GetArtist => artists::get(call),
        Method::ListArtists => artists::list(call),
        Method::CreateArtist => artists::create(call),
        Method::UpdateArtist => artists::update(call),
        Method::DeleteArtist => artists::delete(call),
        Method::GetTrack => tracks::get(call),
        Method::ListTracks => tracks::list(call),
        Method::CreateTrack => tracks::create(call),
        Method::UpdateTrack => tracks::update(call),
        Method::DeleteTrack => tracks::delete(call),
        Method::WithdrawTrack => tracks::withdraw(call),
        Method::WatchTracks => super::watch::watch(call),
    }
}
