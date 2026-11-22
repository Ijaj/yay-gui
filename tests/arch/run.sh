#!/usr/bin/env bash
# Builds the Arch test image and runs the harness scenarios inside it.
#
#   tests/arch/run.sh [scenario] [flags]
#
# scenario: repo-install | aur-install | pin-commit-mismatch | pin-pkgbuild-mismatch | pin-unapproved-base |
# pin-unreviewed-upgrade | remove | all (default). Flags after it
# go to the harness (for example --no-overlay).
#
# Needs docker and a Rust toolchain. The harness binary is built on the host and
# mounted into the container; everything that installs or removes packages runs
# only inside the container. Proxy settings are passed through only when set.
#
# Environment:
#   AURDECK_ARCH_IMAGE        image name (default aurdeck-arch-harness)
#   AURDECK_ARCH_PULL=0       do not `docker build --pull` (when the registry is unreachable)
#   AURDECK_ARCH_REUSE_IMAGE=1  skip the build if the image is less than a day old
#   AURDECK_PROXY_CA          proxy CA bundle (default /root/.ccr/ca-bundle.crt)
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
target="$(cd "$root" && cargo metadata --format-version 1 --no-deps |
  sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
bin="$target/release/aurdeck-arch-harness"
[ -x "$bin" ] || { echo "harness binary not found at $bin" >&2; exit 1; }

# A cached image keeps the pacman -Syu layer it was built with, and the
# scenarios assume a freshly upgraded system, so an image older than a day is
# never reused.
reuse=0
if [ "${AURDECK_ARCH_REUSE_IMAGE:-0}" = 1 ] && created="$(docker image inspect -f '{{.Created}}' "$image" 2>/dev/null)"; then
  age=$(($(date +%s) - $(date -d "$created" +%s)))
  if [ "$age" -lt 86400 ]; then
    reuse=1
  else
    echo "==> image $image is $((age / 3600)) h old; rebuilding"
  fi
fi
if [ "$reuse" = 1 ]; then
  echo "==> reusing image $image (AURDECK_ARCH_REUSE_IMAGE=1)"
else
  echo "==> building image $image"
  pull=(--pull)
  [ "${AURDECK_ARCH_PULL:-1}" = 0 ] && pull=()
  docker build "${pull[@]}" "${net[@]}" "${build_args[@]}" \
    --build-arg "AURDECK_CACHE_BUST=$(date +%Y%m%d)" -t "$image" "$here"
fi
docker run --rm "$image" cat /etc/aurdeck-yay-version /etc/aurdeck-yay-bin-commit

# Capabilities: with --network host a root process in the container (a hostile
# PKGBUILD can sudo with the known password) must not get raw sockets on the
# host's interfaces. Drop everything and add back only what pacman, sudo and
# makepkg need.
caps=(--cap-drop ALL)
for c in CHOWN DAC_OVERRIDE FOWNER FSETID KILL SETFCAP SETGID SETPCAP SETUID SYS_CHROOT AUDIT_WRITE; do
  caps+=(--cap-add "$c")
done

echo "==> running: aurdeck-arch-harness ${args[*]}"
docker run --rm "${net[@]}" "${caps[@]}" "${run_args[@]}" \
  -v "$bin:/usr/local/bin/aurdeck-arch-harness:ro" \
  "$image" aurdeck-arch-harness "${args[@]}"
