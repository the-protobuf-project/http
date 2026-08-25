# Task runner. `just` with no target lists everything.

_default:
    @just --list


proto_dir := "examples/protobuf"
descriptor := "target/music.binpb"

# Build the descriptor set. buf resolves googleapis from buf.lock, so no
# --proto-path is threaded through by hand and nothing is vendored.
[group('proto')]
build-protos:
    @mkdir -p target
    cd {{proto_dir}} && buf build -o ../../{{descriptor}}

# Lint the protos with both linters.
#
# They check different things and disagree in two places, which buf.yaml
# documents: buf covers style and wire-compatibility hazards, api-linter covers
# AIP conformance, and AIP wins where they conflict.
# api-linter runs from the module root because core::0191::proto-package
# compares a file's path to its package: from the repository root it would see
# examples/protobuf/music/v1/music.proto against package music.v1 and report a
# mismatch that is not there.
[group('proto')]
lint-protos: build-protos
    cd {{proto_dir}} && buf lint
    cd {{proto_dir}} && api-linter \
        --descriptor-set-in ../../{{descriptor}} \
        --config api-linter.yaml \
        --set-exit-status \
        $(find music -name '*.proto' | sort)

# Format the protos in place.
[group('proto')]
fmt-protos:
    cd {{proto_dir}} && buf format -w

# Check for breaking changes against the git main branch.
[group('proto')]
breaking-protos:
    cd {{proto_dir}} && buf breaking --against '../../.git#branch=main,subdir={{proto_dir}}'

# Refresh the vendored deps the VS Code api-linter extension reads.
#
# The editor extension resolves imports from a path rather than from buf, and
# the buf module cache is version-hashed so it moves on every update. This
# materializes the same deps somewhere stable. Not needed for the build.
[group('proto')]
vendor-protos:
    cd {{proto_dir}} && buf export . --output ../third_party
    rm -rf examples/third_party/music

# Build the plugin binary.
[group('proto')]
build-plugin:
    go build -o target/protoc-gen-http ./plugin/cmd/protoc-gen-http

# Regenerate the examples' messages and route tables from their protos.
#
# The output is committed, so a reviewer sees what changed in the generated
# surface when a proto changes rather than having to run the generator to find
# out.
#
# rustfmt runs here rather than inside the plugin: protoc-gen-go can call
# go/format in-process, but Rust has no in-process formatter, and requiring a
# Rust toolchain to generate protos would be a real burden on a CI job that only
# wants the Go or Python output.
[group('proto')]
gen: build-plugin
    cd {{proto_dir}} && buf generate
    rustfmt --edition 2024 examples/music-rs/src/generated/*.rs

# Alias kept for muscle memory; `gen` emits every target, not only Rust.
[group('proto')]
gen-rust: gen

# Fail if the committed generated code is stale.
#
# Covers every target: a change that regenerates one runtime's table and not the
# other is exactly the drift the agreement test exists to catch, and it should
# not be possible to commit it in the first place.
[group('ci')]
check-gen: gen
    git diff --exit-code examples/music-rs/src/generated examples/music-go/routes examples/music-go/gen || \
        (echo "generated code is stale; run 'just gen'" && exit 1)

[group('rust')]
build:
    cargo build --workspace --all-features

[group('rust')]
test:
    cargo test --workspace --all-features

# Lint Rust: format, clippy at deny, and the docs the workspace lints require.
[group('rust')]
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings

[group('rust')]
fix:
    cargo fmt --all
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty

# Test the Go runtime.
[group('go')]
test-go:
    go -C transcode-go test ./...

# Test the Go example end to end: routing, the error envelope, the middleware
# plane, and both halves of the no-false-2xx rule.
[group('go')]
test-example-go:
    go -C examples/music-go test ./...

# Test the generator, including the cross-target agreement and determinism
# checks in plugin/target.
[group('go')]
test-plugin:
    go test ./plugin/...

# Lint every Go module in the repository.
#
# gofmt -l prints offending files and exits 0, so the output is turned into a
# failure explicitly; a formatting job that cannot fail is not a check.
[group('go')]
lint-go:
    #!/usr/bin/env bash
    set -euo pipefail
    unformatted=$(gofmt -l transcode-go plugin examples/music-go)
    if [ -n "$unformatted" ]; then
        echo "gofmt needed:"
        echo "$unformatted"
        exit 1
    fi
    go -C transcode-go vet ./...
    go vet ./plugin/...
    go -C examples/music-go vet ./...

# Run the Go example server.
#
# From the repository root, which is what the committed go.work buys: every
# module is a workspace member, so a path relative to the root resolves.
[group('go')]
run-go *ARGS:
    go run ./examples/music-go/cmd/music-server {{ARGS}}

# What CI runs.
[group('ci')]
ci: lint-protos lint-go check-gen lint test test-go test-example-go test-plugin
