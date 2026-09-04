#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later

set -eu
umask 022

arch=${1:?usage: build-artifacts.sh ARCH}
case $(uname -m) in
    x86_64) native_arch=amd64 ;;
    aarch64) native_arch=arm64 ;;
    *) echo "unsupported build architecture: $(uname -m)" >&2; exit 1 ;;
esac
[ "$arch" = "$native_arch" ] || {
    echo "requested $arch artifacts on $native_arch" >&2
    exit 1
}
workspace=$PWD

# Keep Cargo's normal target directory. CI restores it through rust-cache, so
# this build only links artifacts not already produced by the preceding checks.
cargo build \
    --manifest-path "$workspace/Cargo.toml" \
    --locked \
    --release \
    --package gilti
output="$workspace/.container/binary-$arch"
rm -rf "$output"
install -Dm0755 "$workspace/target/release/gilti" "$output/gilti"
