#!/usr/bin/env bash
# A named profile so this does not touch any other minikube cluster
# (engops uses the default profile). vfkit puts the node on the host's
# L2 domain, which is what the mDNS publisher needs on macOS. The
# node-port range is only read when the cluster is created.
set -euo pipefail

profile=${MINIKUBE_PROFILE:-resume-analyzer}
driver=${MINIKUBE_DRIVER:-vfkit}
memory=${MINIKUBE_MEMORY:-6g}
cpus=${MINIKUBE_CPUS:-2}

export MINIKUBE_PROFILE=$profile

if minikube status --profile "$profile" --format '{{.Host}}' 2>/dev/null | grep -q Running; then
  echo "minikube profile $profile already running"
else
  minikube start --profile "$profile" \
    --driver "$driver" \
    --memory "$memory" \
    --cpus "$cpus" \
    --wait=all \
    --extra-config=apiserver.service-node-port-range=80-32767
fi

minikube update-context --profile "$profile" >/dev/null
kubectl config use-context "$profile" >/dev/null
echo "kubectl context: $profile ($(minikube ip --profile "$profile"))"
