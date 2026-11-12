# syntax=docker/dockerfile:1.7

ARG NAME=resume-analyzer
ARG FEATURES=service

# --- Planner: prepare recipe ---
FROM rust-toolchain AS planner
ARG NAME
RUN cargo chef prepare --recipe-path recipe.json --bin ${NAME}

# --- Builder: build the app ---
FROM rust-toolchain AS builder
ARG NAME
ARG FEATURES
COPY recipe.json ./recipe.json
# Install dependencies, run cargo chef cook + cargo build
RUN --mount=type=cache,id=resume-analyzer-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=resume-analyzer-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=resume-analyzer-cargo-target-api,target=/repo/target \
    cargo chef cook --release --locked --recipe-path recipe.json -p app
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY migrations migrations
# Build the app
RUN cargo build --release --locked -p app
COPY --from=planner /repo/recipe.json recipe.json
RUN mkdir -p dist/ && cp target/release/${NAME} dist/

# --- Runtime: final image ---
FROM rust-runtime AS runtime
ARG NAME
COPY --from=builder /repo/dist/${NAME} /usr/local/bin/${NAME}
EXPOSE 8080
ENV BIND=0.0.0.0:8080
CMD ["${NAME}"]