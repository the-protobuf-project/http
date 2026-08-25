//! An AIP-native HTTP/JSON surface for a gRPC service.
//!
//! The wire behaviour this implements is specified in the protocol section of
//! the repository README, and is shared with the Go and Python runtimes. Two
//! properties shape the whole crate:
//!
//! - **Nothing here parses protobuf.** Path templates, field paths, validation
//!   rules and response sets are compiled by `protoc-gen-http` at build time.
//!   This crate executes a table. Two runtimes generated from one IR therefore
//!   cannot disagree about what a request means.
//! - **A failed RPC is never reported as a success.** Unary responses are fully
//!   encoded before the status line is written, and a stream defers its header
//!   until the first message or termination. See README §6.2
//!
//! The handler is a [`tower::Service`] over [`http::Request`], which is what
//! lets one handler sit behind both an HTTP/1.1 listener and an HTTP/3 one
//! without knowing which it is serving.
//!
//! # Relationship to grpc-gateway
//!
//! This is not a port of [grpc-gateway]. It follows the [AIP] corpus where the
//! two disagree — most visibly in the error envelope (AIP-193), in payload
//! validation, and in never reporting a failed RPC as a `200`. The differences
//! are enumerated in the protocol, Divergences
//!
//! [grpc-gateway]: https://github.com/grpc-ecosystem/grpc-gateway
//! [AIP]: https://google.aip.dev/
//! [`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod codec;
pub mod error;
pub mod middleware;
pub mod route;
pub mod stream;
