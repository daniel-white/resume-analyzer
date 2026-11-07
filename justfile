# Run from `nix develop`. Images are bake targets; the cluster is a
# minikube profile named resume-analyzer.

# Dependencies, gateway, and https://app.resume.local. Then the API on the host.
dev:
    ./scripts/up.sh deps
    cargo run -p app

# Bake the API image, load it, and run it in the cluster.
demo:
    ./scripts/up.sh demo

bake *args:
    ./scripts/bake.sh {{args}}

# Render the chart without a cluster or a certificate.
template:
    helm template resume-analyzer deploy/helm/resume-analyzer

check:
    ./scripts/check.sh

# Wave catalog, dirty main, and the next wave. `just factory act` runs one.
factory *args:
    ./scripts/factory.sh {{args}}
