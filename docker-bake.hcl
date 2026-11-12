# Versions are still pinned in the Dockerfiles. The follow-up in
# docs/plan.md moves RUST_VERSION and NODEJS_VERSION here, with no
# default, fed by the flake devShell. An unset value must fail the build
# instead of pulling a floating tag.

variable "REGISTRY" {
  default = "resume-analyzer"
}

variable "TAG" {
  default = "dev"
}

# Pinned by the flake devShell (`export RUST_VERSION=$(rustc --version | cut -d ' ' -f 2)`,
# `export NODEJS_VERSION=$(node --version | cut -c 2-)`). No default on purpose:
# an unset value must fail the build rather than silently fetching a floating tag.
variable "RUST_VERSION" {}
variable "NODEJS_VERSION" {}

target "_common" {
  args = {
    RUST_VERSION = "${RUST_VERSION}"
    NODEJS_VERSION = "${NODEJS_VERSION}"
  }
}

group "default" {
  targets = ["api"]
}

group "all" {
  targets = ["api", "web", "providers", "model", "model-llama"]
}

target "api" {
  inherits = ["_common"]
  context    = "."
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/api:${TAG}"]
}

target "model" {
  context    = "."
  dockerfile = "Dockerfile.model"
  tags       = ["${REGISTRY}/model:${TAG}"]
}

# The candidate-profile GGUF. Baked once, then a llama-cpp initContainer
# copies it into the llama-cpp PVC so the server pod never downloads it.
# See Dockerfile.model-llama.
target "model-llama" {
  context    = "."
  dockerfile = "Dockerfile.model-llama"
  tags       = ["${REGISTRY}/model-llama:${TAG}"]
}

target "web" {
  inherits = ["_common"]
  context    = "."
  dockerfile = "Dockerfile.web"
  tags       = ["${REGISTRY}/web:${TAG}"]
}

target "providers" {
  context    = "providers"
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/ats:${TAG}"]
}
