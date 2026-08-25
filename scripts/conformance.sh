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
set -euo pipefail

GO_PORT="${GO_PORT:-18080}"
RS_PORT="${RS_PORT:-18081}"
RS_TLS_PORT="${RS_TLS_PORT:-18443}"

GO_URL="http://127.0.0.1:${GO_PORT}"
RS_URL="http://127.0.0.1:${RS_PORT}"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

failures=0
go_pid=""
rs_pid=""

# stop kills both servers on any exit path, including a failed assertion, so a
# non-zero exit never leaves a port bound for the next run.
stop() {
  [ -n "$go_pid" ] && kill "$go_pid" 2>/dev/null || true
  [ -n "$rs_pid" ] && kill "$rs_pid" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap stop EXIT

# wait_for blocks until a server answers, or gives up.
#
# Polling rather than a fixed sleep: a fixed sleep is either too short on a busy
# CI runner, where it produces a flake nobody can reproduce, or too long
# everywhere else.
wait_for() {
  local url="$1" name="$2" attempt=0
  until curl -sf -o /dev/null "${url}/v1/artists"; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ]; then
      echo "FATAL: ${name} did not become ready at ${url}" >&2
      exit 1
    fi
    sleep 0.1
  done
}

# fetch asks one server a question, printing its status line, its normalised
# headers, and its body re-serialised canonically.
#
# perl rather than sed for the header casing: \L is a GNU extension, and BSD sed
# emits a literal "L" instead of lowercasing — silently, which would have made
# every header line differ for a reason that is not a real difference.
fetch() {
  local headers body
  headers="$(mktemp)"
  body="$(mktemp)"
  curl -s -D "$headers" -o "$body" "$@" || true

  perl -pe 's/^([A-Za-z0-9-]+):/lc($1).":"/e' <"$headers" \
    | grep -viE '^(date|content-length):' \
    | tr -d '\r'
  # A body that is not JSON is compared as it arrived: an empty 204, or a
  # framing whose bytes are the point.
  jq -S . <"$body" 2>/dev/null || cat "$body"
  rm -f "$headers" "$body"
}

# check_same asks both servers one question and diffs the answers.
check_same() {
  local label="$1"; shift
  local go_out rs_out
  go_out="$(fetch "${@/__URL__/$GO_URL}")"
  rs_out="$(fetch "${@/__URL__/$RS_URL}")"

  if [ "$go_out" = "$rs_out" ]; then
    echo "  ok    ${label}"
    return
  fi
  echo "  DIFF  ${label}"
  diff <(echo "$rs_out") <(echo "$go_out") | sed 's/^/        /' || true
  failures=$((failures + 1))
}

# check_exit asserts both clients exit the same way, which is how a truncated
# stream is observed: curl exits 18 on a body that ends early.
check_exit() {
  local label="$1" path="$2"
  local go_code=0 rs_code=0
  curl -s -o /dev/null "${GO_URL}${path}" || go_code=$?
  curl -s -o /dev/null "${RS_URL}${path}" || rs_code=$?

  if [ "$go_code" = "$rs_code" ]; then
    echo "  ok    ${label} (curl exit ${go_code})"
    return
  fi
  echo "  DIFF  ${label}: rust exit ${rs_code}, go exit ${go_code}"
  failures=$((failures + 1))
}

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
check_same "unknown resource"          __URL__/v1/artists/nobody
check_same "wrong method (405+Allow)"  -X PUT __URL__/v1/artists/miles
check_same "unregistered verb"         __URL__/v1/artists/miles:unknown
check_same "encoded slash kept"        __URL__/v1/artists/a%2Fb

echo
echo "binding and negotiation"
check_same "unknown query parameter"   "__URL__/v1/artists?pagesize=2"
check_same "unparsable query value"    "__URL__/v1/artists?pageSize=many"
check_same "unsupported media type"    -X POST -H 'Content-Type: application/xml' -d '<a/>' __URL__/v1/artists
check_same "unsatisfiable Accept"      -H 'Accept: application/xml' __URL__/v1/artists/miles

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
