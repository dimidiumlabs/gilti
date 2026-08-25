# Gilti Helm chart

The chart installs one Gilti replica backed by one persistent volume. Upgrades
use `Recreate`: two pods must never write the repository filesystem
concurrently. The generated PVC carries `helm.sh/resource-policy: keep` by
default, so uninstalling the release does not delete authoritative Git data.

## SSH access

Public keys are static chart configuration:

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

cgit is anonymous and read-only and scans the complete repository directory.
Consequently every repository available over SSH is also publicly visible over
HTTP. Web cloning and snapshots are disabled in the default policy.

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
ssh:
  authorizedKeys:
    - ssh-ed25519 AAAA... operator@example
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
Repositories and the SSH host key reside on the persistent volume. Public keys
are stored in the generated ConfigMap and must be supplied on every deployment.

## Security context

The Rust HTTP gateway and its cgit children run without privileges. OpenSSH
intentionally keeps a root master so it can enter the `git` account (UID/GID
10000). The chart drops all capabilities and restores only `CHOWN`,
`DAC_OVERRIDE`, `FOWNER`, `SETGID`, `SETUID`, and `SYS_CHROOT`. The root
filesystem is read-only; state, cache, `/run`, and `/tmp` are explicit writable
mounts.
