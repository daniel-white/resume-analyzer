#!/usr/bin/env bash
# Wildcard cert for the local demo hostnames. mkcert's CA has to be in
# the trust store or the browser will reject https://app.resume.local,
# which also means passkeys will not treat it as a secure context.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

cert_dir=$root/.local-certs
if [[ -f "$cert_dir/cert.pem" && -f "$cert_dir/key.pem" ]]; then
  echo "cert already present in .local-certs/; delete the directory to reissue"
  exit 0
fi

mkdir -p "$cert_dir"
mkcert -install
mkcert \
  -cert-file "$cert_dir/cert.pem" \
  -key-file "$cert_dir/key.pem" \
  "*.resume.local" resume.local
echo "issued .local-certs/{cert,key}.pem"
