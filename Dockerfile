# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: AGPL-3.0-or-later

ARG ALPINE_VERSION=3.22.1
ARG ALPINE_DIGEST=sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1

FROM docker.io/library/alpine:${ALPINE_VERSION}@${ALPINE_DIGEST}

ARG VERSION=dev
ARG REVISION=unknown

LABEL org.opencontainers.image.title="Gilti" \
      org.opencontainers.image.description="Boxed tiny Git server powered by cgit and Gitolite" \
      org.opencontainers.image.source="https://github.com/dimidiumlabs/gilti" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"

RUN apk add --no-cache \
      cgit=1.2.3-r5 \
      fcgiwrap=1.1.0-r8 \
      git=2.49.1-r0 \
      gitolite=3.6.13-r1 \
      nginx=1.28.3-r7 \
      openssh-keygen=10.0_p1-r10 \
      openssh-server=10.0_p1-r10 \
      perl=5.40.4-r0 \
      spawn-fcgi=1.6.5-r4 \
      su-exec=0.2-r3 \
      tini=0.19.0-r3 && \
    deluser git && \
    addgroup -S -g 10000 git && \
    install -d -m 0755 -o root -g root /var/lib/gilti && \
    adduser -S -D -u 10000 -G git -h /var/lib/gilti/git -s /bin/sh git && \
    passwd -d git && \
    git config --system init.defaultBranch main && \
    install -d -m 0750 -o git -g git /var/lib/gilti/git /var/cache/cgit && \
    install -d -m 0700 -o root -g root /var/lib/gilti/ssh && \
    install -d -m 0755 /run/gilti /run/gilti-bootstrap && \
    rm -rf /var/cache/apk/* /etc/nginx/http.d/default.conf

COPY --chown=root:root config/cgitrc /etc/cgitrc
COPY --chown=root:root config/nginx.conf /etc/nginx/nginx.conf
COPY --chown=root:root config/sshd_config /etc/ssh/sshd_config
COPY --chown=root:root scripts/entrypoint.sh /usr/local/bin/gilti-entrypoint
COPY --chown=root:root LICENSE README.md /usr/share/doc/gilti/

RUN chmod 0755 /usr/local/bin/gilti-entrypoint && \
    nginx -t -e /dev/stderr -c /etc/nginx/nginx.conf

EXPOSE 8080 2222
VOLUME ["/var/lib/gilti"]

ENTRYPOINT ["/sbin/tini", "-g", "--", "/usr/local/bin/gilti-entrypoint"]
CMD ["serve"]
