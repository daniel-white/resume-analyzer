# Versions are still pinned in the Dockerfiles. The follow-up in
# docs/plan.md moves RUST_VERSION and NODEJS_VERSION here, with no
# default, fed by the flake devShell.

variable "REGISTRY" {
  default = "resume-analyzer"
}

variable "TAG" {
  default = "dev"
}

group "default" {
  targets = ["api"]
}

group "all" {
  targets = ["api", "web", "providers", "model"]
}

target "api" {
  context    = "."
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/api:${TAG}"]
}

target "model" {
  context    = "."
  dockerfile = "Dockerfile.model"
  tags       = ["${REGISTRY}/model:${TAG}"]
}

# The candidate-profile GGUF. Baked once, then a Job loads it into the llama-cpp
# PVC so the server pod never downloads it. See Dockerfile.model-llama.
target "model-llama" {
  context    = "."
  dockerfile = "Dockerfile.model-llama"
  tags       = ["${REGISTRY}/model-llama:${TAG}"]
}

target "web" {
  context    = "."
  dockerfile = "Dockerfile.web"
  tags       = ["${REGISTRY}/web:${TAG}"]
}

target "providers" {
  context    = "providers"
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/ats:${TAG}"]
}
