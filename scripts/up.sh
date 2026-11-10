#!/usr/bin/env bash
# deps:  dependencies + gateway + hostnames, API still runs on the host.
# demo:  bake the API, the compiled web app, and the section model, load them,
#        and serve both on https://app.resume.local. /api stays on Rust; / is
#        the web pod. The ATS stubs are a separate chart. ClusterIP only,
#        never a gateway route.
set -euo pipefail

mode=${1:-deps}
root=$(git rev-parse --show-toplevel)
cd "$root"
# Helm OCI pulls in gateway.sh also use Docker's credential helper.
# shellcheck source=docker-cred.sh
source "$root/scripts/docker-cred.sh"
profile=${MINIKUBE_PROFILE:-resume-analyzer}

"$root/scripts/cluster.sh"
"$root/scripts/cert.sh"
"$root/scripts/gateway.sh"

ip=$(minikube ip --profile "$profile")

"$root/scripts/bake.sh" --load providers
docker save resume-analyzer/ats:dev | minikube image load --profile "$profile" --overwrite -
helm upgrade --install ats-providers deploy/helm/ats-providers --wait --timeout 5m
# The :dev tag stays the same across builds, so Helm alone leaves old provider
# processes running. Replace them after loading the fresh image, in both modes,
# and wait before the API can hand off any new work.
kubectl rollout restart deployment -l app.kubernetes.io/name=ats-providers
kubectl rollout status deployment -l app.kubernetes.io/name=ats-providers --timeout=5m

args=(
  upgrade --install resume-analyzer deploy/helm/resume-analyzer
  --wait --timeout 5m
  --set-file gateway.tls.cert=.local-certs/cert.pem
  --set-file gateway.tls.key=.local-certs/key.pem
  --set "gateway.mdns.targetIp=$ip"
  # Dev tool UIs on the tools hostname (Mailpit at /mail, pgAdmin at
  # /pgadmin, static landing page at /). Both `just dev` and `just demo`
  # get the full tools page; pgAdmin is opt-in in values.yaml so this is
  # what turns it on.
  --set pgadmin.enabled=true
)

case "$mode" in
  deps)
    args+=(--set api.enabled=false --set web.enabled=false)
    ;;
  demo)
    "$root/scripts/bake.sh" --load api web model ollama
    # Named :dev loads can reuse Minikube's cached archive even after a new
    # Docker build. Stream the actual daemon images to replace that cache.
    docker save resume-analyzer/api:dev resume-analyzer/web:dev resume-analyzer/model:dev resume-analyzer/ollama:dev |
      minikube image load --profile "$profile" --overwrite -
    args+=(--set api.enabled=true --set web.enabled=true)
    mkdir -p "$root/.run"
    cat >"$root/.run/api-providers.yaml" <<'EOF'
api:
  providers: "greenline=http://greenline:8080,levr=http://levr:8080,workdaye=http://workdaye:8080,ash=http://ash:8080,smart_resume=http://smart-resume:8080,indy_hackers_hiring=http://indy-hackers-hiring:8080,blast_track=http://blast-track:8080"
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

# The local demo tags are reused. Helm alone will not replace pods when
# only the image contents changed.
if [[ "$mode" == "demo" ]]; then
  kubectl rollout restart deployment/api deployment/web
  kubectl rollout status deployment/api --timeout=5m
  kubectl rollout status deployment/web --timeout=5m
fi

echo
echo "https://tools.resume.local"
if [[ "$mode" == "demo" ]]; then
  echo "https://app.resume.local"
  echo "ats providers stay inside the cluster (greenline, levr, workdaye, ash, smart_resume, indy_hackers_hiring, blast_track on :8080)"
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
  forward smart_resume svc/smart-resume 8105:8080
  forward indy_hackers_hiring svc/indy-hackers-hiring 8106:8080
  forward blast_track svc/blast-track 8107:8080
  echo "port-forwards: postgres 54329, localstack 4566, mailpit 1025 and 8025"
  echo "provider port-forwards (not routed): greenline 8101, levr 8102, workdaye 8103, ash 8104, smart_resume 8105, indy_hackers_hiring 8106, blast_track 8107"
fi
