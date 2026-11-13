# syntax=docker/dockerfile:1

ARG NODEJS_VERSION=24
ARG NODEJS_COREPACK_VERSION=0.35.0
ARG DEBIAN_CODENAME=bookworm

FROM node:${NODEJS_VERSION}-${DEBIAN_CODENAME}-slim AS nodejs-toolchain
WORKDIR /repo
ENV CI=true

RUN npm install -g corepack@${NODEJS_COREPACK_VERSION}
RUN corepack enable
COPY package.json ./
RUN corepack install

COPY tmp/pnpm-manifests.tar ./tmp/
RUN tar -xf tmp/pnpm-manifests.tar -C . \
    && rm -rf ./tmp