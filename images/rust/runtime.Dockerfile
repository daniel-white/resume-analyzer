# syntax=docker/dockerfile:1.7

ARG DEBIAN_VERSION=12

FROM debian:${DEBIAN_VERSION}-slim AS rust-runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libpq5 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin