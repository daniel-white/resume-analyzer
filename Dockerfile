# syntax=docker/dockerfile:1.7.0@sha256:dbbd5e059e8a07ff7ea6233b213b36aa516b4c53c645f1817a4dd18b83cbea56

# Build images through Dagger, which runs docker buildx bake via Dagger: $*
# Toolchain versions come from rust-toolchain.toml and are passed by the bake
# file. There is no default on purpose: an unset value must fail the build
# instead of silently selecting a floating tag.
ARG RUST_VERSION

FROM rust:${RUST_VERSION}-bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea AS chef
WORKDIR /src
# Pin the actual toolchain from rust-toolchain.toml so the build uses the
# same compiler even if a stale RUST_VERSION is exported in the shell.
COPY rust-toolchain.toml ./
# Keep compiler output deterministic. This also avoids writing incremental
# state into a layer that would otherwise be part of the exported cache.
ENV CARGO_INCREMENTAL=0
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
# Keep the cook layer independent of the commit timestamp. SOURCE_DATE_EPOCH
# is only consumed by the source build below.
ARG SOURCE_DATE_EPOCH=0
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}
COPY Cargo.toml Cargo.lock ./
COPY crates crates
# sqlx::migrate! embeds these at compile time. A migration change must rebuild
# the application, but should not invalidate the dependency cook layer.
COPY migrations migrations
RUN --mount=type=cache,id=resume-analyzer-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=resume-analyzer-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo build --release --locked -p app --bin resume-analyzer

FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
ARG VERSION=dev
ARG GIT_SHA=unknown
ARG BUILD_DATE=1970-01-01T00:00:00Z
LABEL org.opencontainers.image.source="https://github.com/daniel-white/resume-analyzer" \
      org.opencontainers.image.revision="${GIT_SHA}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.licenses="MIT"
# The rust image already carries a current CA bundle. Copying it avoids a
# network-dependent apt layer in the final image.
COPY --from=build /etc/ssl /etc/ssl
COPY --from=build /src/target/release/resume-analyzer /usr/local/bin/resume-analyzer
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
EXPOSE 8080
ENV BIND=0.0.0.0:8080
USER 10001:10001
CMD ["resume-analyzer"]
