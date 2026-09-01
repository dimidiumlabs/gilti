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
ldd --version 2>&1 | grep -q musl || {
    echo 'container artifacts must be built in a musl environment' >&2
    exit 1
}

workspace=$PWD
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT INT TERM

CARGO_TARGET_DIR="$build/target" cargo build \
    --manifest-path "$workspace/Cargo.toml" \
    --locked \
    --release \
    --workspace
output="$workspace/.container/binary-$arch"
rm -rf "$output"
install -Dm0755 "$build/target/release/gilti" "$output/gilti"
install -Dm0755 "$build/target/release/gilti-ssh" "$output/gilti-ssh"
install -Dm0644 crates/gilti/assets/gilti.png "$output/gilti.png"
install -Dm0644 crates/gilti/assets/favicon.ico "$output/favicon.ico"
