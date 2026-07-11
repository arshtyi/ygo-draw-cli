# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.92

FROM rust:${RUST_VERSION}-bookworm AS builder

RUN apt-get update \
    && apt-get install --yes --no-install-recommends liblzma-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates libgcc-s1 liblzma5 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --user-group --uid 10001 ygo \
    && mkdir /data \
    && chown ygo:ygo /data

COPY --from=builder /build/target/release/ygo-draw /usr/local/bin/ygo-draw

WORKDIR /data
USER ygo

ENTRYPOINT ["ygo-draw"]
