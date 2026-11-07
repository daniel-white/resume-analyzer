#!/usr/bin/env bash
# Gateway API and Envoy Gateway CRDs are applied here, not by Helm.
# The rendered CRD manifest is larger than Helm's release Secret, so
# `helm install` of the CRD chart fails. The controller chart itself is
# small enough to install normally. Versions match the engops cluster.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
# Also support invoking gateway.sh directly, outside up.sh.
# shellcheck source=docker-cred.sh
source "$root/scripts/docker-cred.sh"

profile=${MINIKUBE_PROFILE:-resume-analyzer}
export MINIKUBE_PROFILE=$profile

kubectl apply --server-side --force-conflicts \
  -f https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.5.0/standard-install.yaml

helm template gateway-crds oci://docker.io/envoyproxy/gateway-crds-helm \
  --version v1.8.0 \
  --set crds.gatewayAPI.enabled=false \
  --set crds.envoyGateway.enabled=true \
  | kubectl apply --server-side --force-conflicts -f -

# CRDs were applied above. Skip the copy bundled in the chart so Helm
  # does not fight that field manager, and so the install stays under
  # the release Secret size limit.
helm upgrade --install envoy-gateway oci://docker.io/envoyproxy/gateway-helm \
  --version v1.9.0 \
  --namespace envoy-gateway-system \
  --create-namespace \
  --skip-crds \
  --wait --timeout 5m
