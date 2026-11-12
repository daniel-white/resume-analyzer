#!/usr/bin/env bash
# A named profile so this does not touch any other minikube cluster
# (engops uses the default profile). vfkit puts the node on the host's
# L2 domain, which is what the mDNS publisher needs on macOS. The
# node-port range is only read when the cluster is created.
set -euo pipefail

profile=${MINIKUBE_PROFILE:-resume-analyzer}
driver=${MINIKUBE_DRIVER:-vfkit}
# 24g: llama.cpp reserves the whole KV pool up front. The llama-cpp chart
# uses -c 16384 (~1.75 GiB of KV) on top of ~2 GiB of model weights, and the
# rest of the stack (postgres, localstack, envoy, dashboard, provider stubs)
# needs a few GiB too, with room for page cache under load. 4 cpus:
# llama-server uses every core it gets, and prompt processing on 2 vCPUs is
# what made decode retries so slow.
memory=${MINIKUBE_MEMORY:-24g}
cpus=${MINIKUBE_CPUS:-4}
# Kubernetes 1.35 enables the ImageVolume feature by default (Beta in 1.35,
# GA in 1.36). minikube 1.38.1 ships this as its stable version and bundles
# containerd 2.x, which is required to mount an OCI image as a pod volume.
k8s_version=${MINIKUBE_K8S_VERSION:-v1.35.1}

export MINIKUBE_PROFILE=$profile

# Host alone is not enough: an interrupted start/stop can leave the VM up
# while kubelet and the apiserver are still down. Only skip `minikube start`
# when every component is up; otherwise fall through and start (idempotent).
if minikube status --profile "$profile" --format '{{.Host}} {{.Kubelet}} {{.APIServer}}' 2>/dev/null | grep -q '^Running Running Running$'; then
  echo "minikube profile $profile already running"
  # An existing profile keeps the runtime it was created with. Image volumes
  # need containerd; the old docker/cri-dockerd runtime cannot mount them, so
  # the llama-cpp pod would silently start without /models.
  current_runtime=$(kubectl get node "$profile" -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}' 2>/dev/null || true)
  case "$current_runtime" in
    containerd://*|"") ;;
    *)
      echo "warning: $profile is running $current_runtime; image volumes need containerd." >&2
      echo "         run 'minikube delete -p $profile' and re-run this script." >&2
      ;;
  esac
else
  # The llama-cpp chart mounts the model-llama OCI image directly as a pod
  # volume (.spec.volumes[].image). On Kubernetes 1.35 that feature is on by
  # default, so no feature gates are needed here; only a CRI runtime that can
  # mount an OCI image. containerd 2.x does; docker/cri-dockerd does not, so
  # containerd is the default.
  #
  # minikube's --gpus flag is only supported with the Docker driver + Docker
  # container-runtime, which is mutually exclusive with containerd. Set
  # MINIKUBE_GPUS=all (or nvidia/amd; Linux hosts with the vendor toolkit) to
  # opt into GPU passthrough. That mode uses the Docker runtime, so it does
  # NOT get image volumes - run llama.cpp with a copied/PVC model there.
  runtime=containerd
  gpu_args=()
  if [[ -n "${MINIKUBE_GPUS:-}" ]]; then
    runtime=docker
    driver=${MINIKUBE_DRIVER:-docker}
    gpu_args+=(--gpus "$MINIKUBE_GPUS")
  fi

  minikube start --profile "$profile" \
    --driver "$driver" \
    --memory "$memory" \
    --cpus "$cpus" \
    --kubernetes-version "$k8s_version" \
    --container-runtime "$runtime" \
    "${gpu_args[@]}" \
    --wait=all \
    --extra-config=apiserver.service-node-port-range=80-32767
fi

minikube update-context --profile "$profile" >/dev/null
kubectl config use-context "$profile" >/dev/null
echo "kubectl context: $profile ($(minikube ip --profile "$profile"))"
