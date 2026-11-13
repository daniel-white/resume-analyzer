# syntax=docker/dockerfile:1

# Build images through Dagger, which runs docker buildx bake via Dagger: $*
# The version variables come from the flake devShell (RUST_VERSION=$(rustc --version | cut -d ' ' -f 2)).
# They are declared here as build args so the host-side job passes them through.
ARG RUST_VERSION=1.100.0
ARG DEBIAN_CODENAME=bookworm

FROM rust:${RUST_VERSION}-${DEBIAN_CODENAME} AS chef
WORKDIR /src
# Pin the actual toolchain from rust-toolchain.toml so the build uses the
# same compiler even if a stale RUST_VERSION is exported in the shell.
COPY rust-toolchain.toml ./
# Mirrors images/rust/toolchain.Dockerfile: install Cargo Chef in the toolchain
# layer so the planner stage (line 16) can run `cargo chef prepare`.
RUN --mount=type=cache,id=resume-analyzer-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=resume-analyzer-cargo-git,target=/usr/local/cargo/git \
    cargo install cargo-chef --version 0.1.78 --locked

FROM chef AS planner
COPY tmp/cargo-manifests.tar ./tmp/
RUN tar -xf tmp/cargo-manifests.tar -C . \
    && rm -rf ./tmp
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=planner /src/recipe.json recipe.json
RUN --mount=type=cache,id=resume-analyzer-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=resume-analyzer-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --release --locked --recipe-path recipe.json -p app
COPY Cargo.toml Cargo.lock ./
COPY crates crates
# sqlx::migrate! embeds these at compile time. A migration change must rebuild
# the application, but should not invalidate the dependency cook layer.
COPY migrations migrations
RUN --mount=type=cache,id=resume-analyzer-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=resume-analyzer-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release --locked -p app

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/resume-analyzer /usr/local/bin/resume-analyzer
EXPOSE 8080
ENV BIND=0.0.0.0:8080
CMD ["resume-analyzer"]