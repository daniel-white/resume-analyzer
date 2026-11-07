#!/usr/bin/env bash
# Offline checks for the compiled demo's image and same-origin routing.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"

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

deps=$(helm template resume-analyzer deploy/helm/resume-analyzer)
if grep -q '  name: web$\|  name: app$' <<<"$deps"; then
  echo "demo: deps mode unexpectedly serves the app" >&2
  exit 1
fi

docker buildx bake --print --file docker-bake.hcl api web providers | node --input-type=module -e '
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const targets = JSON.parse(readFileSync(0, "utf8")).target;
assert.equal(targets.web.dockerfile, "Dockerfile.web");
assert(targets.web.tags.includes("resume-analyzer/web:dev"));
assert(targets.api.tags.includes("resume-analyzer/api:dev"));
assert.equal(targets.providers.context, "providers");
assert(targets.providers.tags.includes("resume-analyzer/ats:dev"));
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
grep -Fq '"$root/scripts/bake.sh" --load api web' scripts/up.sh
grep -Fq 'docker save resume-analyzer/api:dev resume-analyzer/web:dev |' scripts/up.sh
grep -Fq 'minikube image load --profile "$profile" --overwrite -' scripts/up.sh
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
if grep -q 'image inspect resume-analyzer/web' scripts/up.sh; then
  echo "demo: web must be built, not conditionally reused" >&2
  exit 1
fi
echo "demo: compiled images and same-origin routes passed"
