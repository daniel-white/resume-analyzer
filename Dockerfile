# syntax=docker/dockerfile:1

# Match the flake's Rust toolchain; pin Chef rather than installing it on
# every cold build. Keep compiled dependencies in an exportable image layer.
FROM lukemathwalker/cargo-chef:0.1.78-rust-1.99.1-bookworm AS chef
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
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