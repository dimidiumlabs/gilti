# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later

ARG ALPINE_VERSION=3.22.1
ARG ALPINE_DIGEST=sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1

FROM docker.io/library/alpine:${ALPINE_VERSION}@${ALPINE_DIGEST}

ARG TARGETARCH
ARG VERSION=dev
ARG REVISION=unknown

LABEL org.opencontainers.image.title="Gilti" \
      org.opencontainers.image.description="Tiny Git server in a box" \
      org.opencontainers.image.source="https://github.com/dimidiumlabs/gilti" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"

RUN apk add --no-cache \
      bzip2=1.0.8-r6 \
      git=2.49.1-r0 \
      git-daemon=2.49.1-r0 \
      libgcc=14.2.0-r6 \
      lzip=1.25-r0 \
      openssh-keygen=10.0_p1-r10 \
      openssh-server=10.0_p1-r10 \
      su-exec=0.2-r3 \
      tini=0.19.0-r3 \
      xz=5.8.3-r0 \
      zstd=1.5.7-r0 && \
    addgroup -S -g 10000 git && \
    install -d -m 0755 -o root -g root /var/lib/gilti && \
    adduser -S -D -u 10000 -G git -h /var/lib/gilti/git -s /bin/sh git && \
    passwd -d git && \
    install -d -m 0750 -o git -g git \
      /var/lib/gilti/git /var/lib/gilti/git/repositories && \
    install -d -m 0700 -o root -g root /var/lib/gilti/ssh && \
    install -d -m 0755 -o root -g root /run/gilti && \
    install -d -m 0750 -o root -g git /run/gilti/ssh && \
    install -d -m 0755 /etc/gilti && \
    rm -rf /var/cache/apk/*

COPY --chown=root:root --chmod=0755 \
    .container/binary-${TARGETARCH}/gilti \
    .container/binary-${TARGETARCH}/gilti-ssh \
    /usr/local/bin/
COPY --chown=root:root \
    .container/binary-${TARGETARCH}/gilti.css \
    .container/binary-${TARGETARCH}/gilti.js \
    .container/binary-${TARGETARCH}/gilti.png \
    .container/binary-${TARGETARCH}/favicon.ico \
    /usr/share/gilti/
COPY --chown=root:root config/sshd_config /etc/ssh/sshd_config
COPY --chown=root:root --chmod=0755 scripts/entrypoint.sh /usr/local/bin/gilti-entrypoint
COPY --chown=root:root LICENSE README.md LICENSES/ /usr/share/doc/gilti/

RUN /usr/local/bin/gilti --check && \
    /usr/local/bin/gilti-ssh --check

EXPOSE 8080 2222
VOLUME ["/var/lib/gilti"]

ENTRYPOINT ["/sbin/tini", "-g", "--", "/usr/local/bin/gilti-entrypoint"]
CMD ["serve"]
