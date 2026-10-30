//! A three-method API, written in the form `protoc-gen-http` emits, and a
//! dispatcher that serves it from memory.

use crate::codec::{CodecEntry, CodecRegistry, Framing};
use crate::error::{Code, Error, Result};
use crate::gateway::{Call, Dispatcher, Gateway, GatewayBody, GatewayBuilder, Reply, Stream};
use crate::middleware::MethodPattern;
use crate::route::{Capture, Match, MethodInfo, Route, RouteTable};
use bytes::Bytes;
use http::{Request, Response};
use http_body_util::{BodyExt, Full};
use serde::{Deserialize, Serialize};

pub const DOMAIN: &str = "music.example.com";

pub const GET_ARTIST: usize = 0;
pub const CREATE_ARTIST: usize = 1;
pub const WATCH_ARTISTS: usize = 2;

static ARTISTS: &[Match] = &[Match::Literal("v1"), Match::Literal("artists")];
static ARTISTS_ANY: &[Match] = &[
    Match::Literal("v1"),
    Match::Literal("artists"),
    Match::Single,
];
static NONE: &[Capture] = &[];
static CAP_NAME_1_3: &[Capture] = &[Capture {
    field: &["name"],
    json: "name",
    start: 1,
    end: 3,
}];

pub static ROUTES: &[Route] = &[
    Route {
        method: "GET",
        segments: ARTISTS,
        verb: "watch",
        captures: NONE,
        template: "/v1/artists:watch",
        handler: WATCH_ARTISTS,
    },
    Route {
        method: "GET",
        segments: ARTISTS_ANY,
        verb: "",
        captures: CAP_NAME_1_3,
        template: "/v1/{name=artists/*}",
        handler: GET_ARTIST,
    },
    Route {
        method: "POST",
        segments: ARTISTS,
        verb: "",
        captures: NONE,
        template: "/v1/artists",
        handler: CREATE_ARTIST,
    },
];

pub static METHODS: &[MethodInfo] = &[
    MethodInfo {
        name: "GetArtist",
        full_name: "music.v1.ArtistService.GetArtist",
        service: "music.v1.ArtistService",
        pattern: MethodPattern::Get,
        mutating: false,
        server_stream: false,
    },
    MethodInfo {
        name: "CreateArtist",
        full_name: "music.v1.ArtistService.CreateArtist",
        service: "music.v1.ArtistService",
        pattern: MethodPattern::Create,
        mutating: true,
        server_stream: false,
    },
    MethodInfo {
        name: "WatchArtists",
        full_name: "music.v1.ArtistService.WatchArtists",
        service: "music.v1.ArtistService",
        pattern: MethodPattern::Custom,
        mutating: false,
        server_stream: true,
    },
];

pub static CODECS: &[CodecEntry] = &[CodecEntry {
    name: "json",
    media_types: &["application/json"],
    framing: Framing::JsonArray,
    index: 0,
}];

/// A message as the generator would leave it: a prost type with protojson
/// serde impls.
#[derive(Clone, PartialEq, prost::Message, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub display_name: String,
}

/// Serves the fixture API. `?failAfter=N` on the watch fails the stream after
/// N messages; `?panic` on a get panics.
#[derive(Debug, Default)]
pub struct Catalog;

impl Dispatcher for Catalog {
    async fn dispatch(&self, call: &Call<'_>) -> Result<Reply> {
        match call.handler() {
            GET_ARTIST => {
                call.reject_unknown_query(&["view", "panic"])?;
                assert!(!call.query.has("panic"), "asked to panic");
                let name = call.capture("name")?;
                if name != "artists/miles" {
                    return Err(Box::new(Error::from_status(
                        &tonic::Status::not_found(format!("{name} does not exist.")),
                        call.domain,
                    )));
                }
                call.ok(&Artist {
                    name: name.to_string(),
                    display_name: "Miles Davis".into(),
                })
            }
            CREATE_ARTIST => {
                let artist: Artist = call.decode()?;
                call.created(&artist, &format!("/v1/{}", artist.name))
            }
            other => Err(Box::new(Error::binding_mismatch(
                &other.to_string(),
                call.domain,
                call.method.full_name,
            ))),
        }
    }

    async fn dispatch_stream(&self, call: &Call<'_>, out: &mut Stream<'_>) -> Result<()> {
        let fail_after: Option<usize> = call.query.get("failAfter").and_then(|v| v.parse().ok());
        for (sent, name) in ["miles", "coltrane"].into_iter().enumerate() {
            if fail_after.is_some_and(|limit| sent >= limit) {
                return Err(call.error(Code::Unavailable, "The catalog went away."));
            }
            out.send(&Artist {
                name: format!("artists/{name}"),
                display_name: name.into(),
            })
            .await?;
        }
        if fail_after.is_some() {
            return Err(call.error(Code::Unavailable, "The catalog went away."));
        }
        Ok(())
    }
}

/// A dispatcher that serves no streams, to prove the default rejects them.
#[derive(Debug, Default)]
pub struct UnaryOnly;

impl Dispatcher for UnaryOnly {
    async fn dispatch(&self, call: &Call<'_>) -> Result<Reply> {
        call.ok(&Artist {
            name: String::new(),
            display_name: String::new(),
        })
    }
}

pub fn builder<D: Dispatcher>(dispatch: D) -> GatewayBuilder<D> {
    Gateway::builder(
        RouteTable::with_methods(ROUTES, METHODS),
        CodecRegistry::new(CODECS),
        dispatch,
        DOMAIN,
    )
}

pub fn gateway() -> Gateway<Catalog> {
    builder(Catalog).build()
}

pub fn get(uri: &str) -> Request<Full<Bytes>> {
    Request::get(uri).body(Full::new(Bytes::new())).unwrap()
}

pub fn post(uri: &str, content_type: &str, body: &'static str) -> Request<Full<Bytes>> {
    Request::post(uri)
        .header("content-type", content_type)
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap()
}

/// A response with its body read to the end.
pub struct Collected {
    pub status: http::StatusCode,
    pub headers: http::HeaderMap,
    pub body: Bytes,
    pub trailers: Option<http::HeaderMap>,
}

impl Collected {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    pub fn reason(&self) -> String {
        let json = self.json();
        json["error"]["details"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["@type"] == "type.googleapis.com/google.rpc.ErrorInfo")
            .map(|d| d["reason"].as_str().unwrap().to_string())
            .unwrap()
    }
}

pub async fn collect(response: Response<GatewayBody>) -> Collected {
    let (parts, body) = response.into_parts();
    let collected = body.collect().await.expect("the body completed");
    let trailers = collected.trailers().cloned();
    Collected {
        status: parts.status,
        headers: parts.headers,
        body: collected.to_bytes(),
        trailers,
    }
}
