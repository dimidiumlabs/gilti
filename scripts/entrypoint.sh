#!/bin/ash
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later
# shellcheck shell=dash

set -eu

state=/var/lib/gilti
run_dir=/run/gilti
cache_dir=/var/cache/cgit

git_home=$state/git
host_key_dir=$state/ssh

bootstrap_dir=/run/gilti-bootstrap
admin_key=${GILTI_ADMIN_KEY_FILE:-$bootstrap_dir/admin.pub}

log() {
    printf 'gilti: %s\n' "$*" >&2
}

run_as_git() {
    su-exec git:git env HOME="$git_home" USER=git LOGNAME=git "$@"
}

validate_public_key_file() {
    key_file=$1
    [ "$(awk 'NF && $1 !~ /^#/ { count++ } END { print count + 0 }' "$key_file")" -eq 1 ] &&
        ssh-keygen -l -f "$key_file" >/dev/null 2>&1
}

validate_bootstrap_keys() {
    [ -r "$admin_key" ] || {
        log "fresh state requires an admin public key at $admin_key"
        exit 1
    }
    for key_file in "$bootstrap_dir"/*.pub; do
        [ -e "$key_file" ] || continue
        validate_public_key_file "$key_file" || {
            log "bootstrap key $key_file is not exactly one valid SSH public key"
            exit 1
        }
    done
}

stage_additional_admin_keys() {
    install -d -m 0750 -o git -g git "$git_home/.gitolite"
    install -d -m 0750 -o git -g git "$git_home/.gitolite/keydir"
    install -d -m 0750 -o git -g git "$git_home/.gitolite/logs"
    count=0
    for key_file in "$bootstrap_dir"/*.pub; do
        [ -e "$key_file" ] || continue
        [ "$key_file" = "$admin_key" ] && continue
        count=$((count + 1))
        destination=$git_home/.gitolite/keydir/gilti-bootstrap-$count
        install -d -m 0750 -o git -g git "$destination"
        install -m 0644 -o git -g git "$key_file" "$destination/admin.pub"
    done
}

prepare_runtime() {
    [ "$(id -u)" -eq 0 ] || { log "the supervisor must start as root"; exit 1; }
    install -d -m 0755 -o root -g root "$state"
    install -d -m 0750 -o git -g git "$git_home" "$cache_dir"
    install -d -m 0700 -o root -g root "$host_key_dir"
    install -d -m 0755 "$run_dir"
    chown git:git "$cache_dir"
    rm -f "$run_dir/fcgiwrap.sock" "$run_dir/nginx.pid" "$run_dir/sshd.pid"
}

state_status() {
    complete=true
    for path in .gitolite.rc .gitolite repositories .ssh/authorized_keys; do
        [ -e "$git_home/$path" ] || complete=false
    done
    if [ "$complete" = true ]; then
        printf '%s\n' complete
        return
    fi

    partial=false
    for path in .gitolite.rc .gitolite repositories .ssh/authorized_keys projects.list; do
        [ ! -e "$git_home/$path" ] || partial=true
    done
    if [ "$partial" = true ]; then
        printf '%s\n' partial
    else
        printf '%s\n' fresh
    fi
}

initialize() {
    prepare_runtime

    case $(state_status) in
        complete)
            ;;
        partial)
            log "refusing to overwrite partial Gitolite state in $git_home"
            exit 1
            ;;
        fresh)
            validate_bootstrap_keys
            stage_additional_admin_keys
            log "initializing Gitolite"
            run_as_git gitolite setup -pk "$admin_key"
            ;;
    esac

    if [ ! -e "$git_home/projects.list" ]; then
        install -m 0640 -o git -g git /dev/null "$git_home/projects.list"
    fi

    host_key=$host_key_dir/ssh_host_ed25519_key
    if [ -L "$host_key" ]; then
        log "refusing symlinked SSH host key"
        exit 1
    fi
    if [ ! -e "$host_key" ]; then
        log "generating persistent Ed25519 SSH host key"
        ssh-keygen -q -t ed25519 -N '' -f "$host_key"
    fi
    [ -f "$host_key" ] && [ "$(stat -c %u "$host_key")" -eq 0 ] || {
        log "SSH host key must be a regular root-owned file"
        exit 1
    }
    ssh-keygen -y -f "$host_key" >/dev/null 2>&1 || {
        log "persistent SSH host key is invalid"
        exit 1
    }
    chmod 0600 "$host_key"
    ssh-keygen -y -f "$host_key" >"$host_key.pub.tmp"
    chmod 0644 "$host_key.pub.tmp"
    mv -f "$host_key.pub.tmp" "$host_key.pub"

    nginx -t -e /dev/stderr -c /etc/nginx/nginx.conf
    /usr/sbin/sshd -t -f /etc/ssh/sshd_config
}

stop_services() {
    trap - TERM INT HUP
    for pid in ${fcgi_pid:-} ${sshd_pid:-} ${nginx_pid:-}; do
        kill -TERM "$pid" 2>/dev/null || true
    done
    attempts=0
    while [ "$attempts" -lt 50 ]; do
        running=false
        for pid in ${fcgi_pid:-} ${sshd_pid:-} ${nginx_pid:-}; do
            kill -0 "$pid" 2>/dev/null && running=true
        done
        [ "$running" = true ] || break
        attempts=$((attempts + 1))
        sleep 0.1
    done
    for pid in ${fcgi_pid:-} ${sshd_pid:-} ${nginx_pid:-}; do
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
}

supervise() {
    initialize
    trap stop_services TERM INT HUP

    HOME="$git_home" spawn-fcgi -n \
        -s "$run_dir/fcgiwrap.sock" -M 0660 -U git -G git \
        -u git -g git -- /usr/bin/fcgiwrap -f &
    fcgi_pid=$!

    /usr/sbin/sshd -D -e -f /etc/ssh/sshd_config &
    sshd_pid=$!

    nginx -e /dev/stderr -c /etc/nginx/nginx.conf -g 'daemon off;' &
    nginx_pid=$!

    while :; do
        for pid in "$fcgi_pid" "$sshd_pid" "$nginx_pid"; do
            if ! kill -0 "$pid" 2>/dev/null; then
                if wait "$pid"; then status=0; else status=$?; fi
                log "a service exited; stopping the pod"
                stop_services
                exit "$status"
            fi
        done
        sleep 1
    done
}

case ${1:-serve} in
    init)
        initialize
        ;;
    serve)
        supervise
        ;;
    *)
        exec "$@"
        ;;
esac
