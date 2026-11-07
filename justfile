# Run from `nix develop`. Images are bake targets; the cluster is a
# minikube profile named resume-analyzer.

# Dependencies, gateway, and https://app.resume.local. Then the API on the host.
dev:
    ./scripts/up.sh deps
    ATS_PROVIDERS="greenline=http://127.0.0.1:8101,levr=http://127.0.0.1:8102,workdaye=http://127.0.0.1:8103,ash=http://127.0.0.1:8104" CALLBACK_URL="http://host.minikube.internal:8080" cargo run -p app

# Bake the API image, load it, and run it in the cluster.
demo:
    ./scripts/up.sh demo

bake *args:
    ./scripts/bake.sh {{args}}

# Render the chart without a cluster or a certificate.
template:
    helm template resume-analyzer deploy/helm/resume-analyzer
    helm template ats-providers deploy/helm/ats-providers

check:
    ./scripts/check.sh

# Wave catalog, dirty main, and the next wave. `just factory act` runs one.
factory *args:
    ./scripts/factory.sh {{args}}

# Private ATS stubs on 127.0.0.1:8101-8104. Not routed.
# The API uses these ports unless ATS_PROVIDERS is set. ATS_PROVIDERS=stub
# keeps the compiled stub.
providers:
    python3 -m providers --all
