#!/usr/bin/env bash
# Package the resume-analyzer Helm chart deterministically for a release.
#
# Usage:
#   ./scripts/ci/package-chart.sh <VERSION> <API_IMAGE> <WEB_IMAGE> <OUT_DIR>
#
#   VERSION   - release version, e.g. 1.2.3 (the leading 'v' is optional)
#   API_IMAGE - full multi-arch image reference with digest, e.g.
#               ghcr.io/daniel-white/resume-analyzer/api@sha256:...
#   WEB_IMAGE - full multi-arch image reference with digest
#   OUT_DIR   - directory to write the chart tarball and values file into
#
# Produces:
#   <OUT_DIR>/resume-analyzer-<VERSION>.tgz   (Chart.yaml version/appVersion stamped)
#   <OUT_DIR>/values.release-<VERSION>.yaml   (images pinned to the digests)
#
# Determinism: Chart.yaml is stamped with SOURCE_DATE_EPOCH-friendly fields and
# the tarball is created by Helm with reproducible metadata. The same version
# and images always yield the same tarball bytes.
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <VERSION> <API_IMAGE> <WEB_IMAGE> <OUT_DIR>" >&2
  exit 2
fi

version=$1
api_image=$2
web_image=$3
out_dir=$4

# Strip a leading 'v' so the chart version and tag match (1.2.3, not v1.2.3).
version=${version#v}

root=$(git rev-parse --show-toplevel)
cd "$root"
chart=deploy/helm/resume-analyzer

if [[ ! -f "$chart/Chart.yaml" ]]; then
  echo "package-chart: $chart/Chart.yaml not found" >&2
  exit 1
fi

mkdir -p "$out_dir"

# Deterministic build timestamp from the release commit. Helm bakes the
# chart creation time into the tarball unless SOURCE_DATE_EPOCH is set.
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# Copy the chart and stamp Chart.yaml with the release version/appVersion.
cp -a "$chart" "$stage/chart"
chart_yaml="$stage/chart/Chart.yaml"
sed -i \
  -e "s/^version: .*/version: ${version}/" \
  -e "s/^appVersion: .*/appVersion: \"${version}\"/" \
  "$chart_yaml"

# Pin every file's mtime to SOURCE_DATE_EPOCH so helm package emits a
# byte-identical tarball for identical inputs (GNU touch on CI Linux;
# fall back to no-op on systems without @-epoch touch).
if find "$stage/chart" -type f -exec touch -d "@${SOURCE_DATE_EPOCH}" {} + 2>/dev/null; then
  :
else
  echo "package-chart: warning: could not stamp file mtimes; tarball may not be byte-reproducible" >&2
fi

# Render the release values overlay with the pinned images.
sed \
  -e "s|__VERSION__|${version}|g" \
  -e "s|__API_IMAGE__|${api_image}|g" \
  -e "s|__WEB_IMAGE__|${web_image}|g" \
  "$chart/values.release.yaml" > "$stage/values.release-${version}.yaml"

# helm package records SOURCE_DATE_EPOCH and normalizes file metadata, so
# identical inputs yield byte-identical tarballs.
helm package "$stage/chart" \
  --destination "$out_dir" \
  --dependency-update=false >/dev/null

cp "$stage/values.release-${version}.yaml" "$out_dir/values.release-${version}.yaml"

echo "$out_dir/resume-analyzer-${version}.tgz"
echo "$out_dir/values.release-${version}.yaml"
