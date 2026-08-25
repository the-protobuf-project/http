//! The protojson codec.

use crate::codec::{Codec, CodecError, Decode, Encode, Framing, JsonCodec};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};

/// Stands in for a generated message with protojson `Serialize` impls.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Book {
    #[serde(default)]
    name: String,
    #[serde(default)]
    display_name: String,
    /// protojson renders a 64-bit integer as a string, so the generated impls
    /// carry that mapping and the codec stays unaware of it.
    #[serde(default)]
    page_count: String,
}

#[test]
fn identity_holds() {
    let codec = JsonCodec::new();
    let book = Book {
        name: "shelves/s1/books/b1".into(),
        display_name: "Dune".into(),
        page_count: "412".into(),
    };

    let mut out = BytesMut::new();
    codec.encode(&book, &mut out).unwrap();

    let decoded: Book = codec.decode(&out).unwrap();
    assert_eq!(decoded, book);
}

#[test]
fn field_names_are_camel_case() {
    let mut out = BytesMut::new();
    JsonCodec::new()
        .encode(
            &Book {
                display_name: "Dune".into(),
                ..Default::default()
            },
            &mut out,
        )
        .unwrap();

    let text = String::from_utf8(out.to_vec()).unwrap();
    assert!(text.contains("\"displayName\""), "{text}");
    assert!(!text.contains("display_name"), "{text}");
}

#[test]
fn an_empty_body_is_a_default_message() {
    // `POST /v1/books` with no body means every field at its default, which is
    // a valid request rather than a malformed one.
    let decoded: Book = JsonCodec::new().decode(b"").unwrap();
    assert_eq!(decoded, Book::default());
}

#[test]
fn invalid_syntax_is_malformed() {
    let err: CodecError = Decode::<Book>::decode(&JsonCodec::new(), b"{not json").unwrap_err();
    assert!(
        matches!(err, CodecError::Malformed { .. }),
        "expected Malformed, got {err:?}"
    );
}

#[test]
fn an_unknown_field_is_named() {
    // Rejected rather than ignored: a typo in an update call should not be a
    // silent no-op, which is what grpc-gateway does with query parameters.
    let err: CodecError =
        Decode::<Book>::decode(&JsonCodec::new(), br#"{"titel":"Dune"}"#).unwrap_err();

    match err {
        CodecError::UnknownField { path } => assert_eq!(path, "titel"),
        other => panic!("expected UnknownField, got {other:?}"),
    }
}

#[test]
fn a_type_mismatch_is_a_field_error() {
    let err: CodecError =
        Decode::<Book>::decode(&JsonCodec::new(), br#"{"name": 42}"#).unwrap_err();
    assert!(
        matches!(err, CodecError::Field { .. }),
        "expected Field, got {err:?}"
    );
}

#[test]
fn pretty_printing_is_opt_in() {
    let book = Book::default();

    let mut compact = BytesMut::new();
    JsonCodec::new().encode(&book, &mut compact).unwrap();
    assert!(!compact.contains(&b'\n'));

    let mut pretty = BytesMut::new();
    JsonCodec::pretty().encode(&book, &mut pretty).unwrap();
    assert!(pretty.contains(&b'\n'));
}

#[test]
fn encoding_appends_so_a_stream_reuses_one_buffer() {
    let codec = JsonCodec::new();
    let mut out = BytesMut::new();

    codec.encode(&Book::default(), &mut out).unwrap();
    let first = out.len();
    codec.encode(&Book::default(), &mut out).unwrap();

    assert_eq!(out.len(), first * 2, "the second encode must not overwrite");
}

#[test]
fn metadata_matches_the_protocol_registry() {
    let codec = JsonCodec::new();
    assert_eq!(codec.name(), "json");
    assert_eq!(codec.content_type(), "application/json");
    assert_eq!(codec.framing(), Framing::JsonArray);
}
