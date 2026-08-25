#!/usr/bin/env bash
#
# The comparison machinery the conformance run is built from.
#
# Split from the cases so each file has one job: this decides *how* two answers
# are compared and what counts as a legitimate difference; conformance.sh
# decides *what* to ask. A reader adding a case should not have to read the
# normalisation to do it.
#
# Sourced, not executed. Every function reads GO_URL, RS_URL, RS_TLS_PORT and
# TRANSPORT_PATH from the caller, and reports by incrementing `failures`.

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

# check_service asks both servers a question whose answer the *service* decides,
# and compares only what the transcoder is responsible for: the status line, and
# the AIP-193 envelope's own fields.
#
# The details array is deliberately not compared. A service chooses what to
# attach — the Go catalog adds a ResourceInfo where the Rust one does not — and
# holding two demo catalogs to identical google.rpc details would test the
# examples rather than the protocol.
check_service() {
  local label="$1" path="$2"
  local go_out rs_out
  go_out="$(service_shape "${GO_URL}${path}")"
  rs_out="$(service_shape "${RS_URL}${path}")"

  if [ "$go_out" = "$rs_out" ]; then
    echo "  ok    ${label}"
    return
  fi
  echo "  DIFF  ${label}"
  diff <(echo "$rs_out") <(echo "$go_out") | sed 's/^/        /' || true
  failures=$((failures + 1))
}

# service_shape prints the status line and the envelope fields the transcoder
# owns, dropping the details the service chose.
service_shape() {
  local headers body
  headers="$(mktemp)"
  body="$(mktemp)"
  curl -s -D "$headers" -o "$body" "$1" || true

  head -n1 "$headers" | tr -d '\r'
  jq -S '{code: .error.code, status: .error.status, hasErrorInfo:
          ([.error.details[]? | select(."@type" | endswith("ErrorInfo"))] | length)}' \
    <"$body" 2>/dev/null || cat "$body"
  rm -f "$headers" "$body"
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

# check_transports proves the same handler answers identically however a client
# reaches it: HTTP/1.1 in the clear, HTTP/1.1 over TLS 1.3, and HTTP/3 over
# QUIC. The Rust example serves all three from one `Handler` value, so a
# difference between them would mean a listener is interpreting the response
# rather than carrying it.
#
# There is no plaintext HTTP/3 row and there cannot be: QUIC embeds TLS 1.3 in
# its transport handshake.
check_transports() {
  local plain tls
  plain="$(curl -s "${RS_URL}${TRANSPORT_PATH}" | jq -S .)"
  tls="$(curl -sk "https://127.0.0.1:${RS_TLS_PORT}${TRANSPORT_PATH}" | jq -S .)"

  if [ "$plain" = "$tls" ]; then
    echo "  ok    http/1.1 over TLS matches plaintext"
  else
    echo "  DIFF  http/1.1 over TLS differs from plaintext"
    diff <(echo "$plain") <(echo "$tls") | sed 's/^/        /' || true
    failures=$((failures + 1))
  fi

  # HTTP/3 needs a curl built against a QUIC library, which the system curl on
  # macOS is not. Skipping is stated rather than silent — and the transport is
  # still covered, by a real h3 client in examples/music-rs/tests/http3.rs that
  # asserts byte-identity against HTTP/1.1. `cargo test --all-features` runs it.
  if ! curl --version | grep -q HTTP3; then
    echo "  skip  http/3: this curl has no HTTP/3 (covered by tests/http3.rs)"
    return
  fi

  local h3
  h3="$(curl -sk --http3-only "https://127.0.0.1:${RS_TLS_PORT}${TRANSPORT_PATH}" | jq -S .)"
  if [ "$h3" = "$plain" ]; then
    echo "  ok    http/3 over QUIC matches plaintext"
  else
    echo "  DIFF  http/3 over QUIC differs from plaintext"
    diff <(echo "$plain") <(echo "$h3") | sed 's/^/        /' || true
    failures=$((failures + 1))
  fi
}
