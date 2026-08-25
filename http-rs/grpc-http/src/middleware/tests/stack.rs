//! Chain ordering and selector-gated layering.

use super::fixtures::Fixture;
use crate::error::{Code, GatewayError, Result};
use crate::middleware::{
    CallCx, Interceptor, MethodPattern, Outcome, ResponseParts, RouteCx, Selector, Stack,
};
use std::sync::{Arc, Mutex};

/// Records the order phases ran in.
type Log = Arc<Mutex<Vec<String>>>;

/// An interceptor that appends its name to a shared log at every phase.
struct Recorder {
    name: &'static str,
    log: Log,
}

impl Recorder {
    fn new(name: &'static str, log: &Log) -> Self {
        Self {
            name,
            log: Arc::clone(log),
        }
    }

    fn note(&self, phase: &str) {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:{phase}", self.name));
    }
}

impl Interceptor for Recorder {
    fn name(&self) -> &'static str {
        self.name
    }

    fn on_route(&self, _: &mut RouteCx<'_>) -> Result<()> {
        self.note("route");
        Ok(())
    }

    fn on_response(&self, _: &mut CallCx<'_>, _: &mut ResponseParts) -> Result<()> {
        self.note("response");
        Ok(())
    }

    fn on_complete(&self, _: &CallCx<'_>, _: &Outcome<'_>) {
        self.note("complete");
    }
}

/// An interceptor that always rejects.
struct Reject;

impl Interceptor for Reject {
    fn name(&self) -> &'static str {
        "reject"
    }

    fn on_route(&self, _: &mut RouteCx<'_>) -> Result<()> {
        Err(Box::new(GatewayError::new(Code::PermissionDenied, "no")))
    }
}

#[test]
fn request_phases_run_forwards_and_response_phases_backwards() {
    // A stack should nest: the first layer added is outermost, so it sees the
    // request first and the response last.
    let log: Log = Arc::default();
    let stack = Stack::new()
        .layer(Recorder::new("outer", &log))
        .layer(Recorder::new("inner", &log));

    let fixture = Fixture::get();
    let mut route = fixture.route("GetArtist", MethodPattern::Get);
    stack.on_route(&mut route).unwrap();

    let mut call = fixture.call("GetArtist", MethodPattern::Get);
    let mut parts = ResponseParts::ok();
    stack.on_response(&mut call, &mut parts).unwrap();
    stack.on_complete(&call, &Outcome::Success(http::StatusCode::OK));

    assert_eq!(
        log.lock().unwrap().as_slice(),
        [
            "outer:route",
            "inner:route",
            "inner:response",
            "outer:response",
            "inner:complete",
            "outer:complete",
        ]
    );
}

#[test]
fn a_rejection_stops_the_chain() {
    let log: Log = Arc::default();
    let stack = Stack::new()
        .layer(Reject)
        .layer(Recorder::new("after", &log));

    let fixture = Fixture::get();
    let mut route = fixture.route("GetArtist", MethodPattern::Get);

    let err = stack.on_route(&mut route).unwrap_err();
    assert_eq!(err.code, Code::PermissionDenied);
    // The layer after the rejection never ran.
    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn a_selector_gates_which_methods_a_layer_runs_for() {
    let log: Log = Arc::default();
    let stack = Stack::new().layer_on(Recorder::new("writes", &log), Selector::Mutating);
    let fixture = Fixture::post();

    let mut read = fixture.route("GetArtist", MethodPattern::Get);
    stack.on_route(&mut read).unwrap();
    assert!(log.lock().unwrap().is_empty(), "must skip a read");

    let mut write = fixture.route("CreateArtist", MethodPattern::Create);
    stack.on_route(&mut write).unwrap();
    assert_eq!(log.lock().unwrap().as_slice(), ["writes:route"]);
}

#[test]
fn names_report_the_chain_outermost_first() {
    let log: Log = Arc::default();
    let stack = Stack::new()
        .layer(Recorder::new("first", &log))
        .layer(Recorder::new("second", &log));
    assert_eq!(stack.names(), vec!["first", "second"]);
}
