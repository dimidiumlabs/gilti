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
[ -f git/Makefile ] || {
    echo 'Git submodule is not initialized; run git submodule update --init' >&2
    exit 1
}

workspace=$PWD
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT INT TERM

cp -R cgit "$build/cgit"
cp -R git "$build/cgit/git"
make -C "$build/cgit/git" -f ../Makefile -j"$(nproc)" ../cgit \
    NO_CURL=1 \
    NO_GETTEXT=1 \
    NO_OPENSSL=1 \
    NO_REGEX=NeedsStartEnd

CARGO_TARGET_DIR="$build/target" cargo build \
    --manifest-path "$workspace/Cargo.toml" \
    --locked \
    --release \
    --workspace

output="$workspace/.container/binary-$arch"
rm -rf "$output"
install -Dm0755 "$build/target/release/gilti" "$output/gilti"
install -Dm0755 "$build/target/release/gilti-ssh" "$output/gilti-ssh"
install -Dm0755 "$build/cgit/cgit" "$output/gilti-cgit"
strip "$output/gilti-cgit"

install -Dm0644 cgit/cgit.css "$output/cgit.css"
install -Dm0644 cgit/cgit.js "$output/cgit.js"
install -Dm0644 cgit/cgit.png "$output/cgit.png"
install -Dm0644 cgit/favicon.ico "$output/favicon.ico"
install -Dm0644 cgit/COPYING "$output/COPYING.cgit.txt"
install -Dm0644 git/COPYING "$output/COPYING.git.txt"
