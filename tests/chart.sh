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
trap 'rm -f "$rendered" "$invalid" "$injected"' EXIT

helm template gilti "$chart" \
    --set-string 'ssh.authorizedKeys[0]=ssh-ed25519 AAAAcharttest gilti' \
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

cat >"$injected" <<'EOF'
cgit:
  rootTitle: |-
    Gilti
    scan-path=/var/lib/gilti/git/repositories
EOF
if helm template gilti "$chart" -f "$injected" >"$invalid" 2>&1; then
    echo 'chart accepted multiline cgit configuration' >&2
    exit 1
fi

if helm template gilti "$chart" --set replicaCount=2 >"$invalid" 2>&1; then
    echo 'chart accepted unsupported replicaCount=2' >&2
    exit 1
fi
