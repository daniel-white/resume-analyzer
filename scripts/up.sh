#!/usr/bin/env bash
# deps:  dependencies + gateway + hostnames, API still runs on the host.
# demo:  also load the baked images and run the API (and web, if built)
#        inside the cluster.
set -euo pipefail

mode=${1:-deps}
root=$(git rev-parse --show-toplevel)
cd "$root"
profile=${MINIKUBE_PROFILE:-resume-analyzer}

"$root/scripts/cluster.sh"
"$root/scripts/cert.sh"
"$root/scripts/gateway.sh"

ip=$(minikube ip --profile "$profile")
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
  echo "port-forwards: postgres 54329, localstack 4566, mailpit 1025 and 8025"
fi
