#!/usr/bin/env bash
# Offline checks for the compiled demo's image and same-origin routing.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"

# The bake file deliberately has no defaults for these; ordinary CI and local
# invocations derive them from the files that own the versions. Do not inherit
# stale values from the calling shell.
export RUST_VERSION=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml | head -1)
export NODEJS_VERSION=$(tr -d '[:space:]' < .node-version)
export PNPM_VERSION=$(sed -n 's/.*"packageManager": "pnpm@\([^"]*\)".*/\1/p' web/package.json)

# The manifest bundle is a Docker cache key. Two runs must produce the same
# bytes or the chef/cook layers rebuild on every invocation.
./scripts/bake-manifests >/dev/null
first=$(git hash-object tmp/cargo-manifests.tar)
./scripts/bake-manifests >/dev/null
second=$(git hash-object tmp/cargo-manifests.tar)
test "$first" = "$second" || {
  echo "demo: cargo-manifests.tar is not deterministic" >&2
  exit 1
}

render=$(helm template resume-analyzer deploy/helm/resume-analyzer \
  --set api.enabled=true --set web.enabled=true)
RENDER="$render" node --input-type=module <<'JS'
import assert from "node:assert/strict";
const documents = process.env.RENDER.split("\n---");
const route = documents.find(doc => doc.includes("kind: HTTPRoute") && doc.includes("  name: app\n"));
assert(route?.includes("app.resume.local"));
assert(route.includes("value: /api\n") && route.includes("name: api\n"));
assert(route.includes("value: /\n") && route.includes("name: web\n"));
assert(route.indexOf("value: /api\n") < route.indexOf("value: /\n"));
const web = documents.find(doc => doc.includes("kind: Deployment") && doc.includes("  name: web\n"));
const api = documents.find(doc => doc.includes("kind: Deployment") && doc.includes("  name: api\n"));
assert(api?.includes("smart_resume=http://smart-resume:8080"));
assert(api?.includes("indy_hackers_hiring=http://indy-hackers-hiring:8080"));
assert(api.includes("blast_track=http://blast-track:8080"));
assert(api.includes("image: resume-analyzer/model:dev"));
assert(api.includes("name: section-model"));
assert(api.includes("- /bin/cp"));
assert(api.includes("name: SECTION_MODEL_DIR"));
assert(api.includes('value: "/models"'));
assert(api.includes("emptyDir: {}"));
// The API aims at the separately deployed llama-cpp chart.
assert(api.includes("value: http://llama-cpp:8080/v1/chat/completions"));
assert(api.includes("name: SUMMARY_MODEL_URL"));
assert(api.includes("name: FEEDBACK_MODEL_URL"));
assert(web?.includes("image: resume-analyzer/web:dev"));
assert(web.includes("value: http://api:8080"));
assert(web.includes('value: "3000"'));
assert(web.includes("readinessProbe:") && web.includes("path: /health"));
JS

providers=$(helm template ats-providers deploy/helm/ats-providers)
RENDER="$providers" node --input-type=module <<'JS'
import assert from "node:assert/strict";
const documents = process.env.RENDER.split("\n---");
const service = documents.find(doc => doc.includes("kind: Service") && doc.includes("  name: smart-resume\n"));
const deployment = documents.find(doc => doc.includes("kind: Deployment") && doc.includes("  name: smart-resume\n"));
assert(service?.includes("type: ClusterIP"));
assert(service.includes("app: smart-resume"));
assert(deployment?.includes('- "smart_resume"'));
assert(deployment.includes("name: PLUGIN_SECRET"));
const indyService = documents.find(doc => doc.includes("kind: Service") && doc.includes("  name: indy-hackers-hiring\n"));
const indyDeployment = documents.find(doc => doc.includes("kind: Deployment") && doc.includes("  name: indy-hackers-hiring\n"));
assert(indyService?.includes("type: ClusterIP"));
assert(indyDeployment?.includes('- "indy_hackers_hiring"'));
assert(indyDeployment.includes("name: PLUGIN_SECRET"));
const blastService = documents.find(doc => doc.includes("kind: Service") && doc.includes("  name: blast-track\n"));
const blastDeployment = documents.find(doc => doc.includes("kind: Deployment") && doc.includes("  name: blast-track\n"));
assert(blastService?.includes("type: ClusterIP"));
assert(blastDeployment?.includes('- "blast_track"'));
assert(blastDeployment.includes("name: PLUGIN_SECRET"));
assert.equal(documents.filter(doc => doc.includes("kind: Service")).length, 7);
assert(!providersHasRoute(documents));
function providersHasRoute(docs) {
  return docs.some(doc => /kind: (HTTPRoute|Ingress|Gateway)\b/.test(doc));
}
JS

llama=$(helm template llama-cpp deploy/helm/llama-cpp)
RENDER="$llama" node --input-type=module <<'JS'
import assert from "node:assert/strict";
const documents = process.env.RENDER.split("\n---");
const pvc = documents.find(doc => doc.includes("kind: PersistentVolumeClaim") && doc.includes("  name: llama-cpp-models\n"));
assert(!pvc, "demo: llama-cpp must not provision a PVC; the model image is mounted read-only");
const deployment = documents.find(doc => doc.includes("kind: Deployment") && doc.includes("  name: llama-cpp\n"));
assert(deployment?.includes("image: ghcr.io/ggml-org/llama.cpp:server"));
assert(deployment.includes("reference: resume-analyzer/model-llama:dev"));
assert(deployment.includes("/models/llama-3.2-3b-instruct-q4_k_m.gguf"));
assert(deployment.includes("path: /health"));
const service = documents.find(doc => doc.includes("kind: Service") && doc.includes("  name: llama-cpp\n"));
assert(service?.includes("app: llama-cpp"));
// No hooks: hook deletion of the PVC under the running pod is what
// deadlocked upgrades of the combined chart, and a hook Job cannot be
// ordered against a regular-resource PVC. The initContainer above is the
// ordering mechanism instead.
assert(!documents.some(doc => doc.includes("helm.sh/hook")));
assert(!documents.some(doc => doc.includes("kind: Job")));
JS

deps=$(helm template resume-analyzer deploy/helm/resume-analyzer)
if grep -q '  name: web$\|  name: app$' <<<"$deps"; then
  echo "demo: deps mode unexpectedly serves the app" >&2
  exit 1
fi
if grep -q llama-cpp <<<"$deps"; then
  echo "demo: llama-cpp is deployed by its own chart, not resume-analyzer" >&2
  exit 1
fi

docker buildx bake --print --file docker-bake.hcl api web providers model model-llama | node --input-type=module -e '
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const targets = JSON.parse(readFileSync(0, "utf8")).target;
assert.equal(targets.web.dockerfile, "Dockerfile.web");
assert(targets.web.tags.includes("resume-analyzer/web:dev"));
assert(targets.api.tags.includes("resume-analyzer/api:dev"));
assert.equal(targets.providers.context, "providers");
assert(targets.providers.tags.includes("resume-analyzer/ats:dev"));
assert.equal(targets.model.dockerfile, "Dockerfile.model");
assert(targets.model.tags.includes("resume-analyzer/model:dev"));
assert.equal(targets["model-llama"].dockerfile, "Dockerfile.model-llama");
assert(targets["model-llama"].tags.includes("resume-analyzer/model-llama:dev"));
'
node --input-type=module <<'JS'
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const script = readFileSync("scripts/up.sh", "utf8");
const steps = [
  '"$root/scripts/bake.sh" --load providers',
  'docker save resume-analyzer/ats:dev | minikube image load --profile "$profile" --overwrite -',
  'helm upgrade --install ats-providers deploy/helm/ats-providers --wait --timeout 5m',
  'kubectl rollout restart deployment -l app.kubernetes.io/name=ats-providers',
  'kubectl rollout status deployment -l app.kubernetes.io/name=ats-providers --timeout=5m',
  'case "$mode" in',
];
let previous = -1;
for (const step of steps) {
  const position = script.indexOf(step);
  assert(position > previous, `provider refresh step missing or out of order: ${step}`);
  previous = position;
}
JS
grep -Fq '"$root/scripts/bake.sh" --load api web model model-llama' scripts/up.sh
grep -Fq 'docker save resume-analyzer/api:dev resume-analyzer/web:dev resume-analyzer/model:dev resume-analyzer/model-llama:dev |' scripts/up.sh
grep -Fq 'minikube image load --profile "$profile" --overwrite -' scripts/up.sh
grep -Fq 'helm upgrade --install llama-cpp deploy/helm/llama-cpp --wait --timeout 5m' scripts/up.sh
if grep -Eq 'minikube image load .*resume-analyzer/(api|web|ats):dev' scripts/up.sh; then
  echo "demo: stream daemon images, not cached named :dev archives" >&2
  exit 1
fi
grep -Fq 'args+=(--set api.enabled=true --set web.enabled=true)' scripts/up.sh
grep -Fq 'COPY web/src ./src' Dockerfile.web
if grep -Fq 'COPY web/ ./' Dockerfile.web; then
  echo "demo: do not copy local generated web state into the image" >&2
  exit 1
fi
grep -Fq 'FROM scratch' Dockerfile.model
if grep -Eq '^FROM debian:bookworm-slim$' Dockerfile.model; then
  echo "demo: the model image must not ship a full Debian userland" >&2
  exit 1
fi
if grep -q 'image inspect resume-analyzer/web' scripts/up.sh; then
  echo "demo: web must be built, not conditionally reused" >&2
  exit 1
fi
echo "demo: compiled images and same-origin routes passed"
