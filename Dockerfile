FROM node:22-bookworm-slim AS ui
WORKDIR /src/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.96-bookworm AS rust
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
COPY --from=ui /src/web/dist web/dist
RUN cargo build --locked --release -p inoreader

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --create-home reader && \
    apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*
COPY --from=rust /src/target/release/inoreader /usr/local/bin/inoreader
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/inoreader"]
