//! Header projection from error details.

use super::DOMAIN;
use crate::error::{Code, Detail, GatewayError, Help, HelpLink, RetryInfo};
use http::header;
use serde_json::json;

#[test]
fn retry_info_projects_to_retry_after() {
    let err = GatewayError::new(Code::Unavailable, "try later")
        .with_detail(Detail::RetryInfo(RetryInfo {
            retry_delay: Some(prost_types::Duration {
                seconds: 2,
                nanos: 500_000_000,
            }),
        }))
        .ensure_error_info(DOMAIN);

    // Rounded up: a client that waits 2s arrives before the server is ready.
    assert_eq!(err.headers().get(header::RETRY_AFTER).unwrap(), "3");

    let details = err.to_json();
    let retry = details["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["@type"] == json!("type.googleapis.com/google.rpc.RetryInfo"))
        .unwrap()
        .clone();
    assert_eq!(retry["retryDelay"], json!("2.500s"));
}

#[test]
fn unauthenticated_emits_a_well_formed_challenge() {
    // grpc-gateway sets WWW-Authenticate to the raw status message, which
    // violates the RFC 7235 grammar as soon as the message contains a quote.
    let err =
        GatewayError::new(Code::Unauthenticated, "token \"abc\" expired").ensure_error_info(DOMAIN);

    let headers = err.headers();
    let value = headers
        .get(header::WWW_AUTHENTICATE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(
        value.starts_with("Bearer realm=\"library.example.com\""),
        "{value}"
    );
    assert!(value.contains("error=\"invalid_token\""), "{value}");
    assert!(
        value.contains(r#"error_description="token \"abc\" expired""#),
        "{value}"
    );
}

#[test]
fn help_links_project_to_link_headers() {
    let err = GatewayError::new(Code::InvalidArgument, "bad request")
        .with_detail(Detail::Help(Help {
            links: vec![HelpLink {
                description: "Book naming".into(),
                url: "https://example.com/docs/books".into(),
            }],
        }))
        .ensure_error_info(DOMAIN);

    assert_eq!(
        err.headers().get(header::LINK).unwrap(),
        "<https://example.com/docs/books>; rel=\"help\""
    );
}
