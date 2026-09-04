#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later
# shellcheck shell=dash

set -eu
umask 077

state=/var/lib/gilti
run_dir=/run/gilti
ssh_run_dir=$run_dir/ssh
config_path_file=$run_dir/config-path
host_key_dir=$state/ssh
authorized_keys_source=/etc/gilti/authorized_keys
authorized_keys=$ssh_run_dir/authorized_keys

log() {
    printf 'gilti: %s\n' "$*" >&2
}

resolve_config() {
    [ "$#" -eq 2 ] && [ "$1" = --config ] || {
        log 'serve requires --config <PATH>'
        exit 1
    }
    config_source=$(readlink -f -- "$2") || {
        log "cannot resolve configuration path $2"
        exit 1
    }
    [ -f "$config_source" ] && [ -r "$config_source" ] || {
        log "configuration is not a readable file: $config_source"
        exit 1
    }
}

prepare_runtime() {
    [ "$(id -u)" -eq 0 ] || { log "the supervisor must start as root"; exit 1; }
    [ ! -L "$state" ] || { log "refusing symlinked state path $state"; exit 1; }
    # The sticky root-owned state directory lets git create configured storage
    # while preventing it from replacing the root-owned SSH host-key directory.
    install -d -m 1770 -o root -g git "$state"
    install -d -m 0755 -o root -g root "$run_dir"

    extension=${config_source##*.}
    extension=$(printf '%s' "$extension" | tr '[:upper:]' '[:lower:]')
    case $extension in
        json|toml|yaml|yml) ;;
        *) log "configuration must have a .json, .toml, .yaml, or .yml extension"; exit 1 ;;
    esac
    config=$run_dir/config.$extension
    config_tmp=$config.tmp.$$
    cp -- "$config_source" "$config_tmp"
    chown root:git "$config_tmp"
    chmod 0640 "$config_tmp"
    mv -f "$config_tmp" "$config"

    config_path_tmp=$config_path_file.tmp.$$
    printf '%s\n' "$config" >"$config_path_tmp"
    chown root:git "$config_path_tmp"
    chmod 0640 "$config_path_tmp"
    mv -f "$config_path_tmp" "$config_path_file"
}

prepare_authorized_keys() {
    [ -f "$authorized_keys_source" ] && [ -r "$authorized_keys_source" ] || {
        log "SSH public keys are required at $authorized_keys_source"
        exit 1
    }

    output=$authorized_keys.tmp.$$
    candidate=$authorized_keys.key.$$
    : >"$output"
    count=0
    while IFS= read -r key || [ -n "$key" ]; do
        case $key in
            ''|'#'*) continue ;;
        esac
        case $key in
            ssh-*|ecdsa-*|sk-*) ;;
            *)
                rm -f "$output" "$candidate"
                log "$authorized_keys_source contains an invalid SSH public key"
                exit 1
                ;;
        esac
        printf '%s\n' "$key" >"$candidate"
        if ! ssh-keygen -l -f "$candidate" >/dev/null 2>&1; then
            rm -f "$output" "$candidate"
            log "$authorized_keys_source contains an invalid SSH public key"
            exit 1
        fi
        printf 'restrict %s\n' "$key" >>"$output"
        count=$((count + 1))
    done <"$authorized_keys_source"
    rm -f "$candidate"

    if [ "$count" -eq 0 ]; then
        rm -f "$output"
        log "$authorized_keys_source contains no SSH public keys"
        exit 1
    fi
    chown root:git "$output"
    chmod 0640 "$output"
    mv -f "$output" "$authorized_keys"
}

prepare_host_key() {
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
}

prepare() {
    prepare_runtime
    gosu git:git /usr/local/bin/gilti --config "$config" --check

    if gosu git:git /usr/local/bin/gilti --config "$config" shell --is-enabled; then
        ssh_enabled=1
    else
        status=$?
        [ "$status" -eq 3 ] || exit "$status"
        ssh_enabled=0
    fi

    if [ "$ssh_enabled" -eq 1 ]; then
        install -d -m 0700 -o root -g root "$host_key_dir"
        install -d -m 0755 -o root -g root /run/sshd
        install -d -m 0750 -o root -g git "$ssh_run_dir"
        rm -f "$ssh_run_dir/sshd.pid" "$authorized_keys" "$authorized_keys".*
        prepare_authorized_keys
        prepare_host_key
        gosu git:git /usr/local/bin/gilti --config "$config" shell --check
        /usr/sbin/sshd -t -f /etc/ssh/sshd_config
    fi
}

stop_services() {
    trap - TERM INT HUP
    for pid in ${httpd_pid:-} ${sshd_pid:-}; do
        kill -TERM "$pid" 2>/dev/null || true
    done
    attempts=0
    while [ "$attempts" -lt 50 ]; do
        running=false
        for pid in ${httpd_pid:-} ${sshd_pid:-}; do
            kill -0 "$pid" 2>/dev/null && running=true
        done
        [ "$running" = true ] || break
        attempts=$((attempts + 1))
        sleep 0.1
    done
    for pid in ${httpd_pid:-} ${sshd_pid:-}; do
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
}

supervise() {
    prepare
    trap stop_services TERM INT HUP

    if [ "$ssh_enabled" -eq 1 ]; then
        /usr/sbin/sshd -D -e -f /etc/ssh/sshd_config &
        sshd_pid=$!
    fi

    gosu git:git /usr/local/bin/gilti --config "$config" &
    httpd_pid=$!

    while :; do
        for pid in ${sshd_pid:-} "$httpd_pid"; do
            if ! kill -0 "$pid" 2>/dev/null; then
                if wait "$pid"; then status=1; else status=$?; fi
                log "a service exited; stopping the pod"
                stop_services
                exit "$status"
            fi
        done
        sleep 1
    done
}

forced_shell() {
    [ "$(id -u)" -eq 10000 ] || { log "the forced shell must run as git"; exit 1; }
    [ -r "$config_path_file" ] || { log "configuration path is unavailable"; exit 1; }
    IFS= read -r config <"$config_path_file"
    [ -n "$config" ] || { log "configuration path is empty"; exit 1; }
    exec /usr/local/bin/gilti --config "$config" shell
}

case ${1:-} in
    serve)
        shift
        resolve_config "$@"
        supervise
        ;;
    shell)
        [ "$#" -eq 1 ] || { log "invalid forced-shell invocation"; exit 1; }
        forced_shell
        ;;
    '')
        log 'serve requires --config <PATH>'
        exit 1
        ;;
    *)
        exec "$@"
        ;;
esac
