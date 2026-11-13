# Node.js base image targets. The flake devShell exports NODEJS_VERSION
# (from the nixpkgs nodejs_24 package via `node --version | cut -c 2-`).
# This file declares the variable with no default so an unset value fails
# the build.

variable "NODEJS_VERSION" {}

variable "NODEJS_COREPACK_VERSION" {
  default = "0.35.0"
}

target "_nodejs-common" {
  inherits = ["_common"]
  args = {
    NODEJS_VERSION = "${NODEJS_VERSION}"
    NODEJS_COREPACK_VERSION = "${NODEJS_COREPACK_VERSION}"
  }
}

target "_nodejs-toolchain" {
  inherits = ["_nodejs-common"]
  dockerfile = "images/nodejs/toolchain.Dockerfile"
}