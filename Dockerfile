FROM rust:1-slim AS builder

WORKDIR /app

# rust-toolchain.toml is the single source of the Rust version; rustup installs
# that toolchain and its components into this layer.
COPY rust-toolchain.toml ./
RUN rustup toolchain install

# cargo-watch is needed by the dev service for hot-reload; installed in the
# builder stage so the runtime stage stays minimal.
RUN cargo install cargo-watch --locked

COPY Cargo.toml ./
RUN mkdir src && printf 'fn main() {}\n' > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/ac target/release/.fingerprint/ac-*

COPY src ./src
# locales/ must be present at compile time so include_str! embeds the pt-BR catalog
COPY locales ./locales
# .claude/skills/ must be present at compile time so include_str! embeds SKILL.md (ADR 0057)
COPY .claude/skills ./.claude/skills
RUN cargo build --release

FROM builder AS dev

# Coverage tooling lives only in this stage so the release image does not carry it;
# the component goes on the toolchain pinned by rust-toolchain.toml.
RUN rustup component add llvm-tools-preview && \
    cargo install cargo-llvm-cov --locked

FROM debian:trixie-slim AS runtime

# ca-certificates has no stable pinnable version across debian releases;
# the package name itself is the version contract for this system package.
# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ac /usr/local/bin/ac

ENTRYPOINT ["/usr/local/bin/ac"]
