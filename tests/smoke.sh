#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

image=${IMAGE:-gilti:dev}
engine=${CONTAINER_ENGINE:-docker}
name=gilti-smoke-$$
volume=$name-state
http_port=${HTTP_PORT:-18080}
ssh_port=${SSH_PORT:-12222}
work=$(mktemp -d)

remove_container() {
    "$engine" stop -t 10 "$name" >/dev/null 2>&1 || true
    "$engine" rm -f "$name" >/dev/null 2>&1 || true
}

cleanup() {
    remove_container
    "$engine" volume rm "$volume" >/dev/null 2>&1 || true
    rm -rf "$work"
}
trap cleanup EXIT INT TERM

ssh-keygen -q -t ed25519 -N '' -f "$work/admin"
ssh-keygen -q -t ed25519 -N '' -f "$work/stranger"
"$engine" volume create "$volume" >/dev/null

start() {
    key_mount=
    if [ "${1:-with-key}" = with-key ]; then
        key_mount="--mount type=bind,src=$work/admin.pub,dst=/run/gilti-bootstrap/admin.pub,readonly"
    fi
    # shellcheck disable=SC2086
    "$engine" run -d --name "$name" \
        --read-only \
        --cap-drop ALL \
        --cap-add CHOWN --cap-add DAC_OVERRIDE --cap-add FOWNER \
        --cap-add SETGID --cap-add SETUID --cap-add SYS_CHROOT \
        --tmpfs /run:rw,nosuid,nodev,noexec,size=32m \
        --tmpfs /tmp:rw,nosuid,nodev,noexec,size=256m \
        --tmpfs /var/cache/cgit:rw,nosuid,nodev,noexec,size=256m \
        --mount "type=volume,src=$volume,dst=/var/lib/gilti" \
        $key_mount \
        -p "127.0.0.1:$http_port:8080" -p "127.0.0.1:$ssh_port:2222" \
        "$image" >/dev/null

    i=0
    until curl -fsS "http://127.0.0.1:$http_port/healthz" >/dev/null 2>&1; do
        i=$((i + 1))
        if [ "$i" -ge 60 ]; then
            "$engine" logs "$name" >&2
            return 1
        fi
        sleep 1
    done
    i=0
    until ssh-keyscan -p "$ssh_port" 127.0.0.1 >/dev/null 2>&1; do
        i=$((i + 1))
        [ "$i" -lt 30 ] || { "$engine" logs "$name" >&2; return 1; }
        sleep 1
    done
}

start with-key
host_key_mode=$("$engine" exec "$name" stat -c '%u:%a' /var/lib/gilti/ssh/ssh_host_ed25519_key)
[ "$host_key_mode" = 0:600 ] || {
    echo "unexpected SSH host-key ownership/mode: $host_key_mode" >&2
    exit 1
}
if "$engine" exec --user 10000:10000 "$name" test -r /var/lib/gilti/ssh/ssh_host_ed25519_key; then
    echo 'Git/cgit user can read the SSH host private key' >&2
    exit 1
fi
ssh_opts="-o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=$work/known_hosts -i $work/admin -p $ssh_port"
# shellcheck disable=SC2086
ssh $ssh_opts git@127.0.0.1 info | grep -q 'hello admin'

if ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -i "$work/stranger" -p "$ssh_port" git@127.0.0.1 info >/dev/null 2>&1; then
    echo 'an unknown key was accepted' >&2
    exit 1
fi

GIT_SSH_COMMAND="ssh $ssh_opts" git clone \
    "ssh://git@127.0.0.1:$ssh_port/gitolite-admin" "$work/admin-repo"
(
    cd "$work/admin-repo"
    git config user.name 'Gilti smoke test'
    git config user.email 'smoke@gilti.invalid'
    cat >>conf/gitolite.conf <<'EOF'

repo testing
    R = gitweb

repo private
    RW+ = admin
EOF
    git add conf/gitolite.conf
    git commit -m 'Publish testing repository' >/dev/null
    GIT_SSH_COMMAND="ssh $ssh_opts" git push origin HEAD >/dev/null
)

i=0
until curl -fsS "http://127.0.0.1:$http_port/" | grep -q 'testing'; do
    i=$((i + 1))
    [ "$i" -lt 30 ] || { "$engine" logs "$name" >&2; exit 1; }
    sleep 1
done
if curl -fsS "http://127.0.0.1:$http_port/" | grep -q 'private'; then
    echo 'private repository appeared in the cgit index' >&2
    exit 1
fi
status=$(curl -sS -o /dev/null -w '%{http_code}' "http://127.0.0.1:$http_port/private/")
[ "$status" = 404 ] || {
    echo "private repository direct URL returned HTTP $status" >&2
    exit 1
}

fingerprint=$(ssh-keyscan -p "$ssh_port" 127.0.0.1 2>/dev/null | ssh-keygen -lf - | awk '{print $2}')
remove_container
start without-key
fingerprint_after=$(ssh-keyscan -p "$ssh_port" 127.0.0.1 2>/dev/null | ssh-keygen -lf - | awk '{print $2}')
[ "$fingerprint" = "$fingerprint_after" ] || {
    echo 'SSH host key changed after restart' >&2
    exit 1
}
curl -fsS "http://127.0.0.1:$http_port/" | grep -q 'testing'
