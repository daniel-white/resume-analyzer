#!/usr/bin/env bash
# deps:  dependencies + gateway + hostnames, API still runs on the host.
# demo:  also load the baked images and run the API (and web, if built)
#        inside the cluster.
# The ATS stubs are a separate chart. ClusterIP only, never a gateway route.
set -euo pipefail

mode=${1:-deps}
root=$(git rev-parse --show-toplevel)
cd "$root"
profile=${MINIKUBE_PROFILE:-resume-analyzer}

"$root/scripts/cluster.sh"
"$root/scripts/cert.sh"
"$root/scripts/gateway.sh"

ip=$(minikube ip --profile "$profile")

docker buildx bake --file docker-bake.hcl providers
minikube image load --profile "$profile" resume-analyzer/ats:dev
helm upgrade --install ats-providers deploy/helm/ats-providers --wait --timeout 5m

args=(
  upgrade --install resume-analyzer deploy/helm/resume-analyzer
  --wait --timeout 5m
  --set-file gateway.tls.cert=.local-certs/cert.pem
  --set-file gateway.tls.key=.local-certs/key.pem
  --set "gateway.mdns.targetIp=$ip"
)

case "$mode" in
  deps)
    args+=(--set api.enabled=false --set web.enabled=false)
    ;;
  demo)
    docker buildx bake --file docker-bake.hcl api
    minikube image load --profile "$profile" resume-analyzer/api:dev
    if docker image inspect resume-analyzer/web:dev >/dev/null 2>&1; then
      minikube image load --profile "$profile" resume-analyzer/web:dev
      args+=(--set web.enabled=true)
    fi
    args+=(--set api.enabled=true)
    mkdir -p "$root/.run"
    cat >"$root/.run/api-providers.yaml" <<'EOF'
api:
  providers: "greenline=http://greenline:8080,levr=http://levr:8080,workdaye=http://workdaye:8080,ash=http://ash:8080"
  callbackUrl: http://api:8080
  pluginSecret: spine-stub-secret
EOF
    args+=(--values "$root/.run/api-providers.yaml")
    ;;
  *)
    echo "usage: up.sh deps|demo" >&2
    exit 1
    ;;
esac

helm "${args[@]}"

echo
echo "https://mail.resume.local"
if [[ "$mode" == "demo" ]]; then
  echo "https://app.resume.local"
  echo "ats providers stay inside the cluster (greenline, levr, workdaye, ash on :8080)"
else
  echo "API is on the host. https://app.resume.local serves it only in demo mode."
fi
echo "node $ip  (mDNS; if the name does not resolve, the driver is not on L2)"

if [[ "$mode" == "deps" ]]; then
  run=$root/.run
  mkdir -p "$run"
  forward() {
    local name=$1
    shift
    if [[ -f "$run/$name.pid" ]] && kill -0 "$(cat "$run/$name.pid")" 2>/dev/null; then
      return
    fi
    kubectl port-forward "$@" >"$run/$name.log" 2>&1 &
    echo $! >"$run/$name.pid"
  }
  forward postgres svc/postgres 54329:5432
  forward localstack svc/localstack 4566:4566
  forward mailpit svc/mailpit 1025:1025 8025:8025
  forward greenline svc/greenline 8101:8080
  forward levr svc/levr 8102:8080
  forward workdaye svc/workdaye 8103:8080
  forward ash svc/ash 8104:8080
  echo "port-forwards: postgres 54329, localstack 4566, mailpit 1025 and 8025"
  echo "provider port-forwards (not routed): greenline 8101, levr 8102, workdaye 8103, ash 8104"
fi
