# syntax=docker/dockerfile:1

ARG DEBIAN_CODENAME=bookworm
ARG RUST_VERSION=1.99.0

FROM rust:${RUST_VERSION}-${DEBIAN_CODENAME} AS rust-toolchain
WORKDIR /repo
ENV CI=true

RUN --mount=type=cache,id=resume-analyzer-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=resume-analyzer-cargo-git,target=/usr/local/cargo/git \
    cargo install cargo-chef --version 0.1.78 --locked
