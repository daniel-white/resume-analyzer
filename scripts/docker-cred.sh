# Sourced by Docker/Helm entrypoints; caller sets root. Home Manager can put a
# docker-tools multicall binary ahead of OrbStack's real keychain helper.
# Probe the helper's actual protocol ("version" is not supported by all
# working helpers), without changing ~/.docker/config.json or credentials.
if [[ "$(uname -s)" == Darwin ]] \
    && ! docker-credential-osxkeychain list </dev/null >/dev/null 2>&1; then
  for candidate in \
    /usr/local/bin/docker-credential-osxkeychain \
    "${HOME}/.orbstack/bin/docker-credential-osxkeychain" \
    /Applications/OrbStack.app/Contents/MacOS/xbin/docker-credential-osxkeychain
  do
    if [[ -x "$candidate" ]] && "$candidate" list </dev/null >/dev/null 2>&1; then
      mkdir -p "$root/.run/bin"
      ln -sfn "$candidate" "$root/.run/bin/docker-credential-osxkeychain"
      export PATH="$root/.run/bin:$PATH"
      break
    fi
  done
fi
