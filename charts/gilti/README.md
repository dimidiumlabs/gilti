# Gilti Helm chart

The chart installs one Gilti replica backed by one persistent volume. Upgrades
use `Recreate`: two pods must never write the Gitolite filesystem concurrently.
The generated PVC carries `helm.sh/resource-policy: keep` by default, so
uninstalling the release does not delete authoritative Git data.

## Bootstrap

A fresh volume requires an existing Secret containing the administrator's SSH
public key as `admin.pub`:

```console
kubectl create secret generic gilti-bootstrap --from-file=admin.pub
helm upgrade --install gilti . \
  --set bootstrap.existingSecret=gilti-bootstrap
```

Additional `*.pub` entries in the same Secret are committed to `gitolite-admin`
as additional keys for the same `admin` identity. Bootstrap keys are ignored
after successful initialization and the Secret may then be removed from values.
A partially initialized volume is never overwritten automatically.

## Publishing repositories

cgit does not implement Gitolite authorization. Gilti therefore reads Gitolite's
generated `projects.list`; a repository becomes public only when the `gitweb`
pseudo-user can read it:

```text
repo example
    RW+ = alice
    R   = gitweb
```

After pushing this configuration to `gitolite-admin`, `example` appears in cgit.
Repositories not present in `projects.list` are unavailable even through a
direct cgit URL. Web cloning and snapshots are disabled in the default policy.

## Networking

The HTTP and SSH Services are separate so an installation can expose them using
different Kubernetes mechanisms. `httpRoute` creates a Gateway API `HTTPRoute`.
`sshRoute` creates a `TCPRoute`; its `apiVersion` defaults to Gateway API v1 for
Gateway API 1.6 and Envoy Gateway 1.9. Set it to
`gateway.networking.k8s.io/v1alpha2` for an older cluster that still serves only
that version. Enable the route only when the selected Gateway has a TCP
listener.

Example for the first Dimidium Labs installation:

```yaml
cgit:
  clonePrefix: ssh://git@vcs.dimidiumlabs.io/
httpRoute:
  enabled: true
  hostnames: [vcs.dimidiumlabs.io]
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
SSH host keys, Gitolite configuration, repositories, generated authorization,
and audit logs reside on the persistent volume. Back up and restore that volume
as a unit.

## Security context

The Rust HTTP gateway and its cgit children run without privileges. OpenSSH
intentionally keeps a root master so it can enter the `git` account (UID/GID
10000). The chart drops
all capabilities and restores only `CHOWN`, `DAC_OVERRIDE`, `FOWNER`, `SETGID`,
`SETUID`, and `SYS_CHROOT`. The root filesystem is read-only; state, cache,
`/run`, and `/tmp` are explicit writable mounts.
