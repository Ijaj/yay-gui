#!/usr/bin/env bash
# Builds the Arch test image and runs the harness scenarios inside it.
#
#   tests/arch/run.sh [scenario] [flags]
#
# scenario: repo-install | aur-install | remove | all (default). Flags after it
# go to the harness (for example --no-overlay).
#
# Needs docker and a Rust toolchain. The harness binary is built on the host and
# mounted into the container; everything that installs or removes packages runs
# only inside the container. Proxy settings are passed through only when set.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
here="$root/tests/arch"
image="${AURDECK_ARCH_IMAGE:-aurdeck-arch-harness}"

args=("$@")
[ ${#args[@]} -eq 0 ] && args=(all)

docker info >/dev/null 2>&1 || {
  echo "docker is not running (in the cloud agent container: dockerd > /tmp/dockerd.log 2>&1 &)" >&2
  exit 1
}

# Proxy plumbing: only when the environment has it.
build_args=()
run_args=()
for v in HTTPS_PROXY https_proxy HTTP_PROXY http_proxy NO_PROXY no_proxy; do
  if [ -n "${!v:-}" ]; then
    build_args+=(--build-arg "$v")
    run_args+=(-e "$v")
  fi
done
net=()
ca="${AURDECK_PROXY_CA:-/root/.ccr/ca-bundle.crt}"
rm -f "$here"/ca/*.crt
trap 'rm -f "$here"/ca/*.crt' EXIT
if [ -n "${HTTPS_PROXY:-${https_proxy:-}}" ]; then
  net=(--network host)
  if [ -f "$ca" ]; then cp "$ca" "$here/ca/proxy-ca.crt"; fi
fi

echo "==> building the harness binary"
(cd "$root" && cargo build --release -p aurdeck-arch-harness)

if [ "${AURDECK_ARCH_REUSE_IMAGE:-0}" = 1 ] && docker image inspect "$image" >/dev/null 2>&1; then
  echo "==> reusing image $image (AURDECK_ARCH_REUSE_IMAGE=1)"
else
  echo "==> building image $image"
  docker build "${net[@]}" "${build_args[@]}" -t "$image" "$here"
fi
docker run --rm "$image" cat /etc/aurdeck-yay-version

echo "==> running: aurdeck-arch-harness ${args[*]}"
docker run --rm "${net[@]}" "${run_args[@]}" \
  -v "$root/target/release/aurdeck-arch-harness:/usr/local/bin/aurdeck-arch-harness:ro" \
  "$image" aurdeck-arch-harness "${args[@]}"
