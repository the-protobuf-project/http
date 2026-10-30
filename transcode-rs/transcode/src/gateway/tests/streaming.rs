//! README §6.2: a failed stream is never reported as a success.

use super::fixtures::{UnaryOnly, builder, collect, gateway, get};
use http::StatusCode;
use http_body_util::BodyExt;

#[tokio::test]
async fn a_clean_stream_is_a_json_array_with_ok_trailers() {
    let reply = collect(gateway().handle(get("/v1/artists:watch")).await).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers["trailer"], "grpc-status, grpc-message");
    assert!(
        !reply.headers.contains_key("content-length"),
        "a stream's length is not known up front"
    );
    let messages = reply.json();
    assert_eq!(messages.as_array().unwrap().len(), 2);
    assert_eq!(messages[1]["name"], "artists/coltrane");
    assert_eq!(reply.trailers.unwrap()["grpc-status"], "0");
}

#[tokio::test]
async fn a_failure_before_the_first_message_keeps_its_status() {
    let reply = collect(gateway().handle(get("/v1/artists:watch?failAfter=0")).await).await;

    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply.json()["error"]["status"], "UNAVAILABLE");
}

#[tokio::test]
async fn a_failure_after_commit_truncates_the_body() {
    let response = gateway().handle(get("/v1/artists:watch?failAfter=1")).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the status line is spent"
    );

    let mut body = response.into_body();
    let mut data = Vec::new();
    let failure = loop {
        match body.frame().await {
            Some(Ok(frame)) => {
                if let Ok(chunk) = frame.into_data() {
                    data.extend_from_slice(&chunk);
                }
            }
            Some(Err(err)) => break err,
            None => panic!("the body ended cleanly, reporting a failed stream as a success"),
        }
    };

    assert_eq!(failure, crate::gateway::TruncatedStream);
    let text = String::from_utf8(data).unwrap();
    assert!(
        text.contains("artists/miles"),
        "the message before the failure arrived"
    );
    assert!(
        text.contains("UNAVAILABLE"),
        "the in-band error frame arrived: {text}"
    );
}

#[tokio::test]
async fn a_dispatcher_without_streams_answers_unimplemented() {
    let gateway = builder(UnaryOnly).build();
    let reply = collect(gateway.handle(get("/v1/artists:watch")).await).await;

    assert_eq!(reply.status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(reply.json()["error"]["status"], "UNIMPLEMENTED");
}
