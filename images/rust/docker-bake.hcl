# Rust base image targets. The flake devShell exports RUST_VERSION (from
# rust-toolchain.toml via `rustc --version | cut -d ' ' -f 2`). This file
# declares the variable with no default so an unset value fails the build.

variable "RUST_VERSION" {}

target "_rust-common" {
  inherits = ["_common"]
  args = {
    RUST_VERSION = "${RUST_VERSION}"
  }
}

target "_rust-toolchain" {
  inherits = ["_rust-common"]
  dockerfile = "images/rust/toolchain.Dockerfile"
}

target "_rust-runtime" {
  inherits = ["_rust-common"]
  dockerfile = "images/rust/runtime.Dockerfile"
}

target "_rust-bin" {
  inherits = ["_rust-common"]
  dockerfile = "images/rust/bin.Dockerfile"
}

target "_rust-service" {
  inherits = ["_rust-common"]
  dockerfile = "images/rust/service.Dockerfile"
}
