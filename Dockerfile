# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS ui
WORKDIR /src/web
COPY web/package*.json ./
RUN --mount=type=cache,id=inoreader-npm,target=/root/.npm,sharing=locked \
    npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.96-bookworm AS rust
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
COPY --from=ui /src/web/dist web/dist
RUN --mount=type=cache,id=inoreader-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=inoreader-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=inoreader-cargo-target,target=/src/target,sharing=locked \
    --mount=type=cache,id=inoreader-rustup,target=/usr/local/rustup,sharing=locked \
    cargo build --locked --release -p inoreader && \
    cp /src/target/release/inoreader /tmp/inoreader

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --create-home reader && \
    apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*
COPY --from=rust /tmp/inoreader /usr/local/bin/inoreader
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/inoreader"]
