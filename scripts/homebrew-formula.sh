#!/bin/sh
# Print the Homebrew formula for a release: `brew install viktoo/tap/pocket`
# pours the bottle that scripts/bundle-macos.sh packs (a bottle needs no
# up-to-date command line tools; the plain tarball is the fallback).
# Usage: scripts/homebrew-formula.sh VERSION TARBALL_SHA256 BOTTLE_SHA256 [BASE_URL]
# (BASE_URL defaults to the GitHub release of tag vVERSION.)
set -eu

version=$1
sha=$2
bottle_sha=$3
base=${4:-https://github.com/Viktoo/infinite-pocket/releases/download/v$version}

cat <<EOF
# Written by scripts/homebrew-formula.sh in Viktoo/infinite-pocket on each release.
class Pocket < Formula
  desc "Infinite 3D world written by an AI while you walk through it"
  homepage "https://github.com/Viktoo/infinite-pocket"
  url "$base/pocket-$version-macos-arm64.tar.gz"
  version "$version"
  sha256 "$sha"

  bottle do
    root_url "$base"
    sha256 cellar: :any_skip_relocation, all: "$bottle_sha"
  end

  depends_on arch: :arm64
  depends_on macos: :ventura

  def install
    bin.install "pocket"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pocket version")
  end
end
EOF
