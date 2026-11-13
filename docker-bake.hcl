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

# Pinned by the flake devShell, which exports the versions it actually
# provides. No default on purpose: an unset value must fail the build rather
# than silently fetching a floating tag.
variable "RUST_VERSION" {}
variable "NODEJS_VERSION" {}
variable "PNPM_VERSION" {}

# Image metadata. CI sets these from the git tag and commit; local builds use
# the defaults.
variable "VERSION" {
  default = "dev"
}
variable "GIT_SHA" {
  default = "unknown"
}
variable "SOURCE_DATE_EPOCH" {
  default = "0"
}
variable "BUILD_DATE" {
  default = "1970-01-01T00:00:00Z"
}

# Registry-backed BuildKit cache. Empty means no cache import/export, which is
# the local-development default. CI sets these per target and architecture.
variable "CACHE_REGISTRY" {
  default = ""
}
variable "CACHE_ARCH" {
  default = "amd64"
}
variable "CACHE_WRITE" {
  default = "false"
}

target "_common" {
  args = {
    RUST_VERSION = "${RUST_VERSION}"
    NODEJS_VERSION = "${NODEJS_VERSION}"
    VERSION = "${VERSION}"
    GIT_SHA = "${GIT_SHA}"
    SOURCE_DATE_EPOCH = "${SOURCE_DATE_EPOCH}"
    BUILD_DATE = "${BUILD_DATE}"
  }
  labels = {
    "org.opencontainers.image.source" = "https://github.com/daniel-white/resume-analyzer"
    "org.opencontainers.image.revision" = "${GIT_SHA}"
    "org.opencontainers.image.version" = "${VERSION}"
    "org.opencontainers.image.created" = "${BUILD_DATE}"
    "org.opencontainers.image.licenses" = "MIT"
  }
}

group "default" {
  targets = ["api"]
}

group "all" {
  targets = ["api", "web", "providers", "model", "model-llama"]
}

# CI and releases build only these. The model images are intentionally not
# part of this group; they need Hugging Face downloads and are local-only.
group "release" {
  targets = ["api", "web"]
}

target "api" {
  inherits = ["_common"]
  context    = "."
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/api:${TAG}"]
  cache-from = CACHE_REGISTRY == "" ? [] : ["type=registry,ref=${CACHE_REGISTRY}/api:buildcache-${CACHE_ARCH}"]
  cache-to = CACHE_REGISTRY == "" || CACHE_WRITE != "true" ? [] : ["type=registry,ref=${CACHE_REGISTRY}/api:buildcache-${CACHE_ARCH},mode=max,oci-mediatypes=true,image-manifest=true"]
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
  args = {
    PNPM_VERSION = "${PNPM_VERSION}"
  }
  cache-from = CACHE_REGISTRY == "" ? [] : ["type=registry,ref=${CACHE_REGISTRY}/web:buildcache-${CACHE_ARCH}"]
  cache-to = CACHE_REGISTRY == "" || CACHE_WRITE != "true" ? [] : ["type=registry,ref=${CACHE_REGISTRY}/web:buildcache-${CACHE_ARCH},mode=max,oci-mediatypes=true,image-manifest=true"]
}

target "providers" {
  context    = "providers"
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/ats:${TAG}"]
}
