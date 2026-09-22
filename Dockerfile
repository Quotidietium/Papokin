FROM alpine:3.24

ARG TARGETARCH
ARG PAPOKIN_TAG=nightly

RUN apk add --no-cache curl ca-certificates && \
    case "${TARGETARCH}" in \
        "amd64") BIN_ARCH="X64" ;; \
        "arm64") BIN_ARCH="ARM64" ;; \
        *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac && \
    curl -fsSL "https://github.com/Quotidietium/Papokin/releases/download/${PAPOKIN_TAG}/papokin-${BIN_ARCH}-Linux-musl" \
        -o /usr/local/bin/papokin && \
    chmod +x /usr/local/bin/papokin && \
    apk del curl

RUN addgroup -g 2613 papokin && \
    adduser -u 2613 -G papokin -D -h /papokin papokin && \
    chown -R papokin:papokin /papokin

WORKDIR /papokin
USER papokin:papokin

ENV RUST_BACKTRACE=1
EXPOSE 25565

ENTRYPOINT [ "papokin" ]

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
    CMD nc -z 127.0.0.1 25565 || exit 1