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
  targets = ["api", "web"]
}

target "api" {
  context    = "."
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/api:${TAG}"]
}

target "web" {
  context    = "."
  dockerfile = "Dockerfile.web"
  tags       = ["${REGISTRY}/web:${TAG}"]
}
