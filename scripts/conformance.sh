#!/usr/bin/env bash
#
# Asks the Rust and Go runtimes the same questions and fails on any difference.
#
# This is the check the whole project is arranged to make possible. Both servers
# execute a route table generated from one IR by one generator, so a divergence
# here means either a runtime implements the protocol wrongly or the two targets
# emitted tables that disagree — and neither shows up in a unit test, because
# each runtime's own tests are written against its own behaviour.
#
# Headers and bodies are compared separately, each normalised only where a
# difference is legitimate and stated:
#
#   - Header names are lowercased, because HTTP/1.1 header names are
#     case-insensitive and hyper and net/http differ only in presentation.
#   - Date and Content-Length are dropped: one is a clock, the other follows
#     from whitespace that is itself not significant.
#   - JSON bodies are re-serialised canonically with jq. Go's protojson
#     deliberately varies its whitespace to stop callers byte-comparing its
#     output, so comparing raw bytes would report a difference on every run
#     while hiding the ones that matter. Key order, values and escaping are all
#     still compared.
#
# Two scopes, because they answer different questions:
#
#   check_same    for responses the *transcoder* originates — routing,
#                 negotiation, binding, streaming. These are what the runtime
#                 decides, so they must agree byte for byte.
#
#   check_service for responses a *service* originates. README §5.4 draws the
#                 same line. The two examples are two implementations of one
#                 catalog, and each is free to attach its own google.rpc details
#                 to a NOT_FOUND; what they may not disagree about is the status
#                 and the envelope the transcoder wraps it in.
set -euo pipefail

GO_PORT="${GO_PORT:-18080}"
RS_PORT="${RS_PORT:-18081}"
RS_TLS_PORT="${RS_TLS_PORT:-18443}"

GO_URL="http://127.0.0.1:${GO_PORT}"
RS_URL="http://127.0.0.1:${RS_PORT}"

# The request the transport matrix compares. Any route would do; a resource with
# a multi-segment capture exercises the matcher as well as the transport.
TRANSPORT_PATH="/v1/artists/miles/tracks/so-what"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

failures=0
go_pid=""
rs_pid=""

# The comparison machinery: how two answers are compared, and what counts as a
# legitimate difference between them.
# shellcheck source=scripts/lib/compare.sh
source "${root}/scripts/lib/compare.sh"

echo "building both runtimes"
go build -o target/music-go ./examples/music-go/cmd/music-server
cargo build -q -p music-example --bin music-server

echo "starting servers"
./target/music-go -addr "127.0.0.1:${GO_PORT}" >/dev/null 2>&1 &
go_pid=$!
MUSIC_HTTP_ADDR="127.0.0.1:${RS_PORT}" MUSIC_TLS_ADDR="127.0.0.1:${RS_TLS_PORT}" \
  ./target/debug/music-server >/dev/null 2>&1 &
rs_pid=$!

wait_for "$GO_URL" "go"
wait_for "$RS_URL" "rust"

echo
echo "routing"
check_same "GET a resource"            __URL__/v1/artists/miles
check_same "multi-segment capture"     __URL__/v1/artists/miles/tracks/so-what
check_same "list"                      __URL__/v1/artists
check_same "nested list"               __URL__/v1/artists/miles/tracks
check_same "no route matches"          __URL__/v1/nothing
check_same "wrong method (405+Allow)"  -X PUT __URL__/v1/artists/miles

# Service-originated: the path resolved and the catalog answered "no such
# artist". Both must agree it is a 404 NOT_FOUND carrying an ErrorInfo; the rest
# of the details are the catalog's to choose.
check_service "unknown resource"       /v1/artists/nobody
check_service "unregistered verb kept" /v1/artists/miles:unknown
check_service "encoded slash kept"     /v1/artists/a%2Fb

echo
echo "binding and negotiation"
check_same "unknown query parameter"   "__URL__/v1/artists?pagesize=2"
check_same "unparsable query value"    "__URL__/v1/artists?pageSize=many"
check_same "unsupported media type"    -X POST -H 'Content-Type: application/xml' -d '<a/>' __URL__/v1/artists
check_same "unsatisfiable Accept"      -H 'Accept: application/xml' __URL__/v1/artists/miles

echo
echo "transports — one handler, three ways in"
check_transports

echo
echo "streaming — README §6.2"
check_same "failure before commit"     "__URL__/v1/artists/miles/tracks:watch?failAfter=0"
check_exit "clean stream"              "/v1/artists/miles/tracks:watch"
check_exit "failure after commit"      "/v1/artists/miles/tracks:watch?failAfter=1"

echo
if [ "$failures" -gt 0 ]; then
  echo "FAILED: ${failures} difference(s) between the runtimes" >&2
  exit 1
fi
echo "the runtimes agree"
