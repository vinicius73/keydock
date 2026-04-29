# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1
ARG ALPINE_VERSION=3.21

FROM rust:${RUST_VERSION}-alpine${ALPINE_VERSION} AS builder
WORKDIR /src

RUN apk add --no-cache \
    build-base \
    ca-certificates \
    cmake \
    perl \
    pkgconf \
    upx

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps ./apps
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo fetch --locked \
    && cargo build --frozen --locked --release -p keydock \
    && mkdir -p /src/dist \
    && cp /src/target/release/keydock /src/dist/keydock

RUN upx /src/dist/keydock -k -9 \
    && install -m 0755 /src/dist/keydock /usr/local/bin/keydock

FROM alpine:${ALPINE_VERSION} AS runtime

RUN apk add --no-cache ca-certificates

ARG UID=10001
RUN addgroup -g "${UID}" keydock \
    && adduser -D -H -u "${UID}" -G keydock -h /nonexistent -s /sbin/nologin keydock \
    && mkdir -p /var/lib/keydock/data \
    && chown -R keydock:keydock /var/lib/keydock

COPY --from=builder /usr/local/bin/keydock /usr/local/bin/keydock
COPY --chown=keydock:keydock config.example.toml /etc/keydock/keydock.toml

# listen and data_dir come from the baked-in keydock.toml config file.
# Override at runtime with KEYDOCK_HTTP_LISTEN / KEYDOCK_PATHS_DATA_DIR if needed.
ENV RUST_LOG=info

VOLUME ["/etc/keydock", "/var/lib/keydock/data"]

EXPOSE 8080

USER keydock

ENTRYPOINT ["/usr/local/bin/keydock"]
CMD ["serve", "-c", "/etc/keydock/keydock.toml"]
