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
authorized_keys=$work/authorized_keys

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
ssh-keygen -q -t ed25519 -N '' -f "$work/admin-2"
ssh-keygen -q -t ed25519 -N '' -f "$work/stranger"
cat "$work/admin.pub" "$work/admin-2.pub" >"$authorized_keys"
"$engine" volume create "$volume" >/dev/null

printf '%s\n' 'not an SSH key' >"$work/bad-authorized-keys"
if "$engine" run --rm \
    --cap-drop ALL \
    --cap-add CHOWN --cap-add DAC_OVERRIDE --cap-add FOWNER \
    --cap-add SETGID --cap-add SETUID --cap-add SYS_CHROOT \
    --mount "type=volume,src=$volume,dst=/var/lib/gilti" \
    --mount "type=bind,src=$work/bad-authorized-keys,dst=/etc/gilti/authorized_keys,readonly" \
    "$image" >/dev/null 2>&1; then
    echo 'initialization accepted a malformed SSH public key' >&2
    exit 1
fi

start() {
    "$engine" run -d --name "$name" \
        --read-only \
        --cap-drop ALL \
        --cap-add CHOWN --cap-add DAC_OVERRIDE --cap-add FOWNER \
        --cap-add SETGID --cap-add SETUID --cap-add SYS_CHROOT \
        --tmpfs /run:rw,nosuid,nodev,noexec,size=32m \
        --tmpfs /tmp:rw,nosuid,nodev,noexec,size=256m \
        --mount "type=volume,src=$volume,dst=/var/lib/gilti" \
        --mount "type=bind,src=$authorized_keys,dst=/etc/gilti/authorized_keys,readonly" \
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

start
cgit_path=$("$engine" exec "$name" sh -c 'command -v gilti-cgit')
[ "$cgit_path" = /usr/local/bin/gilti-cgit ] || {
    echo "unexpected cgit binary path: $cgit_path" >&2
    exit 1
}
if "$engine" exec "$name" test -e /usr/share/webapps/cgit/cgit.cgi; then
    echo 'legacy cgit.cgi is installed' >&2
    exit 1
fi
if "$engine" exec "$name" test -e /var/cache/cgit; then
    echo 'legacy cgit disk-cache directory exists' >&2
    exit 1
fi
if "$engine" exec "$name" test -e /etc/cgitrc; then
    echo 'legacy cgit configuration file is installed' >&2
    exit 1
fi
sshd_config=$("$engine" exec "$name" /usr/sbin/sshd -T -f /etc/ssh/sshd_config \
    -C user=git,host=localhost,addr=127.0.0.1)
for expected in \
    'authenticationmethods publickey' \
    'passwordauthentication no' \
    'kbdinteractiveauthentication no' \
    'permitemptypasswords no' \
    'permitrootlogin no' \
    'disableforwarding yes' \
    'permittty no' \
    'permituserrc no' \
    'permituserenvironment no' \
    'forcecommand /usr/local/bin/gilti-ssh' \
    'authorizedkeysfile /run/gilti/ssh/authorized_keys'; do
    printf '%s\n' "$sshd_config" | grep -Fqx "$expected" || {
        echo "effective sshd configuration is missing: $expected" >&2
        exit 1
    }
done
[ "$(curl -fsS "http://127.0.0.1:$http_port/healthz")" = ok ] || {
    echo 'unexpected health response' >&2
    exit 1
}
status=$(curl -sS -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$http_port/")
[ "$status" = 403 ] || {
    echo "POST to cgit returned HTTP $status instead of 403" >&2
    exit 1
}
curl -fsS "http://127.0.0.1:$http_port/cgit.css" | grep -q 'cgit'
content_type=$(curl -fsSI "http://127.0.0.1:$http_port/cgit.css" |
    awk -F ': ' 'tolower($1) == "content-type" { gsub("\\r", "", $2); print $2 }')
[ "$content_type" = text/css ] || {
    echo "unexpected cgit.css content type: $content_type" >&2
    exit 1
}
curl -fsS "http://127.0.0.1:$http_port/cgit.js" | grep -q 'function'
content_type=$(curl -fsSI "http://127.0.0.1:$http_port/cgit.js" |
    awk -F ': ' 'tolower($1) == "content-type" { gsub("\\r", "", $2); print $2 }')
[ "$content_type" = text/javascript ] || {
    echo "unexpected cgit.js content type: $content_type" >&2
    exit 1
}
curl -fsSI "http://127.0.0.1:$http_port/healthz" >/dev/null

# shellcheck disable=SC2016 # Expanded by the shell inside the container.
httpd_uid=$("$engine" exec "$name" sh -c '
    for comm in /proc/[0-9]*/comm; do
        [ "$(cat "$comm")" = gilti ] || continue
        stat -c %u "${comm%/comm}"
        exit
    done
    exit 1
')
[ "$httpd_uid" = 10000 ] || {
    echo "gilti runs as unexpected UID $httpd_uid" >&2
    exit 1
}

host_key_mode=$("$engine" exec "$name" stat -c '%u:%a' /var/lib/gilti/ssh/ssh_host_ed25519_key)
[ "$host_key_mode" = 0:600 ] || {
    echo "unexpected SSH host-key ownership/mode: $host_key_mode" >&2
    exit 1
}
keys_mode=$("$engine" exec "$name" stat -c '%u:%g:%a' /run/gilti/ssh/authorized_keys)
[ "$keys_mode" = 0:10000:640 ] || {
    echo "unexpected authorized_keys ownership/mode: $keys_mode" >&2
    exit 1
}
if "$engine" exec --user 10000:10000 "$name" sh -c \
    'printf x >>/run/gilti/ssh/authorized_keys' 2>/dev/null; then
    echo 'Git/cgit user can modify authorized_keys' >&2
    exit 1
fi
if "$engine" exec --user 10000:10000 "$name" rm /run/gilti/ssh/authorized_keys \
    2>/dev/null; then
    echo 'Git/cgit user can remove authorized_keys' >&2
    exit 1
fi
if "$engine" exec --user 10000:10000 "$name" test -r /var/lib/gilti/ssh/ssh_host_ed25519_key; then
    echo 'Git/cgit user can read the SSH host private key' >&2
    exit 1
fi

ssh_opts="-o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=$work/known_hosts -i $work/admin -p $ssh_port"
ssh_opts_2="-o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i $work/admin-2 -p $ssh_port"
# shellcheck disable=SC2086
ssh $ssh_opts git@127.0.0.1 | grep -q 'Gilti: authenticated'
# shellcheck disable=SC2086
ssh $ssh_opts_2 git@127.0.0.1 | grep -q 'Gilti: authenticated'

if ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -i "$work/stranger" -p "$ssh_port" git@127.0.0.1 >/dev/null 2>&1; then
    echo 'an unknown key was accepted' >&2
    exit 1
fi
# shellcheck disable=SC2086
if ssh $ssh_opts git@127.0.0.1 "git-upload-pack '../../etc/passwd'" >/dev/null 2>&1; then
    echo 'gilti-ssh accepted an unsafe repository path' >&2
    exit 1
fi

# A writable HOME must not let repository traffic install process-wide hooks.
# shellcheck disable=SC2016 # Expanded by the shell inside the container.
"$engine" exec --user 10000:10000 "$name" env HOME=/var/lib/gilti/git sh -c '
    mkdir -p "$HOME/evil-hooks"
    printf "#!/bin/sh\ntouch /tmp/global-hook-ran\n" >"$HOME/evil-hooks/pre-receive"
    chmod 0700 "$HOME/evil-hooks/pre-receive"
    printf "[core]\n\thooksPath = %s/evil-hooks\n" "$HOME" >"$HOME/.gitconfig"
'

mkdir "$work/testing"
(
    cd "$work/testing"
    git init -q -b main
    git config user.name 'Gilti smoke test'
    git config user.email 'smoke@gilti.invalid'
    printf '%s\n' '# Testing' >README.md
    git add README.md
    git commit -m 'Initial commit' >/dev/null
    git remote add origin "ssh://git@127.0.0.1:$ssh_port/testing"
    GIT_SSH_COMMAND="ssh $ssh_opts" git push -u origin main >/dev/null
)
if "$engine" exec "$name" test -e /tmp/global-hook-ran; then
    echo 'gilti-ssh honored the writable global Git configuration' >&2
    exit 1
fi
repository_modes=$("$engine" exec "$name" stat -c '%a:%u:%g' \
    /var/lib/gilti/git/repositories/testing.git \
    /var/lib/gilti/git/repositories/testing.git/config)
[ "$repository_modes" = "700:10000:10000
600:10000:10000" ] || {
    echo "unexpected repository ownership/modes: $repository_modes" >&2
    exit 1
}
"$engine" exec --user 10000:10000 "$name" rm -rf \
    /var/lib/gilti/git/.gitconfig /var/lib/gilti/git/evil-hooks
GIT_SSH_COMMAND="ssh $ssh_opts_2" git clone \
    "ssh://git@127.0.0.1:$ssh_port/testing" "$work/testing-clone" >/dev/null
[ -f "$work/testing-clone/README.md" ]
(
    cd "$work/testing-clone"
    git config user.name 'Gilti smoke test 2'
    git config user.email 'smoke-2@gilti.invalid'
    printf '%s\n' 'Second key can push.' >>README.md
    git add README.md
    git commit -m 'Push with second key' >/dev/null
    GIT_SSH_COMMAND="ssh $ssh_opts_2" git push origin main >/dev/null
)

i=0
until curl -fsS "http://127.0.0.1:$http_port/" | grep -q 'testing'; do
    i=$((i + 1))
    [ "$i" -lt 30 ] || { "$engine" logs "$name" >&2; exit 1; }
    sleep 1
done
cache_url="http://127.0.0.1:$http_port/testing/"
curl -fsS -D "$work/cache-1.headers" -o /dev/null "$cache_url"
sleep 2
curl -fsS -D "$work/cache-2.headers" -o /dev/null "$cache_url"
cache_modified_1=$(awk -F ': ' 'tolower($1) == "last-modified" { gsub("\\r", "", $2); print $2 }' "$work/cache-1.headers")
cache_modified_2=$(awk -F ': ' 'tolower($1) == "last-modified" { gsub("\\r", "", $2); print $2 }' "$work/cache-2.headers")
[ -n "$cache_modified_1" ] && [ "$cache_modified_1" = "$cache_modified_2" ] || {
    echo 'CGI response was not served from the in-memory cache' >&2
    exit 1
}

# The running sshd uses its startup snapshot, not the mounted source file.
cat "$work/admin.pub" >"$authorized_keys"
# shellcheck disable=SC2086
ssh $ssh_opts_2 git@127.0.0.1 | grep -q 'Gilti: authenticated'

fingerprint=$(ssh-keyscan -p "$ssh_port" 127.0.0.1 2>/dev/null | ssh-keygen -lf - | awk '{print $2}')
remove_container
start
fingerprint_after=$(ssh-keyscan -p "$ssh_port" 127.0.0.1 2>/dev/null | ssh-keygen -lf - | awk '{print $2}')
[ "$fingerprint" = "$fingerprint_after" ] || {
    echo 'SSH host key changed after restart' >&2
    exit 1
}
curl -fsS "http://127.0.0.1:$http_port/" | grep -q 'testing'
# shellcheck disable=SC2086
ssh $ssh_opts git@127.0.0.1 | grep -q 'Gilti: authenticated'
# shellcheck disable=SC2086
if ssh $ssh_opts_2 git@127.0.0.1 >/dev/null 2>&1; then
    echo 'removed SSH key was accepted after restart' >&2
    exit 1
fi
