# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps ./apps
COPY crates ./crates

RUN cargo build --release -p keydock

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/keydock /usr/local/bin/keydock
COPY config.example.toml /etc/keydock/keydock.toml

EXPOSE 8080

ENTRYPOINT ["keydock"]
CMD ["serve", "-c", "/etc/keydock/keydock.toml"]
