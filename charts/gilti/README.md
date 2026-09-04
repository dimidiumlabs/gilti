# Gilti Helm chart

The chart installs one Gilti replica backed by one persistent volume. Chart
values are rendered as `/etc/gilti/config.json`; the container passes that path
explicitly to Gilti. Upgrades use `Recreate`: two pods must never write the
repository filesystem concurrently. The generated PVC carries
`helm.sh/resource-policy: keep` by default, so uninstalling the release does
not delete authoritative Git data.

## SSH access

Public keys are static chart configuration. Set `ssh.enabled: false` to disable
the SSH daemon and its Service entirely:

```yaml
ssh:
    authorizedKeys:
        - ssh-ed25519 AAAA... operator@example
        - ssh-ed25519 AAAA... automation@example
```

Every configured key has the same permissions: it may fetch, push, and create
any repository. There are no users, per-repository ACLs, or live key updates.
Gilti snapshots the configured keys when the pod starts, so a key change takes
effect after the Deployment rolls out the updated ConfigMap.

Shells, forwarding, tunnels, and arbitrary SSH commands are disabled. A push to
a missing repository initializes it as a bare repository:

```console
git remote add origin ssh://git@git.example.test/example
git push -u origin main
```

## Repository visibility

Repository browsing, smart HTTP cloning, archives, and LFS downloads are
anonymous and read-only. Consequently every repository available over SSH is
also publicly visible over HTTP. HTTP pushes and LFS uploads remain disabled by
default; setting `http.write: true` enables them without authentication and is
appropriate only behind a trusted access-control proxy.

## Repository archives

`archive.formats` selects the archive links and download formats exposed by
Gilti. Supported values are `tar`, `tar.gz`, `tar.bz2`, `tar.xz`, `tar.zst`,
and `zip`; an empty list disables archive downloads. Other archive codec and
buffer defaults come from the typed application configuration.

All typed application settings are available through the advanced `config` map
using the snake_case keys from [`../../config/gilti.toml`](../../config/gilti.toml).
For example, `config.lfs.max_object_bytes: 2GiB` changes the LFS object limit.
The chart-level `web`, `http`, `ssh`, and `archive.formats` values override the
corresponding advanced keys. Unknown application fields are rejected when the
pod validates its generated JSON configuration.

## Networking

The HTTP and SSH Services are separate so an installation can expose them using
different Kubernetes mechanisms. `http.hostnames` restricts accepted HTTP
`Host` authorities; an empty list disables host filtering. `httpRoute`
creates a Gateway API `HTTPRoute`. `sshRoute` creates a `TCPRoute`; its
`apiVersion` defaults to Gateway API v1 for
Gateway API 1.6 and Envoy Gateway 1.9. Set it to
`gateway.networking.k8s.io/v1alpha2` for an older cluster that still serves only
that version. Enable the route only when the selected Gateway has a TCP
listener.

Example for the first Dimidium Labs installation:

```yaml
ssh:
    authorizedKeys:
        - ssh-ed25519 AAAA... operator@example
web:
    clonePrefix: ssh://git@git.dimidiumlabs.io/
http:
    hostnames: [git.dimidiumlabs.io]
httpRoute:
    enabled: true
    hostnames: [git.dimidiumlabs.io]
    parentRefs:
        - name: public
          namespace: network
          sectionName: vcs-https
sshRoute:
    enabled: true
    parentRefs:
        - name: public
          namespace: network
          sectionName: vcs-ssh
```

TLS terminates at the shared Gateway; the chart does not create certificates.
Repositories and the SSH host key reside on the persistent volume mounted at
`/var/lib/gilti`. Public keys are stored in the generated ConfigMap and must be
supplied on every deployment. The chart intentionally emits only its high-level
values; omitted daemon limits use the validated defaults in the application.

## Security context

The Rust HTTP gateway and its native views run without privileges. OpenSSH
intentionally keeps a root master so it can enter the `git` account (UID/GID
10000). The chart drops all capabilities and restores only `CHOWN`,
`DAC_OVERRIDE`, `FOWNER`, `SETGID`, `SETUID`, and `SYS_CHROOT`. The root
filesystem is read-only; state, `/run`, and `/tmp` are explicit writable mounts.
