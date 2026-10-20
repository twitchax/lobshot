# syntax=docker/dockerfile:1
#
# Multi-stage build. cargo-chef caches the dependency layers (native for the server, wasm32 for the
# client), so a source-only change rebuilds in minutes instead of recompiling Bevy.

FROM rust:1-bookworm AS chef
# Prebuilt cargo-chef and trunk through cargo-binstall: building either from source costs minutes
# per cold build.
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && cargo binstall --no-confirm cargo-chef trunk
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# The pinned toolchain (with its wasm32 target) goes in BEFORE cooking, so dependencies and the
# final build share it and the cooked layers are reused; cooked on the image's default toolchain,
# the final build would start over.
COPY rust-toolchain.toml .
RUN rustup toolchain install
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# The same profile, target and package trunk builds the client with (see xtask/src/dist.rs), or
# the cooked layer is not reused.
RUN cargo chef cook --profile wasm-release --target wasm32-unknown-unknown --package lobshot-client --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin lobshot
# trunk fetches the wasm-bindgen matching Cargo.lock, and wasm-opt, by itself.
RUN cargo xtask dist

FROM debian:stable-slim AS runtime
# tini runs as PID 1: it reaps zombies and forwards `docker stop`'s SIGTERM to the binary.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin app
WORKDIR /app
COPY --from=builder /app/target/release/lobshot /usr/local/bin/lobshot
COPY --from=builder /app/dist /app/dist
# The binary's own default bind is loopback; in a container it must listen on every interface.
ENV LOBSHOT_BIND=0.0.0.0:8080 \
    LOBSHOT_DIST=/app/dist
EXPOSE 8080
USER app
ENTRYPOINT ["/usr/bin/tini", "--", "lobshot"]
