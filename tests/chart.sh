#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
chart=$root/charts/gilti

mise run chart -- --chart "$chart" --lint-only
helm template gilti "$chart" >/dev/null

rendered=$(mktemp)
invalid=$(mktemp)
injected=$(mktemp)
ssh_disabled=$(mktemp)
trap 'rm -f "$rendered" "$invalid" "$injected" "$ssh_disabled"' EXIT

helm template gilti "$chart" \
    --set-string 'ssh.authorizedKeys[0]=ssh-ed25519 AAAAcharttest gilti' \
    --set 'http.hostnames[0]=git.example.test' \
    --set-string config.lfs.max_object_bytes=2GiB \
    --set httpRoute.enabled=true \
    --set 'httpRoute.hostnames[0]=git.example.test' \
    --set 'httpRoute.parentRefs[0].name=public' \
    --set sshRoute.enabled=true \
    --set 'sshRoute.parentRefs[0].name=public' >"$rendered"

grep -q '^kind: HTTPRoute$' "$rendered"
grep -q '^kind: TCPRoute$' "$rendered"
grep -q '^apiVersion: gateway.networking.k8s.io/v1$' "$rendered"
grep -q 'helm.sh/resource-policy: keep' "$rendered"
grep -q 'ssh-ed25519 AAAAcharttest gilti' "$rendered"
grep -q 'mountPath: /etc/gilti/authorized_keys' "$rendered"
grep -q 'mountPath: /etc/gilti/config.json' "$rendered"
grep -Fq '"hostnames": [' "$rendered"
grep -Fq '"git.example.test"' "$rendered"
grep -Fq '"ssh": true' "$rendered"
grep -Fq '"formats": [' "$rendered"
grep -Fq '"tar.zst"' "$rendered"
grep -Fq '"max_object_bytes": "2GiB"' "$rendered"

helm template gilti "$chart" --set ssh.enabled=false >"$ssh_disabled"
grep -Fq '"ssh": false' "$ssh_disabled"
if grep -q 'name: GILTI_' "$ssh_disabled"; then
    echo 'chart rendered legacy configuration environment variables' >&2
    exit 1
fi
if grep -q 'containerPort: 2222' "$ssh_disabled" || grep -q '^kind: TCPRoute$' "$ssh_disabled"; then
    echo 'chart exposed SSH while ssh.enabled=false' >&2
    exit 1
fi

cat >"$injected" <<'EOF'
web:
  rootTitle: |-
    Gilti
    scan-path=/var/lib/gilti/git/repositories
EOF
if helm template gilti "$chart" -f "$injected" >"$invalid" 2>&1; then
    echo 'chart accepted multiline web configuration' >&2
    exit 1
fi

if helm template gilti "$chart" --set 'archive.formats[0]=tar.lz' >"$invalid" 2>&1; then
    echo 'chart accepted an unsupported archive format' >&2
    exit 1
fi

if helm template gilti "$chart" --set replicaCount=2 >"$invalid" 2>&1; then
    echo 'chart accepted unsupported replicaCount=2' >&2
    exit 1
fi
