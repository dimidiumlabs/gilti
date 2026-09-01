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
    status=$?
    trap - EXIT INT TERM
    if [ "$status" -ne 0 ]; then
        echo "smoke test failed with exit code $status" >&2
        "$engine" logs "$name" >&2 || true
    fi
    remove_container
    "$engine" volume rm "$volume" >/dev/null 2>&1 || true
    rm -rf "$work"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

ssh-keygen -q -t ed25519 -N '' -f "$work/admin"
ssh-keygen -q -t ed25519 -N '' -f "$work/admin-2"
ssh-keygen -q -t ed25519 -N '' -f "$work/stranger"
cat "$work/admin.pub" "$work/admin-2.pub" >"$authorized_keys"
"$engine" volume create "$volume" >/dev/null
if "$engine" run --rm --entrypoint /bin/sh "$image" -c 'test -e /usr/local/bin/gilti-cgit' >/dev/null 2>&1; then
    echo 'removed legacy browser binary is installed' >&2
    exit 1
fi

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
    until curl -fsS "http://127.0.0.1:$http_port/-/health" >/dev/null 2>&1; do
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
[ "$(curl -fsS "http://127.0.0.1:$http_port/-/health")" = '{"status":"ok"}' ] || {
    echo 'unexpected health response' >&2
    exit 1
}
status=$(curl -sS -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$http_port/")
[ "$status" = 405 ] || {
    echo "POST to repository browser returned HTTP $status instead of 405" >&2
    exit 1
}
global_stylesheet=$work/global.css
app_stylesheet=$work/app.css
curl -fsS "http://127.0.0.1:$http_port/-/assets/global.css" -o "$global_stylesheet"
curl -fsS "http://127.0.0.1:$http_port/-/assets/app.css" -o "$app_stylesheet"
grep -q 'IBM Plex Sans' "$global_stylesheet" || {
    echo 'shared stylesheet is missing IBM Plex Sans' >&2
    exit 1
}
grep -q 'IBM Plex Math' "$global_stylesheet" || {
    echo 'shared stylesheet is missing IBM Plex Math' >&2
    exit 1
}
grep -q 'padding:4px' "$app_stylesheet" || {
    echo 'application stylesheet is missing the Gilti theme' >&2
    exit 1
}
asset_headers=$(curl -fsSI "http://127.0.0.1:$http_port/-/assets/app.css")
content_type=$(printf '%s\n' "$asset_headers" |
    awk -F ': ' 'tolower($1) == "content-type" { gsub("\\r", "", $2); print $2 }')
[ "$content_type" = 'text/css; charset=utf-8' ] || {
    echo "unexpected app.css content type: $content_type" >&2
    exit 1
}
printf '%s\n' "$asset_headers" | grep -qi '^content-security-policy:'
etag=$(printf '%s\n' "$asset_headers" |
    awk -F ': ' 'tolower($1) == "etag" { gsub("\\r", "", $2); print $2 }')
[ -n "$etag" ] || {
    echo 'app.css has no ETag' >&2
    exit 1
}
status=$(curl -sS -o /dev/null -w '%{http_code}' -H "If-None-Match: $etag" \
    "http://127.0.0.1:$http_port/-/assets/app.css")
[ "$status" = 304 ] || {
    echo "conditional app.css returned HTTP $status instead of 304" >&2
    exit 1
}
curl -fsS "http://127.0.0.1:$http_port/-/assets/app.js" | grep -q 'function'
content_type=$(curl -fsSI "http://127.0.0.1:$http_port/-/assets/app.js" |
    awk -F ': ' 'tolower($1) == "content-type" { gsub("\\r", "", $2); print $2 }')
[ "$content_type" = 'text/javascript; charset=utf-8' ] || {
    echo "unexpected app.js content type: $content_type" >&2
    exit 1
}
font_headers=$(curl -fsSI \
    "http://127.0.0.1:$http_port/-/assets/fonts/ibm-plex-mono-variable-1.0.0-roman.woff2")
printf '%s\n' "$font_headers" | grep -qi '^content-type: font/woff2'
printf '%s\n' "$font_headers" | grep -qi '^cache-control: public, max-age=31536000, immutable'
curl -fsSI "http://127.0.0.1:$http_port/favicon.ico" >/dev/null
curl -fsSI "http://127.0.0.1:$http_port/apple-touch-icon.png" >/dev/null
curl -fsSI "http://127.0.0.1:$http_port/robots.txt" >/dev/null
curl -fsSI "http://127.0.0.1:$http_port/-/assets/favicon.svg" >/dev/null
curl -fsSI "http://127.0.0.1:$http_port/-/assets/manifest.webmanifest" >/dev/null
curl -fsSI "http://127.0.0.1:$http_port/-/assets/icon-192.png" >/dev/null
curl -fsSI "http://127.0.0.1:$http_port/-/assets/icon-512.png" >/dev/null
curl -fsS "http://127.0.0.1:$http_port/-/licenses.json" | grep -q 'OFL-1.1'
curl -fsSI "http://127.0.0.1:$http_port/-/health" >/dev/null

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
    echo 'Git service user can modify authorized_keys' >&2
    exit 1
fi
if "$engine" exec --user 10000:10000 "$name" rm /run/gilti/ssh/authorized_keys \
    2>/dev/null; then
    echo 'Git service user can remove authorized_keys' >&2
    exit 1
fi
if "$engine" exec --user 10000:10000 "$name" test -r /var/lib/gilti/ssh/ssh_host_ed25519_key; then
    echo 'Git service user can read the SSH host private key' >&2
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
curl -fsS "http://127.0.0.1:$http_port/testing" | grep -q 'Initial commit'
curl -fsS "http://127.0.0.1:$http_port/testing/+/HEAD/+/tree/README%2emd" | grep -q 'Testing'
old_commit=$(git -C "$work/testing-clone" rev-parse HEAD^)
new_commit=$(git -C "$work/testing-clone" rev-parse HEAD)
curl -fsS "http://127.0.0.1:$http_port/testing/+/$new_commit" | grep -q 'Push with second key'
log_url="http://127.0.0.1:$http_port/testing/+/refs/heads/main/+/log"
curl -fsS "$log_url" -o "$work/log.html"
grep -q '<meta name="generator" content="Gilti">' "$work/log.html" || {
    echo 'log page is missing the Gilti document metadata' >&2
    exit 1
}
grep -q 'Push with second key' "$work/log.html"
log_length=$(wc -c <"$work/log.html" | tr -d ' ')
head_length=$(curl -fsSI "$log_url" |
    awk -F ': ' 'tolower($1) == "content-length" { gsub("\\r", "", $2); print $2 }')
[ "$head_length" = "$log_length" ] || {
    echo "Log HEAD length $head_length differs from GET length $log_length" >&2
    exit 1
}
feed_url="http://127.0.0.1:$http_port/testing/+/refs/heads/main/+/feed/atom"
curl -fsS "$feed_url" -o "$work/feed.xml"
grep -q '<feed xmlns=' "$work/feed.xml"
grep -q '<title>Push with second key</title>' "$work/feed.xml"
grep -q "<id>urn:sha1:$new_commit</id>" "$work/feed.xml"
feed_length=$(wc -c <"$work/feed.xml" | tr -d ' ')
head_length=$(curl -fsSI "$feed_url" |
    awk -F ': ' 'tolower($1) == "content-length" { gsub("\\r", "", $2); print $2 }')
[ "$head_length" = "$feed_length" ] || {
    echo "Atom HEAD length $head_length differs from GET length $feed_length" >&2
    exit 1
}
curl -fsS "http://127.0.0.1:$http_port/testing/+/stats" | grep -q 'Gilti smoke test'
curl -fsS "http://127.0.0.1:$http_port/testing/+/diff/$old_commit..$new_commit?format=raw" |
    grep -q '^+Second key can push\.'
curl -fsS "http://127.0.0.1:$http_port/testing/+/patch/$old_commit..$new_commit/+/README%2emd" |
    grep -q '^Subject: Push with second key'
for format in tar tar.gz tar.bz2 tar.lz tar.xz tar.zst zip; do
    curl -fsS "http://127.0.0.1:$http_port/testing/+/HEAD/+/archive?format=$format" \
        -o "$work/testing.$format"
    [ -s "$work/testing.$format" ] || {
        echo "empty $format archive" >&2
        exit 1
    }
done
summary_status=$(curl -sS -o /dev/null -w '%{http_code}:%{redirect_url}' \
    "http://127.0.0.1:$http_port/testing/+/summary")
[ "$summary_status" = "308:http://127.0.0.1:$http_port/testing" ] || {
    echo "unexpected summary redirect: $summary_status" >&2
    exit 1
}
GIT_CONFIG_GLOBAL=/dev/null git clone -q \
    "http://127.0.0.1:$http_port/testing.git" "$work/testing-http-clone"
[ -f "$work/testing-http-clone/README.md" ]
lfs_oid=2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881
lfs_response=$(printf '{"operation":"download","objects":[{"oid":"%s","size":1}]}' "$lfs_oid" |
    curl -fsS -H 'Content-Type: application/vnd.git-lfs+json' --data-binary @- \
        "http://127.0.0.1:$http_port/testing.git/info/lfs/objects/batch")
printf '%s' "$lfs_response" | grep -q '"code":404' || {
    echo 'unexpected LFS batch response' >&2
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
