# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later

ARG DEBIAN_VERSION=13-slim
ARG DEBIAN_DIGEST=sha256:d7e12182ce18b85b93007c1dedf31f2d29e01ccf3182cc4017c709b6259bc132

FROM docker.io/library/debian:${DEBIAN_VERSION}@${DEBIAN_DIGEST}

ARG TARGETARCH
ARG VERSION=dev
ARG REVISION=unknown

LABEL org.opencontainers.image.title="Gilti" \
      org.opencontainers.image.description="Tiny Git server in a box" \
      org.opencontainers.image.source="https://git.dimidiumlabs.io/gilti" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      bzip2=1.0.8-6 \
      git=1:2.47.3-0+deb13u1 \
      gosu=1.17-3+b4 \
      libgcc-s1=14.2.0-19 \
      lzip=1.25-3 \
      openssh-server=1:10.0p1-7+deb13u4 \
      tini=0.19.0-3+b7 \
      xz-utils=5.8.1-1+deb13u1 \
      zstd=1.5.7+dfsg-1 && \
    groupadd --system --gid 10000 git && \
    useradd --system --uid 10000 --gid git --home-dir /var/lib/gilti/git --shell /bin/sh git && \
    passwd -d git && \
    install -d -m 0755 -o root -g root /var/lib/gilti && \
    install -d -m 0750 -o git -g git \
      /var/lib/gilti/git /var/lib/gilti/git/repositories && \
    install -d -m 0700 -o root -g root /var/lib/gilti/ssh && \
    install -d -m 0755 -o root -g root /run/gilti && \
    install -d -m 0750 -o root -g git /run/gilti/ssh && \
    install -d -m 0755 /etc/gilti && \
    rm -rf /var/lib/apt/lists/*

COPY --chown=root:root --chmod=0755 \
    .container/binary-${TARGETARCH}/gilti \
    .container/binary-${TARGETARCH}/gilti-ssh \
    /usr/local/bin/
COPY --chown=root:root config/sshd_config /etc/ssh/sshd_config
COPY --chown=root:root --chmod=0755 scripts/entrypoint.sh /usr/local/bin/gilti-entrypoint
COPY --chown=root:root LICENSE README.md LICENSES/ /usr/share/doc/gilti/

RUN /usr/local/bin/gilti --check && \
    /usr/local/bin/gilti-ssh --check

EXPOSE 8080 2222
VOLUME ["/var/lib/gilti"]

ENTRYPOINT ["/usr/bin/tini", "-g", "--", "/usr/local/bin/gilti-entrypoint"]
CMD ["serve"]
