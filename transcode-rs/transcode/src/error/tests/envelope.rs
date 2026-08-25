//! The AIP-193 envelope and the status mapping behind it.

use super::DOMAIN;
use crate::error::{Code, Detail, Error, FieldViolation};
use http::StatusCode;
use serde_json::json;

#[test]
fn envelope_code_is_the_http_status_not_the_grpc_code() {
    // grpc-gateway's DefaultHTTPErrorHandler (runtime/errors.go:105) marshals
    // the raw google.rpc.Status, so an INVALID_ARGUMENT body reports
    // {"code": 3, ...} — 3 being the canonical code's number, not an HTTP
    // status. AIP-193 requires the HTTP status here.
    let err = Error::new(Code::InvalidArgument, "Bad book.").ensure_error_info(DOMAIN);
    let body = err.to_json();

    assert_eq!(body["error"]["code"], json!(400));
    assert_eq!(body["error"]["status"], json!("INVALID_ARGUMENT"));
    assert_eq!(body["error"]["message"], json!("Bad book."));
    // The envelope is wrapped; a bare status object is not AIP-193.
    assert!(body.get("code").is_none());
}

#[test]
fn status_mapping_matches_the_protocol_table() {
    let table = [
        (Code::Ok, 200),
        (Code::Cancelled, 499),
        (Code::Unknown, 500),
        (Code::InvalidArgument, 400),
        (Code::DeadlineExceeded, 504),
        (Code::NotFound, 404),
        (Code::AlreadyExists, 409),
        (Code::PermissionDenied, 403),
        (Code::ResourceExhausted, 429),
        (Code::FailedPrecondition, 400),
        (Code::Aborted, 409),
        (Code::OutOfRange, 400),
        (Code::Unimplemented, 501),
        (Code::Internal, 500),
        (Code::Unavailable, 503),
        (Code::DataLoss, 500),
        (Code::Unauthenticated, 401),
    ];
    for (code, want) in table {
        assert_eq!(code.http_status(), want, "{}", code.as_str());
    }
}

#[test]
fn method_not_allowed_stays_405_and_carries_allow() {
    // grpc-gateway maps 405 through codes.Unimplemented (runtime/errors.go:196),
    // which HTTPStatusFromCode turns back into 501. Status and header are lost.
    let err = Error::method_not_allowed("PATCH", &["GET", "DELETE"], DOMAIN);

    assert_eq!(err.http, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(err.to_json()["error"]["code"], json!(405));
    assert_eq!(
        err.headers().get(http::header::ALLOW).unwrap(),
        "GET, DELETE"
    );
}

#[test]
fn field_violations_are_reported_together() {
    // Validation collects every problem rather than stopping at the first, so
    // one round trip tells the caller everything that is wrong.
    let err = Error::invalid_fields(
        vec![
            FieldViolation {
                field: "book.displayName".into(),
                description: "must be between 1 and 63 characters".into(),
                reason: "VALUE_LENGTH".into(),
            },
            FieldViolation {
                field: "parent".into(),
                description: "must match pattern \"shelves/{shelf}\"".into(),
                reason: "RESOURCE_NAME_MALFORMED".into(),
            },
        ],
        "INVALID_ARGUMENT",
        DOMAIN,
        "library.v1.LibraryService.CreateBook",
    );

    let body = err.to_json();
    assert_eq!(body["error"]["code"], json!(400));
    assert_eq!(
        body["error"]["message"],
        json!("Request contains 2 invalid fields.")
    );

    let bad_request = body["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["@type"] == json!("type.googleapis.com/google.rpc.BadRequest"))
        .expect("a BadRequest detail");

    let violations = bad_request["fieldViolations"].as_array().unwrap();
    assert_eq!(violations.len(), 2);
    // The protojson path, which is the name the client actually sent.
    assert_eq!(violations[0]["field"], json!("book.displayName"));
    assert_eq!(violations[0]["reason"], json!("VALUE_LENGTH"));
}

#[test]
fn localized_message_is_preferred_for_the_rendered_message() {
    let err = Error::new(Code::NotFound, "not found")
        .with_detail(Detail::LocalizedMessage(crate::error::LocalizedMessage {
            locale: "fr-FR".into(),
            message: "Livre introuvable.".into(),
        }))
        .ensure_error_info(DOMAIN);

    assert_eq!(
        err.to_json()["error"]["message"],
        json!("Livre introuvable.")
    );
}

#[test]
fn from_tonic_status_maps_code_and_seeds_error_info() {
    let status = tonic::Status::permission_denied("no access to shelf s1");
    let err = Error::from_status(&status, DOMAIN);

    assert_eq!(err.code, Code::PermissionDenied);
    assert_eq!(err.http, StatusCode::FORBIDDEN);

    let body = err.to_json();
    assert_eq!(body["error"]["code"], json!(403));
    assert_eq!(body["error"]["status"], json!("PERMISSION_DENIED"));
    assert_eq!(body["error"]["details"][0]["domain"], json!(DOMAIN));
}
