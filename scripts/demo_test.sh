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
assert(web?.includes("image: resume-analyzer/web:dev"));
assert(web.includes("value: http://api:8080"));
assert(web.includes('value: "3000"'));
assert(web.includes("readinessProbe:") && web.includes("path: /health"));
JS

deps=$(helm template resume-analyzer deploy/helm/resume-analyzer)
if grep -q '  name: web$\|  name: app$' <<<"$deps"; then
  echo "demo: deps mode unexpectedly serves the app" >&2
  exit 1
fi

docker buildx bake --print --file docker-bake.hcl api web | node --input-type=module -e '
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const targets = JSON.parse(readFileSync(0, "utf8")).target;
assert.equal(targets.web.dockerfile, "Dockerfile.web");
assert(targets.web.tags.includes("resume-analyzer/web:dev"));
assert(targets.api.tags.includes("resume-analyzer/api:dev"));
'
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
