#!/usr/bin/env bash
# Package a dewey release artifact.
#
# Usage: package.sh <tag> <artifact-suffix> <binary> <out-dir>
#   tag          release tag, e.g. v0.1.0
#   artifact     platform suffix, e.g. linux-x86_64
#   binary       path to the built binary (target/release/dewey)
#   out-dir      directory to write dewey-<artifact>-v<version>.tar.gz into
set -euo pipefail

tag=$1
artifact=$2
binary=$3
out=$4

version=${tag#v}
if [ "$version" = "$tag" ]; then
    echo "error: tag must start with 'v' (got: $tag)" >&2
    exit 1
fi

mkdir -p "$out"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pkg="$tmp/dewey-$version"
mkdir -p "$pkg"
cp "$binary" "$pkg/dewey"
cp README.md LICENSE.md "$pkg/"

tar -czf "$out/dewey-$artifact-v$version.tar.gz" -C "$tmp" "dewey-$version"
echo "wrote $out/dewey-$artifact-v$version.tar.gz"
