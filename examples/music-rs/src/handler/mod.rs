//! The catalog's HTTP surface: the generated tables, served by a
//! [`transcode::Gateway`] whose dispatcher calls the catalog.
//!
//! The dispatcher is what `protoc-gen-http` will emit beside the tables.
//! Writing it by hand first fixes the shape the generator has to produce. It is
//! the same shape as the Go example's `Service.Dispatch`: a switch on the
//! handler index, one function per method.
//!
//! Everything else — routing, binding, negotiation, the middleware phases, and
//! the single error funnel — is the gateway's, so this module only says what
//! each method does:
//!
//! ```text
//! route ─► 404 / 405        bind ─► 400 / 415        call ─► mapped Status
//!                                                  encode ─► 406 / 500
//! ```

use crate::generated::{CODECS, DOMAIN, METHODS, Method, ROUTES};
use crate::store::Catalog;
use http::{Request, Response};
use std::sync::Arc;
use transcode::codec::CodecRegistry;
use transcode::error::{Error, Result};
use transcode::gateway::{Call, Dispatcher, Gateway, GatewayBody, GatewayBuilder, Reply, Stream};
use transcode::route::RouteTable;

mod artists;
mod call;
mod tracks;
mod watch;

pub use call::CallExt;
pub use watch::FAIL_AFTER;

/// Serves resolved calls from the catalog.
#[derive(Debug)]
pub struct Service {
    catalog: Arc<Catalog>,
}

impl Service {
    /// A dispatcher over a catalog.
    #[must_use]
    pub const fn new(catalog: Arc<Catalog>) -> Self {
        Self { catalog }
    }

    /// The error for a handler index this dispatcher does not serve, which
    /// would mean the tables and the dispatch disagree.
    fn mismatch(call: &Call<'_>) -> Box<Error> {
        Box::new(Error::binding_mismatch(
            call.route.template,
            DOMAIN,
            call.method.full_name,
        ))
    }
}

impl Dispatcher for Service {
    async fn dispatch(&self, call: &Call<'_>) -> Result<Reply> {
        let catalog = &self.catalog;
        match Method::from_handler(call.handler()) {
            Method::GetArtist => artists::get(catalog, call),
            Method::ListArtists => artists::list(catalog, call),
            Method::CreateArtist => artists::create(catalog, call),
            Method::UpdateArtist => artists::update(catalog, call),
            Method::DeleteArtist => artists::delete(catalog, call),
            Method::GetTrack => tracks::get(catalog, call),
            Method::ListTracks => tracks::list(catalog, call),
            Method::CreateTrack => tracks::create(catalog, call),
            Method::UpdateTrack => tracks::update(catalog, call),
            Method::DeleteTrack => tracks::delete(catalog, call),
            Method::WithdrawTrack => tracks::withdraw(catalog, call),
            Method::WatchTracks => Err(Self::mismatch(call)),
        }
    }

    async fn dispatch_stream(&self, call: &Call<'_>, out: &mut Stream<'_>) -> Result<()> {
        match Method::from_handler(call.handler()) {
            Method::WatchTracks => watch::watch(&self.catalog, call, out).await,
            _ => Err(Self::mismatch(call)),
        }
    }
}

/// The catalog's gateway: cheap to clone, and one value behind every listener.
#[derive(Debug, Clone)]
pub struct Handler {
    gateway: Gateway<Service>,
}

impl Handler {
    /// A handler with no interceptors.
    #[must_use]
    pub fn new(catalog: Arc<Catalog>) -> Self {
        Self::builder(catalog).build().into()
    }

    /// Starts configuring the gateway, for a caller adding interceptors. This
    /// is the Go example's `NewHandler(catalog, opts...)`.
    #[must_use]
    pub fn builder(catalog: Arc<Catalog>) -> GatewayBuilder<Service> {
        Gateway::builder(
            RouteTable::with_methods(ROUTES, METHODS),
            CodecRegistry::new(CODECS),
            Service::new(catalog),
            DOMAIN,
        )
    }

    /// The gateway, which is a `tower::Service` in its own right.
    #[must_use]
    pub const fn gateway(&self) -> &Gateway<Service> {
        &self.gateway
    }

    /// The compiled route table, for tests that assert on resolution directly.
    #[must_use]
    pub fn routes(&self) -> &RouteTable {
        self.gateway.table()
    }

    /// Serves one request.
    pub async fn handle<B>(&self, request: Request<B>) -> Response<GatewayBody>
    where
        B: http_body::Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        self.gateway.handle(request).await
    }
}

impl From<Gateway<Service>> for Handler {
    fn from(gateway: Gateway<Service>) -> Self {
        Self { gateway }
    }
}
