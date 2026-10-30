//! The gateway: a generated route table served as a [`tower::Service`].
//!
//! This is the Rust counterpart of the Go runtime's `transcode.New`. A
//! generated module supplies the route table, the method table, the codecs and
//! a [`Dispatcher`]; [`Gateway`] supplies everything around a call — routing,
//! binding, negotiation, the middleware phases, error rendering, and the
//! streaming rules of README §6.2.
//!
//! ```ignore
//! let gateway = Gateway::builder(
//!     RouteTable::with_methods(ROUTES, METHODS),
//!     CodecRegistry::new(CODECS),
//!     Dispatch::new(service),
//!     DOMAIN,
//! )
//! .layer(Recovery)
//! .layer_on(auth, Selector::Mutating)
//! .build();
//!
//! // A tower::Service<http::Request<B>>: mount it on axum, a hybrid gRPC
//! // server, or a bare hyper connection.
//! let app = axum::Router::new().fallback_service(gateway);
//! ```
//!
//! # Calling a gRPC service
//!
//! A dispatcher usually calls a tonic service in-process. [`Call::request`]
//! builds the `tonic::Request` with the forwarded metadata and what remains of
//! the deadline, and [`Call::respond`] renders the `tonic::Response` with its
//! metadata projected into headers:
//!
//! ```ignore
//! Method::GetArtist => {
//!     let request = call.request(GetArtistRequest { name: call.capture("name")?.into() });
//!     call.respond(self.client.clone().get_artist(request).await)
//! }
//! ```
//!
//! [`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html

mod body;
mod call;
mod dispatch;
mod pipeline;
mod query;
mod reply;
mod stream;

#[cfg(test)]
mod tests;

pub use body::{GatewayBody, TruncatedStream};
pub use call::Call;
pub use dispatch::Dispatcher;
pub use pipeline::{Gateway, GatewayBuilder};
pub use query::{PARAM_ALT, PARAM_FIELDS, PARAM_PRETTY_PRINT, Query};
pub use reply::Reply;
pub use stream::Stream;
