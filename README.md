# Gilti — a tiny Git server in a box

Gilti is a tiny web UI for Git that can function as either a read-only showcase
or a Git SSH server by integrating with the system's `sshd`. It is designed for
open-source projects and small teams that want to break free from major hosting
platforms but aren't ready to host complex services like Forgejo.

> Gilti is a young project. Mirror your repositories and create backups.

## Security boundary

- Git fetch is available anonymously over smart HTTP; authenticated fetch and
  push use SSH public-key authentication through `gilti shell`.
- Every configured key has read/write access to every repository and may create
  a repository by pushing to its name for the first time.
- Repository browsing, archives, LFS downloads, and smart HTTP fetches are
  anonymous and read-only; every repository is publicly visible.
- Password authentication, shells, forwarding, and tunnels are disabled; content
  filters are not supported. Optional unauthenticated HTTP writes must be
  enabled explicitly.
- Gilti is a single-replica service backed by one POSIX persistent volume. It is
  not an HA system.

## Container

The image contains a default TOML configuration. SSH-enabled starts also require
an `authorized_keys` file:

```console
docker pull ghcr.io/dimidiumlabs/gilti:nightly
ssh-keygen -q -t ed25519 -N '' -f ./admin
cp ./admin.pub ./authorized_keys
cat >gilti.toml <<'EOF'
[instance]
root_title = "My Git server"

[access]
ssh = true
EOF

docker run --rm \
  --read-only --cap-drop ALL \
  --cap-add CHOWN --cap-add DAC_OVERRIDE --cap-add FOWNER \
  --cap-add SETGID --cap-add SETUID --cap-add SYS_CHROOT \
  --tmpfs /run:rw,nosuid,nodev,noexec,size=32m \
  --tmpfs /tmp:rw,nosuid,nodev,noexec,size=256m \
  -p 8080:8080 -p 2222:2222 \
  -v gilti-state:/var/lib/gilti \
  -v "$PWD/gilti.toml:/etc/gilti/config.toml:ro" \
  -v "$PWD/authorized_keys:/etc/gilti/authorized_keys:ro" \
  ghcr.io/dimidiumlabs/gilti:nightly
```

Gilti requires the configuration path explicitly:

```console
gilti --config /etc/gilti/config.toml
gilti --config /etc/gilti/config.toml --check
gilti --config /etc/gilti/config.toml shell
```

The filename extension selects JSON (`.json`), TOML (`.toml`), or YAML
(`.yaml`/`.yml`). Each file is a Serde representation of the typed `Config`
structure with `server`, `instance`, `git_storage`, `git`, `lfs`, `browser`,
`access`, and `archive` sections.
Unknown fields are rejected and omitted fields receive typed defaults. See the
complete [`config/gilti.toml`](config/gilti.toml) example.

Durations use values such as `250ms`, `10s`, or `2m`. Byte sizes accept explicit
units such as `32KiB` and `10MiB`. Git storage paths must be absolute and
normalized, and `git_storage.repositories` must be below `git_storage.home`.
When `repositories` is omitted, it defaults to the `repositories` directory
below the configured Git home. The container image and Helm chart mount their
persistent state at `/var/lib/gilti`; custom container storage paths must be
placed on a writable mount with suitable ownership.

`server` owns transport limits and trusted-proxy networks, `git` owns executable
paths and smart-HTTP policy, `lfs` owns object and request limits, and `browser`
owns pagination and presentation limits. `archive.formats` selects the download
formats; supported values are `tar`, `tar.gz`, `tar.bz2`, `tar.xz`, `tar.zst`,
and `zip`. TAR compression is streamed by native Rust codecs rather than
external compressor processes; its buffer and codec levels are configured in
the same `archive` section.

Gilti snapshots the authorized keys file at process startup; changing it takes
effect after a restart. Repositories and the persistent SSH host key live on the
state volume.

## Helm

Configure the allowed public keys in values:

```yaml
ssh:
    authorizedKeys:
        - ssh-ed25519 AAAA... operator@example
```

```console
helm upgrade --install gilti ./charts/gilti \
  --namespace gilti --create-namespace \
  --values values.yaml \
  --set web.clonePrefix='ssh://git@git.dimidiumlabs.io/'
```

See [`charts/gilti/README.md`](charts/gilti/README.md) for persistence, routing,
and public-repository configuration.

## Development

Provision tools and run static checks:

```console
mise bootstrap
cargo fmt -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
shellcheck scripts/*.sh tests/*.sh
mise run chart -- --chart charts/gilti --lint-only
```

For layout work, run the source watcher after starting `gilti-playground.service`:

```console
mise run dev
```

It rebuilds the `gilti` binary and restarts the user service after changes to
Gilti sources or manifests. Set `GILTI_DEV_SERVICE` to use a different systemd
user unit. Browser refresh remains manual.

The CI matrix builds architecture-specific native Linux artifacts before
assembling the image; the Dockerfile does not compile source code. Linux
packages are intentionally not produced. Gilti's release artifacts are a
multi-platform OCI image and an OCI Helm chart.

## Contributing

We welcome your contributions, including code, bug reports, ideas, and success
stories.

If you are making a contribution for the first time or from a new email, please
add yourself to the `.mailmap`.

### Signoff

To include your code, we ask that you read and agree to the [CLA](./CLA.md). To
sign, add a `CLA-Version: 1.0` and a `Signed-off-by` trailer to every commit
(`git commit -s --trailer "CLA-Version: 1.0"`). Each commit in a pull request
must carry a valid `Signed-off-by` line matching the commit author. Please use
your real name. We cannot include code from anonymous contributors.

AI agents MUST NOT add Signed-off-by tags. Only humans can legally certify the
Contributor License Agreement.

### AI policy

You may use AI agents when writing code and documentation. AI is not allowed for
media including images, videos, fonts at all. You must fully read, understand,
and cleanup any code generated by the agent. We ask that you disclose the
agent's use and indicate the tool, model, and extent of contribution.

Contributions should include an Assisted-by tag in the following format:
`Assisted-by: AGENT_NAME:MODEL_VERSION [TOOL1] [TOOL2]`, for example:
`Assisted-by: Claude:claude-4.6-opus coccinelle sparse`

Remember, AI agents should make software better, not worse.

## Licensing

Gilti source code is licensed under AGPL-3.0-or-later. Documentation is licensed
under CC-BY-4.0. Components installed into the image retain their respective
upstream licenses.
