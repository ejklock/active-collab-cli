FROM rust:1-slim AS builder

# cargo-watch is needed by the dev service for hot-reload; installed in the
# builder stage so the runtime stage stays minimal.
RUN rustup component add clippy rustfmt && \
    cargo install cargo-watch --locked

WORKDIR /app

COPY Cargo.toml ./
RUN mkdir src && printf 'fn main() {}\n' > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/ac target/release/.fingerprint/ac-*

COPY src ./src
# locales/ must be present at compile time so include_str! embeds the pt-BR catalog
COPY locales ./locales
RUN cargo build --release

FROM debian:trixie-slim AS runtime

# ca-certificates has no stable pinnable version across debian releases;
# the package name itself is the version contract for this system package.
# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ac /usr/local/bin/ac

ENTRYPOINT ["/usr/local/bin/ac"]
