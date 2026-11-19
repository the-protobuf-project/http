#!/usr/bin/env bash
#
# Prints the Homebrew cask for a protoc-gen-http release, from that release's
# SHA256SUMS. The release workflow writes it to the-protobuf-project/homebrew-tap
# as Casks/protoc-gen-http.rb, in the shape GoReleaser gives the other casks.
#
#   scripts/homebrew-cask.sh 0.2.1 dist/SHA256SUMS
set -euo pipefail
v=$1; sums=$2
sha() { awk -v f="protoc-gen-http-v${v}-$1.tar.gz" '{ sub(/^\.\//, "", $2) } $2 == f { print $1 }' "$sums"; }
cat <<CASK
cask "protoc-gen-http" do
  postflight_steps do
    on_macos do
      run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "protoc-gen-http"], chdir: "."
    end
  end

  version "${v}"

  on_macos do
    on_arm do
      sha256 "$(sha darwin-arm64)"
      url "https://github.com/the-protobuf-project/http/releases/download/v#{version}/protoc-gen-http-v#{version}-darwin-arm64.tar.gz"
    end
    on_intel do
      sha256 "$(sha darwin-amd64)"
      url "https://github.com/the-protobuf-project/http/releases/download/v#{version}/protoc-gen-http-v#{version}-darwin-amd64.tar.gz"
    end
  end
  on_linux do
    on_arm do
      sha256 "$(sha linux-arm64)"
      url "https://github.com/the-protobuf-project/http/releases/download/v#{version}/protoc-gen-http-v#{version}-linux-arm64.tar.gz"
    end
    on_intel do
      sha256 "$(sha linux-amd64)"
      url "https://github.com/the-protobuf-project/http/releases/download/v#{version}/protoc-gen-http-v#{version}-linux-amd64.tar.gz"
    end
  end

  name "protoc-gen-http"
  desc "Generate HTTP/JSON transcoding for gRPC services from google.api.http"
  homepage "https://github.com/the-protobuf-project/http"

  livecheck do
    skip "Auto-generated on release."
  end

  binary "protoc-gen-http"

  zap :not_required
end
CASK
